use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::ClaudeCodeError;
use crate::protocol::{RpcId, RpcRequest};

pub struct SdkServer<'a, S> {
    name: &'a str,
    prefix: String,
    tools: Vec<McpTool<'a, S>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool<'a, S> {
    pub name: &'a str,
    pub description: &'a str,
    pub input_schema: &'a S,
}

#[derive(Serialize)]
pub struct McpReply<'a, S> {
    mcp_response: RpcResponse<'a, S>,
}

#[derive(Serialize)]
struct RpcResponse<'a, S> {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<RpcId>,
    #[serde(flatten)]
    outcome: RpcOutcome<'a, S>,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum RpcOutcome<'a, S> {
    Result(RpcResult<'a, S>),
    Error { code: i64, message: String },
}

#[derive(Serialize)]
#[serde(untagged)]
enum RpcResult<'a, S> {
    Initialize(InitializeResult<'a>),
    Tools { tools: &'a [McpTool<'a, S>] },
    Empty(Map<String, Value>),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResult<'a> {
    protocol_version: Option<String>,
    capabilities: Capabilities,
    server_info: ServerInfo<'a>,
}

#[derive(Serialize)]
struct Capabilities {
    tools: ToolsCapability,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolsCapability {
    list_changed: bool,
}

#[derive(Serialize)]
struct ServerInfo<'a> {
    name: &'a str,
    version: &'static str,
}

#[derive(Serialize)]
struct McpConfig<'a> {
    #[serde(rename = "mcpServers")]
    mcp_servers: BTreeMap<&'a str, ServerConfig<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename = "sdk")]
struct ServerConfig<'a> {
    name: &'a str,
}

impl<'a, S: Serialize> SdkServer<'a, S> {
    pub fn new(name: &'a str, tools: Vec<McpTool<'a, S>>) -> Self {
        Self {
            name,
            prefix: format!("mcp__{name}__"),
            tools,
        }
    }

    pub const fn name(&self) -> &'a str {
        self.name
    }

    pub fn config(&self) -> Result<String, ClaudeCodeError> {
        Ok(serde_json::to_string(&McpConfig {
            mcp_servers: BTreeMap::from([(self.name, ServerConfig { name: self.name })]),
        })?)
    }

    pub fn tool_name(&self, name: String) -> String {
        name.strip_prefix(&self.prefix)
            .map(str::to_owned)
            .unwrap_or(name)
    }

    pub fn answer(&self, request: RpcRequest) -> McpReply<'_, S> {
        let RpcRequest { id, method, params } = request;
        let outcome = match method.as_deref() {
            Some("initialize") => RpcOutcome::Result(RpcResult::Initialize(InitializeResult {
                protocol_version: params.protocol_version,
                capabilities: Capabilities {
                    tools: ToolsCapability {
                        list_changed: false,
                    },
                },
                server_info: ServerInfo {
                    name: self.name,
                    version: env!("CARGO_PKG_VERSION"),
                },
            })),
            Some("tools/list") => RpcOutcome::Result(RpcResult::Tools { tools: &self.tools }),
            Some(method) if id.is_some() => RpcOutcome::Error {
                code: -32601,
                message: format!("unknown method {method}"),
            },
            _ => RpcOutcome::Result(RpcResult::Empty(Map::new())),
        };

        McpReply {
            mcp_response: RpcResponse {
                jsonrpc: "2.0",
                id,
                outcome,
            },
        }
    }
}
