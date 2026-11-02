use futures_util::future::BoxFuture;
use reqwest::{Method, RequestBuilder};
use schemars::Schema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{
    Effort, Endpoint, FunctionCall, LlmError, Message, Provider, Reply, ToolCall, ToolDefinition,
    ToolKind, joined, list_models, send,
};

const VERSION: &str = "2026-10-21";
const MAX_TOKENS: u32 = 16_000;

pub struct Anthropic(pub Endpoint);

impl Anthropic {
    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let request = self
            .0
            .http
            .request(method, format!("{}{path}", self.0.url))
            .header("anthropic-version", VERSION);

        match &self.0.key {
            Some(key) => request.header("x-api-key", key),
            None => request,
        }
    }
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_config: Option<OutputConfig>,
    #[serde(skip_serializing_if = "String::is_empty")]
    system: String,
    messages: Vec<Turn<'a>>,
    tools: Vec<Tool<'a>>,
}

#[derive(Serialize)]
struct OutputConfig {
    effort: Effort,
}

#[derive(Serialize)]
struct Turn<'a> {
    role: Role,
    content: Vec<Block<'a>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    User,
    Assistant,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Block<'a> {
    Text {
        text: &'a str,
    },
    ToolUse {
        id: &'a str,
        name: &'a str,
        input: Value,
    },
    ToolResult {
        tool_use_id: &'a str,
        content: &'a str,
    },
}

#[derive(Serialize)]
struct Tool<'a> {
    name: &'a str,
    description: &'a str,
    input_schema: &'a Schema,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<StopReason>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(other)]
    Other,
}

#[derive(PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StopReason {
    Refusal,
    #[serde(other)]
    Other,
}

fn system(messages: &[Message]) -> String {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::System { content } => Some(content.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn turns(messages: &[Message]) -> Vec<Turn<'_>> {
    let mut turns: Vec<Turn<'_>> = Vec::new();

    for turn in messages.iter().filter_map(Turn::new) {
        match turns.last_mut() {
            Some(last) if last.role == turn.role => last.content.extend(turn.content),
            _ => turns.push(turn),
        }
    }

    turns
}

impl<'a> Turn<'a> {
    fn new(message: &'a Message) -> Option<Self> {
        let (role, content) = match message {
            Message::System { .. } => return None,
            Message::User { content } => (Role::User, vec![Block::Text { text: content }]),
            Message::Assistant {
                content,
                tool_calls,
            } => (
                Role::Assistant,
                content
                    .iter()
                    .map(|text| Block::Text { text })
                    .chain(tool_calls.iter().map(Block::from))
                    .collect(),
            ),
            Message::Tool {
                tool_call_id,
                content,
            } => (
                Role::User,
                vec![Block::ToolResult {
                    tool_use_id: tool_call_id,
                    content,
                }],
            ),
        };

        (!content.is_empty()).then_some(Self { role, content })
    }
}

impl<'a> From<&'a ToolCall> for Block<'a> {
    fn from(call: &'a ToolCall) -> Self {
        Self::ToolUse {
            id: &call.id,
            name: &call.function.name,
            input: serde_json::from_str(&call.function.arguments)
                .ok()
                .filter(Value::is_object)
                .unwrap_or_else(|| Value::Object(Map::new())),
        }
    }
}

impl<'a> From<&'a ToolDefinition> for Tool<'a> {
    fn from(tool: &'a ToolDefinition) -> Self {
        Self {
            name: &tool.name,
            description: &tool.description,
            input_schema: &tool.parameters,
        }
    }
}

impl TryFrom<MessagesResponse> for Reply {
    type Error = LlmError;

    fn try_from(response: MessagesResponse) -> Result<Self, Self::Error> {
        if response.stop_reason == Some(StopReason::Refusal) {
            return Err(LlmError::Refused);
        }

        let mut parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in response.content {
            match block {
                ContentBlock::Text { text } => parts.push(text),
                ContentBlock::ToolUse { id, name, input } => tool_calls.push(ToolCall {
                    id,
                    kind: ToolKind::Function,
                    function: FunctionCall {
                        name,
                        arguments: input.to_string(),
                    },
                }),
                ContentBlock::Other => {},
            }
        }

        Ok(Self {
            content: joined(parts),
            tool_calls,
        })
    }
}

impl Provider for Anthropic {
    fn complete<'a>(
        &'a self,
        model: &'a str,
        effort: Option<Effort>,
        messages: &'a [Message],
        tools: &'a [ToolDefinition],
    ) -> BoxFuture<'a, Result<Reply, LlmError>> {
        Box::pin(async move {
            let request = self
                .request(Method::POST, "/v1/messages")
                .json(&MessagesRequest {
                    model,
                    max_tokens: MAX_TOKENS,
                    output_config: effort.map(|effort| OutputConfig { effort }),
                    system: system(messages),
                    messages: turns(messages),
                    tools: tools.iter().map(Tool::from).collect(),
                });
            let response: MessagesResponse = send(request).await?.json().await?;
            response.try_into()
        })
    }

    fn models(&self) -> BoxFuture<'_, Result<Vec<String>, LlmError>> {
        Box::pin(list_models(
            self.request(Method::GET, "/v1/models?limit=1000"),
        ))
    }
}
