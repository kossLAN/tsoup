use schemars::Schema;
use tsoup_proto::{McpTool, Request, Response};

use super::{Outcome, ToolError, arguments};
use crate::llm::{FunctionDefinition, ToolDefinition, ToolKind};
use crate::session::Session;

const PREFIX: &str = "mcp__";
const SEPARATOR: &str = "__";

pub fn definition(tool: &McpTool) -> Option<ToolDefinition> {
    let parameters = Schema::try_from(tool.input_schema.clone()).ok()?;

    Some(ToolDefinition {
        kind: ToolKind::Function,
        function: FunctionDefinition {
            name: format!("{PREFIX}{}{SEPARATOR}{}", tool.server, tool.name).into(),
            description: tool.description.clone().into(),
            parameters,
        },
    })
}

pub fn parse(name: &str) -> Option<(&str, &str)> {
    name.strip_prefix(PREFIX)?.split_once(SEPARATOR)
}

pub async fn call(
    session: &Session,
    server: &str,
    tool: &str,
    raw: &str,
) -> Result<Outcome, ToolError> {
    let request = Request::McpCall {
        server: server.to_owned(),
        tool: tool.to_owned(),
        arguments: arguments(raw)?,
    };

    match session.vm.request(request).await? {
        Response::McpOutput { output, error } => Ok(Outcome {
            output,
            error,
            diff: None,
        }),
        Response::Failed { message } => Err(ToolError::Guest(message)),
        _ => Err(ToolError::UnexpectedResponse),
    }
}
