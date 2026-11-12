use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use super::{Caller, Outcome, Scope, Tool, ToolError};
use crate::workspace::Workspace;

pub struct ResumeAgent;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "The id of one of your subagents, from its report or list_agents")]
    agent: Uuid,
    #[schemars(
        description = "The follow-up task. The subagent remembers its conversation so far, so only say what is new."
    )]
    prompt: String,
}

impl<E: Workspace> Tool<E> for ResumeAgent {
    const NAME: &'static str = "resume_agent";
    const DESCRIPTION: &'static str =
        "Restart one of your subagents in the background with a follow-up task.
Its new report arrives as a message when it finishes, like spawn_agent's.
The subagent keeps its conversation, so use it to continue, correct or retry earlier work
instead of explaining everything again to a new subagent.
It works on subagents that finished, failed or were stopped by the user.";
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
        let stop = caller.session.resume_agent(caller.id, agent).await?;
        Ok(caller.delegate(agent, stop, prompt).await)
    }
}
