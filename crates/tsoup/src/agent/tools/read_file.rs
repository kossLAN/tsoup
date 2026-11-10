use schemars::JsonSchema;
use serde::Deserialize;

use super::{Caller, Outcome, Tool, ToolError, head};
use crate::workspace::Workspace;

pub struct ReadFile;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "A path, relative paths resolve against the working directory")]
    path: String,
}

impl<E: Workspace> Tool<E> for ReadFile {
    const NAME: &'static str = "read_file";
    const DESCRIPTION: &'static str = "Read a file.";

    type Arguments = Arguments;

    async fn run(
        &self,
        _caller: &Caller<'_>,
        environment: &E,
        Arguments { path }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let content = environment
            .read(&path)
            .await?
            .ok_or(ToolError::NoFile(path))?;
        Ok(Outcome::text(head(content)))
    }
}
