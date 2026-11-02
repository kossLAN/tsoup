use schemars::JsonSchema;
use serde::Deserialize;
use tsoup_proto::MAX_OUTPUT;

use super::{Outcome, Tool, ToolError};
use crate::session::Session;
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
        _session: &Session,
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

fn head(mut content: String) -> String {
    if content.len() > MAX_OUTPUT {
        let end = (0..=MAX_OUTPUT)
            .rev()
            .find(|&index| content.is_char_boundary(index))
            .unwrap_or(0);
        content.truncate(end);
        content.push_str("\n[truncated]");
    }

    content
}
