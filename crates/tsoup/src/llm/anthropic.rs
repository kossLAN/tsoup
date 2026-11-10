use std::ops::ControlFlow;

use futures_util::future::BoxFuture;
use reqwest::{Method, RequestBuilder};
use schemars::Schema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::stream::{Collector, Data, Events};
use super::{
    Effort, Endpoint, Image, LlmError, Message, Model, OnProgress, Provider, Reply, ToolCall,
    ToolDefinition, list_models, send, system,
};

const VERSION: &str = "2026-10-21";
const MAX_TOKENS: u32 = 64_000;

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
    thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_config: Option<OutputConfig>,
    #[serde(skip_serializing_if = "String::is_empty")]
    system: String,
    messages: Vec<Turn<'a>>,
    tools: Vec<Tool<'a>>,
    stream: bool,
}

#[derive(Serialize)]
#[serde(tag = "type", rename = "adaptive")]
struct Thinking {
    display: Display,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Display {
    Summarized,
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
    Image {
        source: ImageSource<'a>,
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
#[serde(tag = "type", rename = "base64")]
pub(super) struct ImageSource<'a> {
    media_type: &'a str,
    data: &'a str,
}

impl<'a> From<&'a Image> for ImageSource<'a> {
    fn from(image: &'a Image) -> Self {
        Self {
            media_type: &image.media_type,
            data: &image.data,
        }
    }
}

#[derive(Serialize)]
struct Tool<'a> {
    name: &'a str,
    description: &'a str,
    input_schema: &'a Schema,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum StreamEvent {
    MessageStart {
        message: StartedMessage,
    },
    ContentBlockStart {
        index: usize,
        content_block: StartedBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: BlockDelta,
    },
    MessageDelta {
        delta: MessageDelta,
        usage: Option<TokenUsage>,
    },
    MessageStop,
    Error {
        error: StreamError,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct StartedMessage {
    usage: Option<TokenUsage>,
}

#[derive(Deserialize)]
#[expect(clippy::struct_field_names, reason = "the fields are named by the API")]
pub(super) struct TokenUsage {
    input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

impl TokenUsage {
    fn apply(&self, collector: &mut Collector<'_>) {
        let input = self.input_tokens.map(|tokens| {
            tokens
                + self.cache_creation_input_tokens.unwrap_or(0)
                + self.cache_read_input_tokens.unwrap_or(0)
        });
        collector.usage(input, self.output_tokens);
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum StartedBlock {
    Text,
    Thinking,
    ToolUse {
        id: String,
        name: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum BlockDelta {
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        thinking: String,
    },
    InputJsonDelta {
        partial_json: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct MessageDelta {
    stop_reason: Option<StopReason>,
}

#[derive(PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StopReason {
    Refusal,
    ModelContextWindowExceeded,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct StreamError {
    message: String,
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
            Message::User { content, images } => (
                Role::User,
                images
                    .iter()
                    .map(|image| Block::Image {
                        source: image.into(),
                    })
                    .chain((!content.is_empty()).then_some(Block::Text { text: content }))
                    .collect(),
            ),
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

impl StreamEvent {
    pub(super) fn apply<F>(
        self,
        collector: &mut Collector<'_>,
        rename: F,
    ) -> Result<ControlFlow<()>, LlmError>
    where
        F: FnOnce(String) -> String,
    {
        match self {
            Self::MessageStart { message } => {
                if let Some(usage) = message.usage {
                    usage.apply(collector);
                }
            },
            Self::ContentBlockStart {
                index,
                content_block,
            } => match content_block {
                StartedBlock::Text => collector.start_text(),
                StartedBlock::Thinking => collector.start_thinking(),
                StartedBlock::ToolUse { id, name } => collector.tool_call(index, id, rename(name)),
                StartedBlock::Other => {},
            },
            Self::ContentBlockDelta { index, delta } => match delta {
                BlockDelta::TextDelta { text } => collector.text(&text),
                BlockDelta::ThinkingDelta { thinking } => collector.thinking(&thinking),
                BlockDelta::InputJsonDelta { partial_json } => {
                    collector.tool_arguments(index, &partial_json);
                },
                BlockDelta::Other => {},
            },
            Self::MessageDelta { delta, usage } => {
                match delta.stop_reason {
                    Some(StopReason::Refusal) => return Err(LlmError::Refused),
                    Some(StopReason::ModelContextWindowExceeded) => {
                        return Err(LlmError::Overflow);
                    },
                    Some(StopReason::Other) | None => {},
                }

                if let Some(usage) = usage {
                    usage.apply(collector);
                }
            },
            Self::MessageStop => return Ok(ControlFlow::Break(())),
            Self::Error { error } => return Err(LlmError::remote(error.message)),
            Self::Other => {},
        }

        Ok(ControlFlow::Continue(()))
    }
}

impl Provider for Anthropic {
    fn complete<'a>(
        &'a self,
        model: &'a str,
        effort: Option<Effort>,
        messages: &'a [Message],
        tools: &'a [ToolDefinition],
        progress: &'a OnProgress<'a>,
    ) -> BoxFuture<'a, Result<Reply, LlmError>> {
        Box::pin(async move {
            let request = self
                .request(Method::POST, "/v1/messages")
                .json(&MessagesRequest {
                    model,
                    max_tokens: MAX_TOKENS,
                    thinking: effort.map(|_| Thinking {
                        display: Display::Summarized,
                    }),
                    output_config: effort.map(|effort| OutputConfig { effort }),
                    system: system(messages),
                    messages: turns(messages),
                    tools: tools.iter().map(Tool::from).collect(),
                    stream: true,
                });
            let mut events = Events::new(send(request).await?);
            let mut collector = Collector::new(progress);

            while let Some(Data::Event(event)) = events.next::<StreamEvent>().await? {
                if event.apply(&mut collector, |name| name)?.is_break() {
                    break;
                }
            }

            Ok(collector.finish())
        })
    }

    fn models(&self) -> BoxFuture<'_, Result<Vec<Model>, LlmError>> {
        Box::pin(list_models(
            self.request(Method::GET, "/v1/models?limit=1000"),
        ))
    }
}
