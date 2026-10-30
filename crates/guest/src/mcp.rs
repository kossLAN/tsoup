use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use rmcp::model::{CallToolRequestParams, ContentBlock, EmbeddedResource, ResourceContents};
use rmcp::service::{ClientInitializeError, RunningService};
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt as _};
use serde::Deserialize;
use serde_json::{Map, Value};
use tokio::process::Command;
use tsoup_proto::{MAX_OUTPUT, McpFailure, McpTool, Response, WORKSPACE};

const CONFIG: &str = "/etc/tsoup/mcp.json";
const CALL_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, thiserror::Error)]
enum McpError {
    #[error("reading {CONFIG}: {0}")]
    Config(io::Error),
    #[error("parsing {CONFIG}: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("the command is empty")]
    EmptyCommand,
    #[error("starting the server: {0}")]
    Spawn(#[from] io::Error),
    #[error("initializing the server: {0}")]
    Initialize(Box<ClientInitializeError>),
}

#[derive(Deserialize)]
struct Server {
    command: Vec<String>,
}

pub struct Mcp {
    servers: HashMap<String, RunningService<RoleClient, ()>>,
    failures: Vec<McpFailure>,
}

impl Mcp {
    pub async fn start() -> Self {
        let mut mcp = Self {
            servers: HashMap::new(),
            failures: Vec::new(),
        };

        let config = match config().await {
            Ok(config) => config,
            Err(error) => {
                tracing::error!(%error, "loading the MCP servers");
                return mcp;
            },
        };

        for (name, server) in config {
            match connect(&server.command).await {
                Ok(service) => {
                    mcp.servers.insert(name, service);
                },
                Err(error) => mcp.failures.push(McpFailure {
                    server: name,
                    message: error.to_string(),
                }),
            }
        }

        mcp
    }

    pub async fn tools(&self) -> Response {
        let mut tools = Vec::new();
        let mut failures = self.failures.clone();

        for (server, service) in &self.servers {
            match service.list_all_tools().await {
                Ok(listed) => tools.extend(listed.into_iter().map(|tool| McpTool {
                    server: server.clone(),
                    name: tool.name.into_owned(),
                    description: tool.description.unwrap_or_default().into_owned(),
                    input_schema: Value::Object(Arc::unwrap_or_clone(tool.input_schema)),
                })),
                Err(error) => failures.push(McpFailure {
                    server: server.clone(),
                    message: error.to_string(),
                }),
            }
        }

        Response::McpTools { tools, failures }
    }

    pub async fn call(
        &self,
        server: &str,
        tool: String,
        arguments: Map<String, Value>,
    ) -> Response {
        let Some(service) = self.servers.get(server) else {
            return Response::Failed {
                message: format!("there is no MCP server named {server}"),
            };
        };

        let call = service.call_tool(CallToolRequestParams::new(tool).with_arguments(arguments));

        match tokio::time::timeout(CALL_TIMEOUT, call).await {
            Ok(Ok(result)) => Response::McpOutput {
                output: flatten(&result.content),
                error: result.is_error.unwrap_or(false),
            },
            Ok(Err(error)) => Response::Failed {
                message: format!("the MCP server failed: {error}"),
            },
            Err(_) => Response::Failed {
                message: format!(
                    "the MCP call timed out after {} seconds",
                    CALL_TIMEOUT.as_secs()
                ),
            },
        }
    }
}

async fn config() -> Result<HashMap<String, Server>, McpError> {
    let config = tokio::fs::read(CONFIG).await.map_err(McpError::Config)?;
    Ok(serde_json::from_slice(&config)?)
}

async fn connect(command: &[String]) -> Result<RunningService<RoleClient, ()>, McpError> {
    let (program, args) = command.split_first().ok_or(McpError::EmptyCommand)?;
    let mut process = Command::new(program);
    process.args(args).current_dir(WORKSPACE);

    let transport = TokioChildProcess::new(process)?;
    ().serve(transport)
        .await
        .map_err(|error| McpError::Initialize(Box::new(error)))
}

fn flatten(content: &[ContentBlock]) -> String {
    let mut output = content
        .iter()
        .map(|block| match block {
            ContentBlock::Text(text) => text.text.clone(),
            ContentBlock::Image(image) => format!("[image: {}]", image.mime_type),
            ContentBlock::Audio(audio) => format!("[audio: {}]", audio.mime_type),
            ContentBlock::Resource(EmbeddedResource {
                resource: ResourceContents::TextResourceContents { text, .. },
                ..
            }) => text.clone(),
            ContentBlock::Resource(EmbeddedResource {
                resource: ResourceContents::BlobResourceContents { uri, .. },
                ..
            }) => format!("[resource: {uri}]"),
            ContentBlock::ResourceLink(resource) => format!("[resource: {}]", resource.uri),
            _ => String::from("[unsupported content]"),
        })
        .collect::<Vec<_>>()
        .join("\n");

    let end = (0..=output.len().min(MAX_OUTPUT))
        .rev()
        .find(|&index| output.is_char_boundary(index))
        .unwrap_or(0);
    output.truncate(end);
    output
}
