use schemars::JsonSchema;
use serde::Deserialize;

use super::{Caller, Outcome, Scope, Tool, ToolError};
use crate::workspace::Workspace;

pub struct SpawnAgent;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "A few words naming the task, shown to the user")]
    name: String,
    #[schemars(
        description = "The complete task. The subagent sees nothing of this conversation, so include all the context it needs and say what its final report should contain."
    )]
    prompt: String,
    #[schemars(
        description = "One of the models the system prompt lists for this argument, omit it to use the default"
    )]
    #[serde(default)]
    model: Option<String>,
}

impl<E: Workspace> Tool<E> for SpawnAgent {
    const NAME: &'static str = "spawn_agent";
    const DESCRIPTION: &'static str = "Hand a task to a subagent and wait for its final report.
The subagent works in the same workspace with the same tools.
Call it several times in one message to run subagents in parallel,
for independent pieces of work or to keep long explorations out of your context.
The report names the subagent's id, which resume_agent takes to continue its work.";
    const PARALLEL: bool = true;

    type Arguments = Arguments;

    fn available(scope: Scope) -> bool {
        scope.depth < scope.limit
    }

    async fn run(
        &self,
        caller: &Caller<'_>,
        _environment: &E,
        Arguments {
            name,
            prompt,
            model,
        }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let session = caller.session;
        let settings = caller.agent.settings.get().await;
        let model = match model {
            Some(label) => match settings.subagent_model(&label) {
                Some(model) => Some(model.clone()),
                None => return Err(ToolError::UnknownModel(label)),
            },
            None => match settings.subagents.model {
                Some(model) => Some(model),
                None => session.model(caller.id).await?,
            },
        };
        let model = match model {
            Some(model) => Some(model),
            None => session.selection().await.model.or(settings.default_model),
        };
        let (id, stop) = session.spawn_agent(caller.id, name, model).await?;
        caller.delegate(id, stop, prompt).await
    }
}
