use std::collections::HashSet;

use schemars::JsonSchema;
use serde::Deserialize;

use super::{Caller, Outcome, Scope, Tool, ToolError};
use crate::session::{Action, Sandbox};

pub struct SetActions;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "The buttons to show, replacing the current ones, [] removes them")]
    actions: Vec<Action>,
}

impl Tool<Sandbox> for SetActions {
    const NAME: &'static str = "set_actions";
    const DESCRIPTION: &'static str = "Give the user buttons that run a command in the VM,
e.g. Run to start the app, so they can do it themselves later without asking you.
They run detached like shell's background mode, while the VM runs.";

    type Arguments = Arguments;

    fn available(scope: Scope) -> bool {
        scope.depth == 0
    }

    async fn run(
        &self,
        caller: &Caller<'_>,
        _environment: &Sandbox,
        Arguments { actions }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let mut labels = HashSet::new();

        if let Some(action) = actions.iter().find(|action| !labels.insert(&action.label)) {
            return Err(ToolError::DuplicateAction(action.label.clone()));
        }

        caller.session.set_actions(actions).await?;
        Ok(Outcome::text("the user sees the actions"))
    }
}
