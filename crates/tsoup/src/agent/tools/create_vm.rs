use schemars::JsonSchema;
use serde::Deserialize;

use super::{Outcome, Tool, ToolError, booted};
use crate::session::{Sandbox, Session};

pub struct CreateVm;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(
        description = "The source of a NixOS module, e.g. `{ pkgs, ... }: { environment.systemPackages = [ pkgs.firefox ]; }`. Omit it to rebuild from the last module."
    )]
    #[serde(default)]
    module: Option<String>,
    #[schemars(
        description = "Optional modules to add to the VM by name, e.g. [\"graphics\"], pass [] for a plain headless VM. Omit it to keep the last selection."
    )]
    #[serde(default)]
    modules: Option<Vec<String>>,
}

impl Tool<Sandbox> for CreateVm {
    const NAME: &'static str = "create_vm";
    const DESCRIPTION: &'static str = "Build a microVM from a NixOS module and boot it,
replacing the running VM. /workspace is shared with the VM.";

    type Arguments = Arguments;

    async fn run(
        &self,
        _session: &Session,
        environment: &Sandbox,
        Arguments { module, modules }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let failures = environment.vm.create(module, modules).await?;
        Ok(booted(&failures))
    }
}
