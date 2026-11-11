use schemars::JsonSchema;
use serde::Deserialize;

use super::{Caller, Outcome, Scope, Tool, ToolError};
use crate::session::AgentSummary;
use crate::workspace::Workspace;

pub struct ListAgents;

#[expect(
    clippy::empty_structs_with_brackets,
    reason = "tool arguments always arrive as a JSON object"
)]
#[derive(Deserialize, JsonSchema)]
pub struct Arguments {}

impl<E: Workspace> Tool<E> for ListAgents {
    const NAME: &'static str = "list_agents";
    const DESCRIPTION: &'static str =
        "List the subagents you have spawned, with their ids, states and names.";

    type Arguments = Arguments;

    fn available(scope: Scope) -> bool {
        scope.depth < scope.limit
    }

    async fn run(
        &self,
        caller: &Caller<'_>,
        _environment: &E,
        Arguments {}: Arguments,
    ) -> Result<Outcome, ToolError> {
        let agents = caller.session.subagents(caller.id).await;

        if agents.is_empty() {
            return Ok(Outcome::text("you have not spawned any subagents"));
        }

        let lines: Vec<_> = agents
            .iter()
            .map(
                |AgentSummary {
                     id, state, name, ..
                 }| format!("{id} {}: {name}", state.label()),
            )
            .collect();
        Ok(Outcome::text(lines.join("\n")))
    }
}
