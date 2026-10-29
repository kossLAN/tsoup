use schemars::JsonSchema;
use serde::Deserialize;

use super::{Outcome, Tool, ToolError};
use crate::session::Session;

pub struct CreateVm;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(
        description = "The source of a NixOS module, e.g. `{ pkgs, ... }: { environment.systemPackages = [ pkgs.firefox ]; }`. Omit it to rebuild from the last module."
    )]
    #[serde(default)]
    module: Option<String>,
}

impl Tool for CreateVm {
    const NAME: &'static str = "create_vm";
    const DESCRIPTION: &'static str = "Build a microVM from a NixOS module and boot it, replacing \
                                       the running VM. /workspace is shared with the VM.";

    type Arguments = Arguments;

    async fn run(
        &self,
        session: &Session,
        Arguments { module }: Arguments,
    ) -> Result<Outcome, ToolError> {
        session.vm.create(module).await?;

        Ok(Outcome::text(
            "the VM is running and its display is visible to the user",
        ))
    }
}
