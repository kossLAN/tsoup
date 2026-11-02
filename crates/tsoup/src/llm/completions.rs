use futures_util::future::BoxFuture;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use super::{
    Effort, Endpoint, LlmError, Message, Provider, Reply, ToolCall, ToolDefinition, ToolKind,
    list_models, send,
};

pub struct ChatCompletions(pub Endpoint);

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<Effort>,
    messages: Vec<ChatMessage<'a>>,
    tools: Vec<ChatTool<'a>>,
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
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

impl Provider for ChatCompletions {
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
                });
            let response: ChatResponse = send(request).await?.json().await?;
            let choice = response
                .choices
                .into_iter()
                .next()
                .ok_or(LlmError::NoChoices)?;

            Ok(Reply {
                content: choice.message.content.filter(|content| !content.is_empty()),
                tool_calls: choice.message.tool_calls.unwrap_or_default(),
            })
        })
    }

    fn models(&self) -> BoxFuture<'_, Result<Vec<String>, LlmError>> {
        Box::pin(list_models(self.0.bearer(Method::GET, "/models")))
    }
}
