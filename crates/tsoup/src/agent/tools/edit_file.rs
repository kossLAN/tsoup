use schemars::JsonSchema;
use serde::Deserialize;

use super::workspace::{blocking, changed, open, relative};
use super::{Outcome, Tool, ToolError};
use crate::session::Session;

pub struct EditFile;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "A path inside /workspace")]
    path: String,
    #[schemars(description = "Text that occurs exactly once in the file")]
    old: String,
    #[schemars(description = "The text that replaces old")]
    new: String,
}

impl Tool for EditFile {
    const NAME: &'static str = "edit_file";
    const DESCRIPTION: &'static str =
        "Replace one exact occurrence of a piece of text in a workspace file.";

    type Arguments = Arguments;

    async fn run(
        &self,
        session: &Session,
        Arguments { path, old, new }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let workspace = session.workspace();
        let target = path.clone();
        let (before, after) = blocking(move || {
            let dir = open(&workspace)?;
            let file = relative(&target);
            let before = dir.read_to_string(file)?;

            match before.matches(&old).count() {
                0 => return Err(ToolError::NoMatch),
                1 => {},
                count => return Err(ToolError::AmbiguousMatch(count)),
            }

            let after = before.replacen(&old, &new, 1);
            dir.write(file, &after)?;
            Ok((before, after))
        })
        .await?;
        Ok(changed(path, &before, &after))
    }
}
