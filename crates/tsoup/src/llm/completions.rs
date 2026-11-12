use std::borrow::Cow;
use std::{iter, mem};

use futures_util::future::BoxFuture;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use super::stream::{Collector, Counts, Data, Events};
use super::{
    Effort, Endpoint, Image, LlmError, Message, Model, OnProgress, Provider, Reply, ToolCall,
    ToolDefinition, ToolKind, list_models, send,
};

pub struct ChatCompletions(pub Endpoint);

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<Effort>,
    messages: Vec<ChatMessage<'a>>,
    tools: Vec<ChatTool<'a>>,
    stream: bool,
    stream_options: StreamOptions,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
enum ChatMessage<'a> {
    System {
        content: &'a str,
    },
    User {
        content: UserContent<'a>,
    },
    Assistant {
        content: Option<&'a str>,
        #[serde(skip_serializing_if = "<[_]>::is_empty")]
        tool_calls: &'a [ToolCall],
    },
    Tool {
        tool_call_id: &'a str,
        content: &'a str,
    },
}

#[derive(Serialize)]
#[serde(untagged)]
enum UserContent<'a> {
    Text(&'a str),
    Parts(Vec<Part<'a>>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Part<'a> {
    Text { text: Cow<'a, str> },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,
}

impl From<&Image> for Part<'_> {
    fn from(image: &Image) -> Self {
        Self::ImageUrl {
            image_url: ImageUrl { url: image.url() },
        }
    }
}

impl<'a> From<&'a Message> for ChatMessage<'a> {
    fn from(message: &'a Message) -> Self {
        match message {
            Message::System { content } => Self::System { content },
            Message::User { content, images } => Self::User {
                content: if images.is_empty() {
                    UserContent::Text(content)
                } else {
                    UserContent::Parts(
                        iter::once(Part::Text {
                            text: content.into(),
                        })
                        .chain(images.iter().map(Part::from))
                        .collect(),
                    )
                },
            },
            Message::Assistant {
                content,
                tool_calls,
            } => Self::Assistant {
                content: content.as_deref(),
                tool_calls,
            },
            Message::Tool {
                tool_call_id,
                content,
                ..
            } => Self::Tool {
                tool_call_id,
                content,
            },
        }
    }
}

// Tool messages only carry text, so the images a run of tool calls returned follow it as a user
// message.
fn chat(messages: &[Message]) -> Vec<ChatMessage<'_>> {
    let mut chat = Vec::new();
    let mut returned = Vec::new();

    for message in messages {
        match message {
            Message::Tool {
                tool_call_id,
                images,
                ..
            } if !images.is_empty() => {
                returned.push(Part::Text {
                    text: format!("The images tool call {tool_call_id} returned:").into(),
                });
                returned.extend(images.iter().map(Part::from));
            },
            Message::Tool { .. } => {},
            _ => attach(&mut chat, &mut returned),
        }

        chat.push(ChatMessage::from(message));
    }

    attach(&mut chat, &mut returned);
    chat
}

fn attach<'a>(chat: &mut Vec<ChatMessage<'a>>, returned: &mut Vec<Part<'a>>) {
    if !returned.is_empty() {
        chat.push(ChatMessage::User {
            content: UserContent::Parts(mem::take(returned)),
        });
    }
}

#[derive(Serialize)]
struct ChatTool<'a> {
    #[serde(rename = "type")]
    kind: ToolKind,
    function: &'a ToolDefinition,
}

#[derive(Deserialize)]
struct Chunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    error: Option<ChunkError>,
    usage: Option<ChunkUsage>,
}

#[derive(Deserialize)]
struct ChunkUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    prompt_tokens_details: Option<PromptDetails>,
    completion_tokens_details: Option<CompletionDetails>,
}

#[derive(Deserialize)]
struct PromptDetails {
    cached_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct CompletionDetails {
    reasoning_tokens: Option<u64>,
}

impl From<ChunkUsage> for Counts {
    fn from(usage: ChunkUsage) -> Self {
        Self {
            input: usage.prompt_tokens,
            output: usage.completion_tokens,
            cache_read: usage
                .prompt_tokens_details
                .and_then(|details| details.cached_tokens),
            cache_write: None,
            reasoning: usage
                .completion_tokens_details
                .and_then(|details| details.reasoning_tokens),
        }
    }
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Deserialize)]
struct ChunkDelta {
    content: Option<String>,
    reasoning_content: Option<String>,
    reasoning: Option<String>,
    tool_calls: Option<Vec<ChunkToolCall>>,
}

#[derive(Deserialize)]
struct ChunkToolCall {
    index: usize,
    id: Option<String>,
    function: Option<ChunkFunction>,
}

#[derive(Deserialize)]
struct ChunkFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct ChunkError {
    message: String,
}

impl ChunkDelta {
    fn apply(self, collector: &mut Collector<'_>) {
        for thinking in [self.reasoning_content, self.reasoning]
            .into_iter()
            .flatten()
        {
            collector.thinking(&thinking);
        }

        if let Some(content) = self.content {
            collector.text(&content);
        }

        for call in self.tool_calls.into_iter().flatten() {
            let (name, arguments) = call
                .function
                .map(|function| (function.name, function.arguments))
                .unwrap_or_default();

            if let (Some(id), Some(name)) = (call.id, name) {
                collector.tool_call(call.index, id, name);
            }

            if let Some(arguments) = arguments {
                collector.tool_arguments(call.index, &arguments);
            }
        }
    }
}

impl Provider for ChatCompletions {
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
                .0
                .bearer(Method::POST, "/chat/completions")
                .json(&ChatRequest {
                    model,
                    reasoning_effort: effort,
                    messages: chat(messages),
                    tools: tools
                        .iter()
                        .map(|function| ChatTool {
                            kind: ToolKind::Function,
                            function,
                        })
                        .collect(),
                    stream: true,
                    stream_options: StreamOptions {
                        include_usage: true,
                    },
                });
            let mut events = Events::new(send(request).await?);
            let mut collector = Collector::new(progress);
            let mut answered = false;

            while let Some(Data::Event(chunk)) = events.next::<Chunk>().await? {
                if let Some(error) = chunk.error {
                    return Err(LlmError::remote(error.message));
                }

                if let Some(usage) = chunk.usage {
                    collector.usage(usage.into());
                }

                for choice in chunk.choices {
                    answered = true;
                    choice.delta.apply(&mut collector);
                }
            }

            if !answered {
                return Err(LlmError::NoChoices);
            }

            Ok(collector.finish())
        })
    }

    fn models(&self) -> BoxFuture<'_, Result<Vec<Model>, LlmError>> {
        Box::pin(list_models(self.0.bearer(Method::GET, "/models")))
    }
}
