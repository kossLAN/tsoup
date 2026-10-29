use std::io;

use schemars::JsonSchema;
use serde::Deserialize;

use super::workspace::{blocking, changed, open, relative};
use super::{Outcome, Tool, ToolError};
use crate::session::Session;

pub struct WriteFile;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "A path inside /workspace")]
    path: String,
    #[schemars(description = "The complete new contents of the file")]
    content: String,
}

impl Tool for WriteFile {
    const NAME: &'static str = "write_file";
    const DESCRIPTION: &'static str = "Create or overwrite a file in the workspace.";

    type Arguments = Arguments;

    async fn run(
        &self,
        session: &Session,
        Arguments { path, content }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let workspace = session.workspace();
        let target = path.clone();
        let (before, after) = blocking(move || {
            let dir = open(&workspace)?;
            let file = relative(&target);

            let before = match dir.read_to_string(file) {
                Ok(before) => before,
                Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
                Err(error) => return Err(error.into()),
            };

            if let Some(parent) = file
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                dir.create_dir_all(parent)?;
            }

            dir.write(file, &content)?;
            Ok((before, content))
        })
        .await?;
        Ok(changed(path, &before, &after))
    }
}
