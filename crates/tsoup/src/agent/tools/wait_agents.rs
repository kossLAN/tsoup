use std::num::NonZeroU64;
use std::time::Duration;

use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use super::{Caller, Outcome, Scope, Tool, ToolError};
use crate::agent::subagents::describe;
use crate::workspace::Workspace;

pub struct WaitAgents;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(
        description = "The ids of the subagents to wait for, omit it to wait for all of your running subagents"
    )]
    #[serde(default)]
    agents: Vec<Uuid>,
    #[schemars(
        description = "Seconds to wait at most, omit it to wait until they finish. Subagents still running when it runs out keep working, so use it to check on them or steer them in between."
    )]
    #[serde(default)]
    timeout: Option<NonZeroU64>,
}

impl<E: Workspace> Tool<E> for WaitAgents {
    const NAME: &'static str = "wait_agents";
    const DESCRIPTION: &'static str =
        "Wait until your running subagents finish and return their reports.
Call it after spawn_agent or resume_agent in the same message to start subagents and wait for them in one step.
It stops waiting early when a message arrives for you or the timeout runs out.";

    type Arguments = Arguments;

    fn available(scope: Scope) -> bool {
        scope.depth < scope.limit
    }

    async fn run(
        &self,
        caller: &Caller<'_>,
        _environment: &E,
        Arguments { agents, timeout }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let mine = caller.session.subagents(caller.id).await;

        if let Some(&stranger) = agents
            .iter()
            .find(|&&agent| !mine.iter().any(|summary| summary.id == agent))
        {
            return Err(ToolError::NotSubagent(stranger));
        }

        let waiting = caller.subagents.wait(caller.session, caller.id, &agents);
        let expired = match timeout {
            Some(timeout) => {
                let limit = Duration::from_secs(timeout.get());

                match tokio::time::timeout(limit, waiting).await {
                    Ok(waited) => waited.map(|()| None)?,
                    Err(_) => Some(timeout),
                }
            },
            None => waiting.await.map(|()| None)?,
        };
        let (reports, running) = caller.subagents.take(&agents).await;

        let mut sections: Vec<_> = reports
            .iter()
            .map(|report| describe(report.agent, report.state, report.text.as_deref()))
            .collect();

        if !running.is_empty() {
            let reason = expired.map_or_else(
                || "a message arrived for you".to_owned(),
                |timeout| format!("the {timeout} seconds ran out"),
            );
            let running: Vec<_> = running.iter().map(Uuid::to_string).collect();
            sections.push(format!(
                "stopped waiting because {reason}, still running: {}",
                running.join(", ")
            ));
        }

        if sections.is_empty() {
            let which = if agents.is_empty() { "your" } else { "those" };
            sections.push(format!(
                "none of {which} subagents are running, their reports came earlier"
            ));
        }

        Ok(Outcome::text(sections.join("\n\n")))
    }
}
