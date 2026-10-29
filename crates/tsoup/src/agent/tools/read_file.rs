use schemars::JsonSchema;
use serde::Deserialize;
use tsoup_proto::MAX_OUTPUT;

use super::workspace::{blocking, open, relative};
use super::{Outcome, Tool, ToolError};
use crate::session::Session;

pub struct ReadFile;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "A path inside /workspace")]
    path: String,
}

impl Tool for ReadFile {
    const NAME: &'static str = "read_file";
    const DESCRIPTION: &'static str = "Read a file from the workspace.";

    type Arguments = Arguments;

    async fn run(
        &self,
        session: &Session,
        Arguments { path }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let workspace = session.workspace();
        let content =
            blocking(move || Ok(open(&workspace)?.read_to_string(relative(&path))?)).await?;
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
