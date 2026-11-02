use schemars::JsonSchema;
use serde::Deserialize;

use super::{Outcome, Tool, ToolError};
use crate::session::Session;
use crate::workspace::Workspace;

pub struct WriteFile;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "A path, relative paths resolve against the working directory")]
    path: String,
    #[schemars(description = "The complete new contents of the file")]
    content: String,
}

impl<E: Workspace> Tool<E> for WriteFile {
    const NAME: &'static str = "write_file";
    const DESCRIPTION: &'static str = "Create or overwrite a file.";

    type Arguments = Arguments;

    async fn run(
        &self,
        _session: &Session,
        environment: &E,
        Arguments { path, content }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let before = environment.read(&path).await?.unwrap_or_default();
        let outcome = Outcome::changed(&path, &before, &content);
        environment.write(&path, content).await?;
        Ok(outcome)
    }
}
