use schemars::JsonSchema;
use serde::Deserialize;
use tsoup_proto::Exit;

use super::{Outcome, Tool, ToolError};
use crate::session::Session;
use crate::workspace::Workspace;

const TIMEOUT_SECS: u64 = 600;

pub struct Shell;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "The command, run with sh -c in the working directory")]
    command: String,
    #[schemars(
        description = "Start the command detached without waiting for it, for graphical programs and servers"
    )]
    #[serde(default)]
    background: bool,
}

impl<E: Workspace> Tool<E> for Shell {
    const NAME: &'static str = "shell";
    const DESCRIPTION: &'static str = "Run a shell command.";

    type Arguments = Arguments;

    async fn run(
        &self,
        _session: &Session,
        environment: &E,
        Arguments {
            command,
            background,
        }: Arguments,
    ) -> Result<Outcome, ToolError> {
        if background {
            environment.spawn(command).await?;
            return Ok(Outcome::text("started in the background"));
        }

        let exit = environment
            .exec(command, TIMEOUT_SECS)
            .await?
            .ok_or(ToolError::TimedOut(TIMEOUT_SECS))?;

        Ok(Outcome {
            output: format_exit(&exit),
            error: exit.code != Some(0),
            diff: None,
        })
    }
}

fn format_exit(
    Exit {
        code,
        stdout,
        stderr,
    }: &Exit,
) -> String {
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
