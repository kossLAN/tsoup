use schemars::JsonSchema;
use serde::Deserialize;

use super::{Caller, Outcome, Scope, Tool, ToolError, booted};
use crate::session::Sandbox;

pub struct RunVm;

#[expect(
    clippy::empty_structs_with_brackets,
    reason = "tool arguments always arrive as a JSON object"
)]
#[derive(Deserialize, JsonSchema)]
pub struct Arguments {}

impl Tool<Sandbox> for RunVm {
    const NAME: &'static str = "run_vm";
    const DESCRIPTION: &'static str = "Boot the VM create_vm last built, without rebuilding it.
Use it to start the VM again after it was shut down.";

    type Arguments = Arguments;

    fn available(scope: Scope) -> bool {
        scope.depth == 0
    }

    async fn run(
        &self,
        _caller: &Caller<'_>,
        environment: &Sandbox,
        Arguments {}: Arguments,
    ) -> Result<Outcome, ToolError> {
        let failures = environment.vm.run().await?;
        Ok(booted(&failures))
    }
}
