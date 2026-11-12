use schemars::Schema;
use tsoup_proto::{McpImage, McpOutput, McpTool};

use super::{Outcome, ToolError, arguments};
use crate::llm::{Image, ToolDefinition};
use crate::workspace::Workspace;

const PREFIX: &str = "mcp__";
const SEPARATOR: &str = "__";
// Anthropic rejects images over 5 MiB, and a rejected image fails every later turn.
const MAX_IMAGE: usize = 5 * 1024 * 1024;

pub fn definition(tool: &McpTool) -> Option<ToolDefinition> {
    let parameters = Schema::try_from(tool.input_schema.clone()).ok()?;

    Some(ToolDefinition {
        name: format!("{PREFIX}{}{SEPARATOR}{}", tool.server, tool.name).into(),
        description: tool.description.clone().into(),
        parameters,
    })
}

pub fn parse(name: &str) -> Option<(&str, &str)> {
    name.strip_prefix(PREFIX)?.split_once(SEPARATOR)
}

pub async fn call<E>(
    environment: &E,
    server: &str,
    tool: &str,
    raw: &str,
) -> Result<Outcome, ToolError>
where
    E: Workspace,
{
    let McpOutput {
        mut output,
        images: returned,
        error,
    } = environment
        .mcp_call(server.to_owned(), tool.to_owned(), arguments(raw)?)
        .await?;
    let mut images = Vec::new();

    for McpImage { media_type, data } in returned {
        let image = Image { media_type, data };

        let skipped = if !image.supported() {
            "is not a supported format"
        } else if image.data.len() > MAX_IMAGE {
            "is too large to show the model"
        } else {
            images.push(image);
            continue;
        };

        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&format!("[{} image {skipped}]", image.media_type));
    }

    Ok(Outcome {
        output,
        images,
        error,
        diff: None,
    })
}
