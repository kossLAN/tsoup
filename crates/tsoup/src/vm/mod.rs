mod build;
mod preview;

use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use serde::Serialize;
use tokio::net::{UnixListener, UnixStream};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, broadcast, watch};
use tokio_util::codec::Framed;
use tsoup_proto::{CodecError, Frame, HostCodec, McpFailure, McpTool, Request, Response};

use self::build::RUNNER_LINK;
pub use self::build::{BuildError, Builder, describe};
pub use self::preview::Preview;
use crate::session::{Event, WORKSPACE_DIR};

const AGENT_SOCKET: &str = "agent.sock";
const CONSOLE_LOG: &str = "console.log";
const BOOT_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum VmState {
    Stopped,
    Building,
    Booting,
    Running { previews: Vec<Preview> },
    Failed { message: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct VmStatus {
    #[serde(flatten)]
    pub state: VmState,
    pub built: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error(transparent)]
    Build(#[from] BuildError),
    #[error("starting the VM: {0}")]
    Io(#[from] io::Error),
    #[error("reading the VM previews: {0}")]
    Previews(#[from] serde_json::Error),
    #[error("the VM exited while booting ({0}), see {CONSOLE_LOG}")]
    Exited(ExitStatus),
    #[error("the VM did not boot within {} seconds, see {CONSOLE_LOG}", BOOT_TIMEOUT.as_secs())]
    BootTimeout,
    #[error("no VM is running, call create_vm or run_vm first")]
    NotRunning,
    #[error("the VM is already running")]
    AlreadyRunning,
    #[error("no VM has been built yet, call create_vm first")]
    NotBuilt,
    #[error(
        "the VM has no preview named {0} that can show a port, enable the module that provides it"
    )]
    NoForward(String),
    #[error("talking to the guest agent: {0}")]
    Guest(#[from] CodecError),
    #[error("the guest agent hung up")]
    Disconnected,
    #[error("the guest agent failed: {0}")]
    GuestFailed(String),
    #[error("the guest agent sent an unexpected response")]
    UnexpectedResponse,
}

pub struct Vm {
    dir: PathBuf,
    runtime: PathBuf,
    builder: Arc<Builder>,
    running: Mutex<Option<Running>>,
    status: watch::Sender<VmStatus>,
    events: broadcast::Sender<Event>,
}

impl Vm {
    pub async fn new(
        dir: PathBuf,
        runtime: PathBuf,
        builder: Arc<Builder>,
        events: broadcast::Sender<Event>,
    ) -> Self {
        let built = tokio::fs::try_exists(dir.join(RUNNER_LINK))
            .await
            .unwrap_or(false);

        Self {
            dir,
            runtime,
            builder,
            running: Mutex::new(None),
            status: watch::Sender::new(VmStatus {
                state: VmState::Stopped,
                built,
            }),
            events,
        }
    }

    pub fn status(&self) -> VmStatus {
        self.status.borrow().clone()
    }

    pub fn socket(&self, preview: &str) -> Option<PathBuf> {
        let VmState::Running { previews } = &self.status.borrow().state else {
            return None;
        };

        previews
            .iter()
            .find(|candidate| candidate.name == preview)
            .map(|preview| preview.socket.clone())
    }

    pub async fn create(
        &self,
        module: Option<String>,
        modules: Option<Vec<String>>,
    ) -> Result<Vec<McpFailure>, VmError> {
        let mut running = self.running.lock().await;

        self.set_state(VmState::Building);

        let runner = match self
            .builder
            .build(&self.dir, &self.runtime, module, modules)
            .await
        {
            Ok(runner) => runner,
            Err(error) => {
                self.set_state(running.as_ref().map_or_else(
                    || VmState::Failed {
                        message: error.to_string(),
                    },
                    Running::state,
                ));
                return Err(error.into());
            },
        };

        self.update(|status| status.built = true);
        stop(&mut running).await;
        self.boot(&mut running, &runner).await
    }

    pub async fn run(&self) -> Result<Vec<McpFailure>, VmError> {
        let mut running = self.running.lock().await;

        if running.is_some() {
            return Err(VmError::AlreadyRunning);
        }

        let runner = self.dir.join(RUNNER_LINK);

        if !tokio::fs::try_exists(&runner).await? {
            self.update(|status| status.built = false);
            return Err(VmError::NotBuilt);
        }

        self.boot(&mut running, &runner).await
    }

    async fn boot(
        &self,
        running: &mut Option<Running>,
        runner: &Path,
    ) -> Result<Vec<McpFailure>, VmError> {
        self.set_state(VmState::Booting);

        match Running::launch(&self.dir, &self.runtime, runner).await {
            Ok((launched, failures)) => {
                self.set_state(launched.state());
                *running = Some(launched);
                Ok(failures)
            },
            Err(error) => {
                self.set_state(VmState::Failed {
                    message: error.to_string(),
                });
                Err(error)
            },
        }
    }

    pub async fn destroy(&self) {
        stop(&mut *self.running.lock().await).await;

        match tokio::fs::remove_dir_all(&self.runtime).await {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                tracing::warn!(%error, "removing a VM runtime directory");
            },
            _ => {},
        }

        self.set_state(VmState::Stopped);
    }

    pub async fn reconcile(&self) {
        let running = self.running.lock().await;

        if matches!(
            self.status.borrow().state,
            VmState::Building | VmState::Booting
        ) {
            self.set_state(running.as_ref().map_or(VmState::Stopped, Running::state));
        }
    }

    pub async fn mcp_tools(&self) -> Vec<McpTool> {
        self.running
            .lock()
            .await
            .as_ref()
            .map(|vm| vm.tools.clone())
            .unwrap_or_default()
    }

    pub async fn request(&self, request: Request) -> Result<Response, VmError> {
        self.exchange(&mut *self.running.lock().await, request)
            .await
    }

    pub async fn spawn(&self, command: String) -> Result<(), VmError> {
        match self.request(Request::Spawn { command }).await? {
            Response::Spawned => Ok(()),
            Response::Failed { message } => Err(VmError::GuestFailed(message)),
            _ => Err(VmError::UnexpectedResponse),
        }
    }

    pub async fn show_preview(&self, name: &str, port: u16) -> Result<(), VmError> {
        let mut running = self.running.lock().await;
        self.forward(&mut running, name, port, true).await
    }

    pub async fn restore_previews(&self) -> Result<(), VmError> {
        let mut running = self.running.lock().await;
        let mut targets = Preview::saved_targets(&self.dir).await;

        targets.retain(|name, _| {
            running
                .as_ref()
                .and_then(|vm| vm.forwarded(name))
                .is_some_and(|preview| preview.target.is_none())
        });

        for (name, port) in targets {
            self.forward(&mut running, &name, port, false).await?;
        }

        Ok(())
    }

    async fn forward(
        &self,
        running: &mut Option<Running>,
        name: &str,
        port: u16,
        require_listener: bool,
    ) -> Result<(), VmError> {
        let from = running
            .as_ref()
            .ok_or(VmError::NotRunning)?
            .forwarded(name)
            .map(|preview| preview.port)
            .ok_or_else(|| VmError::NoForward(name.to_owned()))?;

        let request = Request::Forward {
            from,
            to: port,
            require_listener,
        };

        match self.exchange(running, request).await? {
            Response::Forwarded => {},
            Response::Failed { message } => return Err(VmError::GuestFailed(message)),
            _ => return Err(VmError::UnexpectedResponse),
        }

        if let Some(vm) = running.as_mut() {
            if let Some(preview) = vm.forwarded_mut(name) {
                preview.target = Some(port);
            }

            if let Err(error) = Preview::save_targets(&self.dir, &vm.previews).await {
                tracing::warn!(%error, "saving the VM preview targets");
            }

            self.set_state(vm.state());
        }

        Ok(())
    }

    async fn exchange(
        &self,
        running: &mut Option<Running>,
        request: Request,
    ) -> Result<Response, VmError> {
        let vm = running.as_mut().ok_or(VmError::NotRunning)?;

        match vm.guest.request(request).await {
            Ok(response) => Ok(response),
            Err(error) => {
                stop(running).await;
                self.set_state(VmState::Failed {
                    message: error.to_string(),
                });
                Err(error)
            },
        }
    }

    fn set_state(&self, state: VmState) {
        self.update(|status| status.state = state);
    }

    fn update(&self, change: impl FnOnce(&mut VmStatus)) {
        self.status.send_modify(change);
        let _ = self.events.send(Event::Vm {
            status: self.status(),
        });
    }
}

async fn stop(running: &mut Option<Running>) {
    if let Some(mut vm) = running.take()
        && let Err(error) = vm.process.kill().await
    {
        tracing::warn!(%error, "killing a VM");
    }
}

struct Running {
    process: Child,
    guest: Guest,
    previews: Vec<Preview>,
    tools: Vec<McpTool>,
}

impl Running {
    async fn launch(
        dir: &Path,
        runtime: &Path,
        runner: &Path,
    ) -> Result<(Self, Vec<McpFailure>), VmError> {
        let previews = Preview::load(runner).await?;
        tokio::fs::create_dir_all(runtime).await?;

        let stale = previews
            .iter()
            .map(|preview| preview.socket.clone())
            .chain([AGENT_SOCKET, WORKSPACE_DIR].map(|name| runtime.join(name)));

        for path in stale {
            match tokio::fs::remove_file(path).await {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
                _ => {},
            }
        }

        tokio::fs::symlink(dir.join(WORKSPACE_DIR), runtime.join(WORKSPACE_DIR)).await?;

        let listener = UnixListener::bind(runtime.join(AGENT_SOCKET))?;
        let console = tokio::fs::File::create(dir.join(CONSOLE_LOG))
            .await?
            .into_std()
            .await;
        let mut process = Command::new(runner.join("bin/microvm-run"))
            .current_dir(runtime)
            .stdin(Stdio::null())
            .stdout(console.try_clone()?)
            .stderr(console)
            .kill_on_drop(true)
            .spawn()?;

        let connect = async {
            let (stream, _) = listener.accept().await?;
            let mut guest = Guest::new(stream);
            guest.request(Request::Ping).await?;

            match guest.request(Request::McpTools).await? {
                Response::McpTools { tools, failures } => Ok((guest, tools, failures)),
                _ => Err(VmError::UnexpectedResponse),
            }
        };

        let (guest, tools, failures) = tokio::select! {
            guest = tokio::time::timeout(BOOT_TIMEOUT, connect) => {
                guest.map_err(|_| VmError::BootTimeout)??
            },
            status = process.wait() => return Err(VmError::Exited(status?)),
        };

        Ok((
            Self {
                process,
                guest,
                previews,
                tools,
            },
            failures,
        ))
    }

    fn state(&self) -> VmState {
        VmState::Running {
            previews: self
                .previews
                .iter()
                .filter(|preview| preview.visible())
                .cloned()
                .collect(),
        }
    }

    fn forwarded(&self, name: &str) -> Option<&Preview> {
        self.previews
            .iter()
            .find(|preview| preview.forwarded && preview.name == name)
    }

    fn forwarded_mut(&mut self, name: &str) -> Option<&mut Preview> {
        self.previews
            .iter_mut()
            .find(|preview| preview.forwarded && preview.name == name)
    }
}

struct Guest {
    framed: Framed<UnixStream, HostCodec>,
    last_id: u64,
}

impl Guest {
    fn new(stream: UnixStream) -> Self {
        Self {
            framed: Framed::new(stream, HostCodec::default()),
            last_id: 0,
        }
    }

    async fn request(&mut self, body: Request) -> Result<Response, VmError> {
        self.last_id += 1;
        let id = self.last_id;
        self.framed.send(Frame { id, body }).await?;

        loop {
            let frame = self.framed.next().await.ok_or(VmError::Disconnected)??;

            if frame.id == id {
                return Ok(frame.body);
            }
        }
    }
}
