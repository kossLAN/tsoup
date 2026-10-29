use schemars::JsonSchema;
use serde::Deserialize;

use super::{Outcome, Tool, ToolError};
use crate::session::Session;

pub struct DestroyVm;

#[expect(
    clippy::empty_structs_with_brackets,
    reason = "tool arguments always arrive as a JSON object"
)]
#[derive(Deserialize, JsonSchema)]
pub struct Arguments {}

impl Tool for DestroyVm {
    const NAME: &'static str = "destroy_vm";
    const DESCRIPTION: &'static str = "Shut the VM down.";

    type Arguments = Arguments;

    async fn run(&self, session: &Session, Arguments {}: Arguments) -> Result<Outcome, ToolError> {
        session.vm.destroy().await;
        Ok(Outcome::text("the VM was destroyed"))
    }
}
