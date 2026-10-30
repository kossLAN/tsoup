use schemars::JsonSchema;
use serde::Deserialize;

use super::{Outcome, Tool, ToolError};
use crate::session::Session;

pub struct ShowWebsite;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(
        description = "The port the web server listens on inside the VM, it may listen on localhost only."
    )]
    port: u16,
}

impl Tool for ShowWebsite {
    const NAME: &'static str = "show_website";
    const DESCRIPTION: &'static str = "Show the user a website served from the running VM.
It appears in their preview pane, start its server in the background with shell first.";

    type Arguments = Arguments;

    async fn run(
        &self,
        session: &Session,
        Arguments { port }: Arguments,
    ) -> Result<Outcome, ToolError> {
        session.vm.show_website(port).await?;
        Ok(Outcome::text("the website is visible to the user"))
    }
}
