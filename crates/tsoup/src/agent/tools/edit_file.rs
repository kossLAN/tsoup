use schemars::JsonSchema;
use serde::Deserialize;

use super::{Caller, Outcome, Tool, ToolError};
use crate::workspace::Workspace;

pub struct EditFile;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "A path, relative paths resolve against the working directory")]
    path: String,
    #[schemars(description = "Text that occurs exactly once in the file")]
    old: String,
    #[schemars(description = "The text that replaces old")]
    new: String,
}

impl<E: Workspace> Tool<E> for EditFile {
    const NAME: &'static str = "edit_file";
    const DESCRIPTION: &'static str = "Replace one exact occurrence of a piece of text in a file.";

    type Arguments = Arguments;

    async fn run(
        &self,
        _caller: &Caller<'_>,
        environment: &E,
        Arguments { path, old, new }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let Some(before) = environment.read(&path).await? else {
            return Err(ToolError::NoFile(path));
        };

        match before.matches(&old).count() {
            0 => return Err(ToolError::NoMatch),
            1 => {},
            count => return Err(ToolError::AmbiguousMatch(count)),
        }

        let after = before.replacen(&old, &new, 1);
        let outcome = Outcome::changed(&path, &before, &after);
        environment.write(&path, after).await?;
        Ok(outcome)
    }
}
