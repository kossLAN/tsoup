use schemars::JsonSchema;
use serde::Deserialize;
use tsoup_proto::{Request, Response};

use super::{Outcome, Tool, ToolError};
use crate::session::Session;

const TIMEOUT_SECS: u64 = 600;

pub struct Shell;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "The command, run with sh -c in /workspace")]
    command: String,
    #[schemars(
        description = "Start the command detached without waiting for it, for graphical programs and servers"
    )]
    #[serde(default)]
    background: bool,
}

impl Tool for Shell {
    const NAME: &'static str = "shell";
    const DESCRIPTION: &'static str = "Run a shell command in the VM.";

    type Arguments = Arguments;

    async fn run(
        &self,
        session: &Session,
        Arguments {
            command,
            background,
        }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let request = if background {
            Request::Spawn { command }
        } else {
            Request::Exec {
                command,
                timeout_secs: TIMEOUT_SECS,
            }
        };

        match session.vm.request(request).await? {
            Response::Exited {
                code,
                stdout,
                stderr,
            } => Ok(Outcome {
                output: format_exit(code, &stdout, &stderr),
                error: code != Some(0),
                diff: None,
            }),
            Response::Spawned => Ok(Outcome::text("started in the background")),
            Response::TimedOut => Err(ToolError::TimedOut(TIMEOUT_SECS)),
            Response::Failed { message } => Err(ToolError::Guest(message)),
            Response::Pong | Response::Forwarded => Err(ToolError::UnexpectedResponse),
        }
    }
}

fn format_exit(code: Option<i32>, stdout: &str, stderr: &str) -> String {
    let mut output = code.map_or_else(
        || "killed by a signal".to_owned(),
        |code| format!("exit code {code}"),
    );

    for (name, stream) in [("stdout", stdout), ("stderr", stderr)] {
        if !stream.is_empty() {
            output.push_str(&format!("\n{name}:\n{stream}"));
        }
    }

    output
}
