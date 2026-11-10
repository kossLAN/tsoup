use std::iter;
use std::ops::ControlFlow;

use futures_util::future::BoxFuture;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use super::stream::{Collector, Data, Events};
use super::{
    Effort, Endpoint, LlmError, Message, Model, OnProgress, Provider, Reply, ToolCall,
    ToolDefinition, ToolKind, list_models, send,
};

pub struct Responses(pub Endpoint);

#[derive(Serialize)]
struct ResponsesRequest<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<Reasoning>,
    input: Vec<Item<'a>>,
    tools: Vec<FunctionTool<'a>>,
    store: bool,
    stream: bool,
}

#[derive(Serialize)]
struct Reasoning {
    effort: Effort,
    summary: Summary,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Summary {
    Auto,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Item<'a> {
    Message {
        role: Role,
        content: &'a str,
    },
    #[serde(rename = "message")]
    Input {
        role: Role,
        content: Vec<InputPart<'a>>,
    },
    FunctionCall {
        call_id: &'a str,
        name: &'a str,
        arguments: &'a str,
    },
    FunctionCallOutput {
        call_id: &'a str,
        output: &'a str,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    System,
    User,
    Assistant,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InputPart<'a> {
    InputText { text: &'a str },
    InputImage { image_url: String },
}

#[derive(Serialize)]
struct FunctionTool<'a> {
    #[serde(rename = "type")]
    kind: ToolKind,
    #[serde(flatten)]
    function: &'a ToolDefinition,
    strict: bool,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum StreamEvent {
    #[serde(rename = "response.output_item.added")]
    ItemAdded {
        output_index: usize,
        item: AddedItem,
    },
    #[serde(rename = "response.content_part.added")]
    ContentPartAdded,
    #[serde(
        rename = "response.output_text.delta",
        alias = "response.refusal.delta"
    )]
    TextDelta { delta: String },
    #[serde(rename = "response.reasoning_summary_part.added")]
    SummaryPartAdded,
    #[serde(rename = "response.reasoning_summary_text.delta")]
    SummaryTextDelta { delta: String },
    #[serde(rename = "response.function_call_arguments.delta")]
    ArgumentsDelta { output_index: usize, delta: String },
    #[serde(rename = "response.completed", alias = "response.incomplete")]
    Completed {
        #[serde(default)]
        response: CompletedResponse,
    },
    #[serde(rename = "response.failed")]
    Failed { response: FailedResponse },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AddedItem {
    Reasoning,
    FunctionCall {
        call_id: String,
        name: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Default, Deserialize)]
struct CompletedResponse {
    usage: Option<ResponseUsage>,
}

#[derive(Deserialize)]
struct ResponseUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct FailedResponse {
    error: Option<ResponseError>,
}

#[derive(Deserialize)]
struct ResponseError {
    message: String,
}

fn items(message: &Message) -> Vec<Item<'_>> {
    match message {
        Message::System { content } => vec![Item::Message {
            role: Role::System,
            content,
        }],
        Message::User { content, images } if images.is_empty() => vec![Item::Message {
            role: Role::User,
            content,
        }],
        Message::User { content, images } => vec![Item::Input {
            role: Role::User,
            content: iter::once(InputPart::InputText { text: content })
                .chain(images.iter().map(|image| InputPart::InputImage {
                    image_url: image.url(),
                }))
                .collect(),
        }],
        Message::Assistant {
            content,
            tool_calls,
        } => content
            .iter()
            .map(|content| Item::Message {
                role: Role::Assistant,
                content,
            })
            .chain(tool_calls.iter().map(Item::from))
            .collect(),
        Message::Tool {
            tool_call_id,
            content,
        } => vec![Item::FunctionCallOutput {
            call_id: tool_call_id,
            output: content,
        }],
    }
}

impl<'a> From<&'a ToolCall> for Item<'a> {
    fn from(call: &'a ToolCall) -> Self {
        Self::FunctionCall {
            call_id: &call.id,
            name: &call.function.name,
            arguments: &call.function.arguments,
        }
    }
}

impl<'a> From<&'a ToolDefinition> for FunctionTool<'a> {
    fn from(function: &'a ToolDefinition) -> Self {
        Self {
            kind: ToolKind::Function,
            function,
            strict: false,
        }
    }
}

impl StreamEvent {
    fn apply(self, collector: &mut Collector<'_>) -> Result<ControlFlow<()>, LlmError> {
        match self {
            Self::ItemAdded {
                item: AddedItem::Reasoning,
                ..
            }
            | Self::SummaryPartAdded => collector.start_thinking(),
            Self::ItemAdded {
                output_index,
                item: AddedItem::FunctionCall { call_id, name },
            } => collector.tool_call(output_index, call_id, name),
            Self::ContentPartAdded => collector.start_text(),
            Self::TextDelta { delta } => collector.text(&delta),
            Self::SummaryTextDelta { delta } => collector.thinking(&delta),
            Self::ArgumentsDelta {
                output_index,
                delta,
            } => collector.tool_arguments(output_index, &delta),
            Self::Completed { response } => {
                if let Some(usage) = response.usage {
                    collector.usage(usage.input_tokens, usage.output_tokens);
                }

                return Ok(ControlFlow::Break(()));
            },
            Self::Failed { response } => {
                return Err(LlmError::remote(response.error.map_or_else(
                    || "the response failed".to_owned(),
                    |error| error.message,
                )));
            },
            Self::Error { message } => return Err(LlmError::remote(message)),
            Self::ItemAdded {
                item: AddedItem::Other,
                ..
            }
            | Self::Other => {},
        }

        Ok(ControlFlow::Continue(()))
    }
}

impl Provider for Responses {
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
                .bearer(Method::POST, "/responses")
                .json(&ResponsesRequest {
                    model,
                    reasoning: effort.map(|effort| Reasoning {
                        effort,
                        summary: Summary::Auto,
                    }),
                    input: messages.iter().flat_map(items).collect(),
                    tools: tools.iter().map(FunctionTool::from).collect(),
                    store: false,
                    stream: true,
                });
            let mut events = Events::new(send(request).await?);
            let mut collector = Collector::new(progress);

            while let Some(Data::Event(event)) = events.next::<StreamEvent>().await? {
                if event.apply(&mut collector)?.is_break() {
                    break;
                }
            }

            Ok(collector.finish())
        })
    }

    fn models(&self) -> BoxFuture<'_, Result<Vec<Model>, LlmError>> {
        Box::pin(list_models(self.0.bearer(Method::GET, "/models")))
    }
}
