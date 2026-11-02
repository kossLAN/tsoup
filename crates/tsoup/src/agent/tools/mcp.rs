use schemars::Schema;
use tsoup_proto::{McpOutput, McpTool};

use super::{Outcome, ToolError, arguments};
use crate::llm::ToolDefinition;
use crate::workspace::Workspace;

const PREFIX: &str = "mcp__";
const SEPARATOR: &str = "__";

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
    let McpOutput { output, error } = environment
        .mcp_call(server.to_owned(), tool.to_owned(), arguments(raw)?)
        .await?;

    Ok(Outcome {
        output,
        error,
        diff: None,
    })
}
