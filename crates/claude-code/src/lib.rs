mod mcp;
mod protocol;

use std::io;
use std::path::PathBuf;
use std::process::Stdio;

use serde::Serialize;
use serde::de::{DeserializeOwned, IgnoredAny};
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub use self::mcp::{McpReply, McpTool, SdkServer};
use self::protocol::{
    AuthStatus, Authentication, ControlRequestInput, Initialized, Reported, Request,
};
pub use self::protocol::{
    ControlReply, ControlRequest, ControlResponse, ExtraUsage, Finished, Limit, LimitScope, Named,
    Output, RateLimits, RpcRequest, UsageReport, UserInput,
};

const INITIALIZE: &str = "initialize";
const AUTHENTICATE: &str = "authenticate";
const CALLBACK: &str = "callback";
const USAGE: &str = "/usage";

#[derive(Debug, thiserror::Error)]
pub enum ClaudeCodeError {
    #[error("running claude code: {0}")]
    Io(#[from] io::Error),
    #[error("malformed claude code output: {0}")]
    Json(#[from] serde_json::Error),
    #[error("claude code exited unexpectedly: {0}")]
    Exited(String),
    #[error("claude code failed: {0}")]
    Failed(String),
    #[error("no claude code sign-in is in progress, start one first")]
    NoLogin,
    #[error("paste the full code shown after signing in")]
    InvalidCode,
    #[error("claude code reported no plan usage, sign in with a Claude subscription")]
    NoUsage,
}

pub struct Cli {
    command: PathBuf,
    directory: PathBuf,
    login: Mutex<Option<Process>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub signed_in: bool,
    pub email: Option<String>,
    pub subscription: Option<String>,
}

impl From<AuthStatus> for Account {
    fn from(status: AuthStatus) -> Self {
        Self {
            signed_in: status.logged_in,
            email: status.email,
            subscription: status.subscription_type,
        }
    }
}

pub struct Process {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    stderr: JoinHandle<String>,
}

impl Process {
    pub async fn send<M>(&mut self, message: &M) -> Result<(), ClaudeCodeError>
    where
        M: Serialize + Sync,
    {
        let mut line = serde_json::to_string(message)?;
        tracing::trace!(line, "to claude code");
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await?;
        Ok(())
    }

    pub async fn next<E>(&mut self) -> Result<Output<E>, ClaudeCodeError>
    where
        E: DeserializeOwned,
    {
        loop {
            let Some(line) = self.lines.next_line().await? else {
                let stderr = (&mut self.stderr).await.unwrap_or_default();
                return Err(ClaudeCodeError::Exited(stderr.trim().to_owned()));
            };

            if !line.trim().is_empty() {
                tracing::trace!(line, "from claude code");
                return Ok(serde_json::from_str(&line)?);
            }
        }
    }

    pub async fn initialize(&mut self, servers: &[&str]) -> Result<(), ClaudeCodeError> {
        self.control(
            INITIALIZE,
            Request::Initialize {
                sdk_mcp_servers: servers,
            },
        )
        .await
    }

    async fn control(&mut self, id: &str, request: Request<'_>) -> Result<(), ClaudeCodeError> {
        self.send(&ControlRequestInput {
            request_id: id,
            request,
        })
        .await
    }

    async fn request<T>(&mut self, id: &str, request: Request<'_>) -> Result<T, ClaudeCodeError>
    where
        T: DeserializeOwned,
    {
        self.control(id, request).await?;
        self.response(id).await
    }

    async fn response<T>(&mut self, id: &str) -> Result<T, ClaudeCodeError>
    where
        T: DeserializeOwned,
    {
        loop {
            match self.next::<IgnoredAny>().await? {
                Output::ControlResponse {
                    response:
                        ControlResponse::Success {
                            request_id,
                            response,
                        },
                } if request_id == id => return Ok(serde_json::from_value(response)?),
                Output::ControlResponse {
                    response: ControlResponse::Error { request_id, error },
                } if request_id == id => return Err(ClaudeCodeError::Failed(error)),
                _ => {},
            }
        }
    }

    async fn finish(self) -> Result<(), ClaudeCodeError> {
        let Self {
            mut child, stdin, ..
        } = self;
        drop(stdin);
        child.wait().await?;
        Ok(())
    }
}

impl Cli {
    pub fn new(command: PathBuf, directory: PathBuf) -> Self {
        Self {
            command,
            directory,
            login: Mutex::new(None),
        }
    }

    pub fn spawn(
        &self,
        args: impl IntoIterator<Item = String>,
    ) -> Result<Process, ClaudeCodeError> {
        let mut child = Command::new(&self.command)
            .args([
                "-p",
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
                "--verbose",
                "--no-session-persistence",
                "--tools",
                "",
                "--setting-sources=",
                "--strict-mcp-config",
                "--disable-slash-commands",
            ])
            .args(args)
            .current_dir(&self.directory)
            .env_remove("CLAUDECODE")
            .env("CLAUDE_CODE_DISABLE_CLAUDE_MDS", "1")
            .env("ENABLE_TOOL_SEARCH", "false")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let (Some(stdin), Some(stdout), Some(mut stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            return Err(io::Error::other("claude code was spawned without pipes").into());
        };

        Ok(Process {
            child,
            stdin,
            lines: BufReader::new(stdout).lines(),
            stderr: tokio::spawn(async move {
                let mut text = String::new();
                let _ = stderr.read_to_string(&mut text).await;
                text
            }),
        })
    }

    pub async fn models(&self) -> Result<Vec<String>, ClaudeCodeError> {
        let mut process = self.spawn([])?;
        process.initialize(&[]).await?;
        let initialized: Initialized = process.response(INITIALIZE).await?;
        let mut models: Vec<String> = Vec::new();

        for model in initialized.models {
            let id = model.resolved_model.unwrap_or(model.value);

            if !models.contains(&id) {
                models.push(id);
            }
        }

        Ok(models)
    }

    pub async fn account(&self) -> Result<Account, ClaudeCodeError> {
        let output = Command::new(&self.command)
            .args(["auth", "status", "--json"])
            .current_dir(&self.directory)
            .env_remove("CLAUDECODE")
            .kill_on_drop(true)
            .output()
            .await?;
        let status: AuthStatus = serde_json::from_slice(&output.stdout)?;
        Ok(status.into())
    }

    pub async fn usage(&self) -> Result<UsageReport, ClaudeCodeError> {
        let output = Command::new(&self.command)
            .args([
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--no-session-persistence",
                "--tools",
                "",
                "--setting-sources=",
                "--strict-mcp-config",
                USAGE,
            ])
            .current_dir(&self.directory)
            .env_remove("CLAUDECODE")
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await?;

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| serde_json::from_str::<Reported>(line).ok())
            .find_map(|line| line.usage_report)
            .ok_or(ClaudeCodeError::NoUsage)
    }

    pub async fn start_login(&self) -> Result<String, ClaudeCodeError> {
        let mut login = self.login.lock().await;
        *login = None;

        let mut process = self.spawn([])?;
        process.initialize(&[]).await?;
        let _: IgnoredAny = process.response(INITIALIZE).await?;
        let authentication: Authentication = process
            .request(
                AUTHENTICATE,
                Request::ClaudeAuthenticate {
                    login_with_claude_ai: true,
                },
            )
            .await?;

        *login = Some(process);
        Ok(authentication.manual_url)
    }

    pub async fn finish_login(&self, code: &str) -> Result<Account, ClaudeCodeError> {
        let (code, state) = code
            .trim()
            .split_once('#')
            .filter(|(code, state)| !code.is_empty() && !state.is_empty())
            .ok_or(ClaudeCodeError::InvalidCode)?;
        let mut process = self
            .login
            .lock()
            .await
            .take()
            .ok_or(ClaudeCodeError::NoLogin)?;

        let _: IgnoredAny = process
            .request(
                CALLBACK,
                Request::ClaudeOauthCallback {
                    authorization_code: code,
                    state,
                },
            )
            .await?;
        process.finish().await?;
        self.account().await
    }
}
