use std::num::NonZeroU64;

use schemars::JsonSchema;
use serde::Deserialize;
use tsoup_proto::Exit;

use super::{Caller, Outcome, Tool, ToolError};
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
    #[schemars(
        description = "Seconds the command may run before it is killed, 600 when omitted. Raise it for a long build or job instead of polling for it with sleep, lower it for a command that should be quick."
    )]
    #[serde(default)]
    timeout: Option<NonZeroU64>,
}

impl<E: Workspace> Tool<E> for Shell {
    const NAME: &'static str = "shell";
    const DESCRIPTION: &'static str = "Run a shell command with sh -c.
Every command starts in the working directory, so never cd into it first,
and a cd inside one command does not carry over to the next.";

    type Arguments = Arguments;

    async fn run(
        &self,
        _caller: &Caller<'_>,
        environment: &E,
        Arguments {
            command,
            background,
            timeout,
        }: Arguments,
    ) -> Result<Outcome, ToolError> {
        if background {
            environment.spawn(command).await?;
            return Ok(Outcome::text("started in the background"));
        }

        let timeout = timeout.map_or(TIMEOUT_SECS, NonZeroU64::get);
        let exit = environment
            .exec(command, timeout)
            .await?
            .ok_or(ToolError::TimedOut(timeout))?;

        Ok(Outcome {
            error: exit.code != Some(0),
            ..Outcome::text(format_exit(&exit))
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
