use futures_util::future::BoxFuture;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use super::stream::{Collector, Data, Events};
use super::{
    Effort, Endpoint, LlmError, Message, OnProgress, Provider, Reply, ToolCall, ToolDefinition,
    ToolKind, list_models, send,
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
}

#[derive(Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
enum ChatMessage<'a> {
    System {
        content: &'a str,
    },
    User {
        content: &'a str,
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

impl<'a> From<&'a Message> for ChatMessage<'a> {
    fn from(message: &'a Message) -> Self {
        match message {
            Message::System { content } => Self::System { content },
            Message::User { content } => Self::User { content },
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
            } => Self::Tool {
                tool_call_id,
                content,
            },
        }
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
                    messages: messages.iter().map(ChatMessage::from).collect(),
                    tools: tools
                        .iter()
                        .map(|function| ChatTool {
                            kind: ToolKind::Function,
                            function,
                        })
                        .collect(),
                    stream: true,
                });
            let mut events = Events::new(send(request).await?);
            let mut collector = Collector::new(progress);
            let mut answered = false;

            while let Some(Data::Event(chunk)) = events.next::<Chunk>().await? {
                if let Some(error) = chunk.error {
                    return Err(LlmError::Remote(error.message));
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

    fn models(&self) -> BoxFuture<'_, Result<Vec<String>, LlmError>> {
        Box::pin(list_models(self.0.bearer(Method::GET, "/models")))
    }
}
