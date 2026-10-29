use schemars::JsonSchema;
use serde::Deserialize;

use super::{Outcome, Tool, ToolError};
use crate::session::Session;

pub struct RunVm;

#[expect(
    clippy::empty_structs_with_brackets,
    reason = "tool arguments always arrive as a JSON object"
)]
#[derive(Deserialize, JsonSchema)]
pub struct Arguments {}

impl Tool for RunVm {
    const NAME: &'static str = "run_vm";
    const DESCRIPTION: &'static str = "Boot the VM that create_vm last built, without rebuilding \
                                       it. Use it to start the VM again after it was shut down.";

    type Arguments = Arguments;

    async fn run(&self, session: &Session, Arguments {}: Arguments) -> Result<Outcome, ToolError> {
        session.vm.run().await?;

        Ok(Outcome::text(
            "the VM is running and its display is visible to the user",
        ))
    }
}
