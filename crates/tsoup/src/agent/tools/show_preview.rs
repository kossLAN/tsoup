use schemars::JsonSchema;
use serde::Deserialize;

use super::{Outcome, Tool, ToolError};
use crate::session::Session;

pub struct ShowPreview;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "The preview a module provides, e.g. website.")]
    preview: String,
    #[schemars(
        description = "The port the server listens on inside the VM, it may listen on localhost only."
    )]
    port: u16,
}

impl Tool for ShowPreview {
    const NAME: &'static str = "show_preview";
    const DESCRIPTION: &'static str = "Point a preview at a server running in the VM.
It appears in the user's preview pane, start the server in the background with shell first.";

    type Arguments = Arguments;

    async fn run(
        &self,
        session: &Session,
        Arguments { preview, port }: Arguments,
    ) -> Result<Outcome, ToolError> {
        session.vm.show_preview(&preview, port).await?;
        Ok(Outcome::text(format!(
            "the {preview} preview is visible to the user"
        )))
    }
}
