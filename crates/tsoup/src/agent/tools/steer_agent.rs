use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use super::{Caller, Outcome, Scope, Tool, ToolError};
use crate::session::Entry;
use crate::workspace::Workspace;

pub struct SteerAgent;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(
        description = "The id of one of your running subagents, from spawn_agent or list_agents"
    )]
    agent: Uuid,
    #[schemars(
        description = "The message for the subagent. It remembers its conversation so far, so only say what is new."
    )]
    prompt: String,
}

impl<E: Workspace> Tool<E> for SteerAgent {
    const NAME: &'static str = "steer_agent";
    const DESCRIPTION: &'static str =
        "Send a message to one of your running subagents, which reads it before its next step and keeps working.
Use it to correct a subagent that is going the wrong way, to narrow or widen its task
or to pass on what you learned after it started, instead of waiting for it to finish.
It only reaches a subagent that is still running, resume_agent continues one that has finished.";
    const PARALLEL: bool = true;

    type Arguments = Arguments;

    fn available(scope: Scope) -> bool {
        scope.depth < scope.limit
    }

    async fn run(
        &self,
        caller: &Caller<'_>,
        _environment: &E,
        Arguments { agent, prompt }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let session = caller.session;
        let mine = session.subagents(caller.id).await;

        if !mine.iter().any(|summary| summary.id == agent) {
            return Err(ToolError::NotSubagent(agent));
        }

        let prompt = Entry::User {
            text: prompt,
            images: Vec::new(),
        };
        session.steer(agent, prompt).await?;
        Ok(Outcome::text(format!(
            "agent {agent} reads the message before its next step"
        )))
    }
}
