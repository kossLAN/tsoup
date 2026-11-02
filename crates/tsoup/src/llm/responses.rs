use futures_util::future::BoxFuture;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use super::{
    Effort, Endpoint, FunctionCall, LlmError, Message, Provider, Reply, ToolCall, ToolDefinition,
    ToolKind, joined, list_models, send,
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
}

#[derive(Serialize)]
struct Reasoning {
    effort: Effort,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Item<'a> {
    Message {
        role: Role,
        content: &'a str,
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
struct FunctionTool<'a> {
    #[serde(rename = "type")]
    kind: ToolKind,
    #[serde(flatten)]
    function: &'a ToolDefinition,
    strict: bool,
}

#[derive(Deserialize)]
struct ResponsesResponse {
    output: Vec<OutputItem>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputItem {
    Message {
        content: Vec<OutputContent>,
    },
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputContent {
    OutputText {
        text: String,
    },
    Refusal {
        refusal: String,
    },
    #[serde(other)]
    Other,
}

fn items(message: &Message) -> Vec<Item<'_>> {
    match message {
        Message::System { content } => vec![Item::Message {
            role: Role::System,
            content,
        }],
        Message::User { content } => vec![Item::Message {
            role: Role::User,
            content,
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

impl OutputContent {
    fn text(self) -> Option<String> {
        match self {
            Self::OutputText { text } => Some(text),
            Self::Refusal { refusal } => Some(refusal),
            Self::Other => None,
        }
    }
}

impl From<ResponsesResponse> for Reply {
    fn from(response: ResponsesResponse) -> Self {
        let mut parts = Vec::new();
        let mut tool_calls = Vec::new();

        for item in response.output {
            match item {
                OutputItem::Message { content } => {
                    parts.extend(content.into_iter().filter_map(OutputContent::text));
                },
                OutputItem::FunctionCall {
                    call_id,
                    name,
                    arguments,
                } => tool_calls.push(ToolCall {
                    id: call_id,
                    kind: ToolKind::Function,
                    function: FunctionCall { name, arguments },
                }),
                OutputItem::Other => {},
            }
        }

        Self {
            content: joined(parts),
            tool_calls,
        }
    }
}

impl Provider for Responses {
    fn complete<'a>(
        &'a self,
        model: &'a str,
        effort: Option<Effort>,
        messages: &'a [Message],
        tools: &'a [ToolDefinition],
    ) -> BoxFuture<'a, Result<Reply, LlmError>> {
        Box::pin(async move {
            let request = self
                .0
                .bearer(Method::POST, "/responses")
                .json(&ResponsesRequest {
                    model,
                    reasoning: effort.map(|effort| Reasoning { effort }),
                    input: messages.iter().flat_map(items).collect(),
                    tools: tools.iter().map(FunctionTool::from).collect(),
                    store: false,
                });
            let response: ResponsesResponse = send(request).await?.json().await?;
            Ok(response.into())
        })
    }

    fn models(&self) -> BoxFuture<'_, Result<Vec<String>, LlmError>> {
        Box::pin(list_models(self.0.bearer(Method::GET, "/models")))
    }
}
