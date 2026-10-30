use schemars::JsonSchema;
use serde::Deserialize;

use super::{Outcome, Tool, ToolError, booted};
use crate::session::Session;
use crate::vm::VmKind;

pub struct CreateVm;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(
        description = "The source of a NixOS module, e.g. `{ pkgs, ... }: { environment.systemPackages = [ pkgs.firefox ]; }`. Omit it to rebuild from the last module."
    )]
    #[serde(default)]
    module: Option<String>,
    #[schemars(
        description = "headless suits command line programs and web apps, graphical adds a KWin Wayland desktop the user watches live. Omit it to keep the last kind."
    )]
    #[serde(default)]
    kind: Option<VmKind>,
}

impl Tool for CreateVm {
    const NAME: &'static str = "create_vm";
    const DESCRIPTION: &'static str = "Build a microVM from a NixOS module and boot it,
replacing the running VM. /workspace is shared with the VM.";

    type Arguments = Arguments;

    async fn run(
        &self,
        session: &Session,
        Arguments { module, kind }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let failures = session.vm.create(module, kind).await?;
        Ok(booted(&failures))
    }
}
