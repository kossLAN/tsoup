use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rmcp::model::{CallToolRequestParams, ContentBlock, EmbeddedResource, ResourceContents};
use rmcp::service::{ClientInitializeError, RunningService, ServiceError};
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt as _};
use serde::Deserialize;
use serde_json::{Map, Value};
use tokio::process::Command;
use tsoup_proto::mcp::{Request, Response};
use tsoup_proto::{MAX_OUTPUT, McpFailure, McpImage, McpListing, McpOutput, McpTool};

const CALL_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, thiserror::Error)]
enum StartError {
    #[error("the command is empty")]
    EmptyCommand,
    #[error("starting the server: {0}")]
    Spawn(#[from] io::Error),
    #[error("initializing the server: {0}")]
    Initialize(Box<ClientInitializeError>),
}

#[derive(Debug, thiserror::Error)]
pub enum CallError {
    #[error("there is no MCP server named {0}")]
    NoServer(String),
    #[error("the MCP server failed: {0}")]
    Failed(#[from] ServiceError),
    #[error("the MCP call timed out after {} seconds", CALL_TIMEOUT.as_secs())]
    TimedOut,
}

#[derive(Deserialize)]
struct Server {
    command: Vec<String>,
}

#[derive(Default)]
pub struct Mcp {
    servers: HashMap<String, RunningService<RoleClient, ()>>,
    failures: Vec<McpFailure>,
}

impl Mcp {
    pub async fn start(config: &Path, dir: &Path) -> Self {
        let mut mcp = Self::default();

        let config = match load(config).await {
            Ok(config) => config,
            Err(error) => {
                tracing::error!(config = %config.display(), %error, "loading the MCP servers");
                return mcp;
            },
        };

        for (name, server) in config {
            match connect(&server.command, dir).await {
                Ok(service) => {
                    mcp.servers.insert(name, service);
                },
                Err(error) => {
                    tracing::warn!(server = name, %error, "starting an MCP server");
                    mcp.failures.push(McpFailure {
                        server: name,
                        message: error.to_string(),
                    });
                },
            }
        }

        mcp
    }

    pub async fn tools(&self) -> McpListing {
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

        McpListing { tools, failures }
    }

    pub async fn handle(&self, request: Request) -> Result<Response, CallError> {
        Ok(match request {
            Request::Tools => Response::Tools(self.tools().await),
            Request::Call {
                server,
                tool,
                arguments,
            } => Response::Output(self.call(&server, tool, arguments).await?),
        })
    }

    async fn call(
        &self,
        server: &str,
        tool: String,
        arguments: Map<String, Value>,
    ) -> Result<McpOutput, CallError> {
        let service = self
            .servers
            .get(server)
            .ok_or_else(|| CallError::NoServer(server.to_owned()))?;
        let call = service.call_tool(CallToolRequestParams::new(tool).with_arguments(arguments));
        let result = tokio::time::timeout(CALL_TIMEOUT, call)
            .await
            .map_err(|_| CallError::TimedOut)??;

        Ok(flatten(result.content, result.is_error.unwrap_or(false)))
    }
}

async fn load(config: &Path) -> io::Result<HashMap<String, Server>> {
    Ok(serde_json::from_slice(&tokio::fs::read(config).await?)?)
}

async fn connect(
    command: &[String],
    dir: &Path,
) -> Result<RunningService<RoleClient, ()>, StartError> {
    let (program, args) = command.split_first().ok_or(StartError::EmptyCommand)?;
    let mut process = Command::new(program);
    process.args(args).current_dir(dir);

    let transport = TokioChildProcess::new(process)?;
    ().serve(transport)
        .await
        .map_err(|error| StartError::Initialize(Box::new(error)))
}

fn flatten(content: Vec<ContentBlock>, error: bool) -> McpOutput {
    let mut text = Vec::new();
    let mut images = Vec::new();

    for block in content {
        match block {
            ContentBlock::Text(content) => text.push(content.text),
            ContentBlock::Image(image) => images.push(McpImage {
                media_type: image.mime_type,
                data: image.data,
            }),
            ContentBlock::Audio(audio) => text.push(format!("[audio: {}]", audio.mime_type)),
            ContentBlock::Resource(EmbeddedResource {
                resource: ResourceContents::TextResourceContents { text: content, .. },
                ..
            }) => text.push(content),
            ContentBlock::Resource(EmbeddedResource {
                resource:
                    ResourceContents::BlobResourceContents {
                        mime_type: Some(media_type),
                        blob,
                        ..
                    },
                ..
            }) if media_type.starts_with("image/") => images.push(McpImage {
                media_type,
                data: blob,
            }),
            ContentBlock::Resource(EmbeddedResource {
                resource: ResourceContents::BlobResourceContents { uri, .. },
                ..
            }) => text.push(format!("[resource: {uri}]")),
            ContentBlock::ResourceLink(resource) => {
                text.push(format!("[resource: {}]", resource.uri));
            },
            _ => text.push(String::from("[unsupported content]")),
        }
    }

    let mut output = text.join("\n");
    let end = (0..=output.len().min(MAX_OUTPUT))
        .rev()
        .find(|&index| output.is_char_boundary(index))
        .unwrap_or(0);
    output.truncate(end);

    McpOutput {
        output,
        images,
        error,
    }
}
