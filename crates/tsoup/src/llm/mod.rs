mod anthropic;
mod claude_code;
mod completions;
mod responses;
mod stream;

use std::borrow::Cow;

use futures_util::future::BoxFuture;
use reqwest::{Client, Method, RequestBuilder, StatusCode};
use schemars::Schema;
use serde::{Deserialize, Serialize};
use tsoup_claude_code::ClaudeCodeError;

pub use self::anthropic::Anthropic;
pub use self::claude_code::ClaudeCode;
pub use self::completions::ChatCompletions;
pub use self::responses::Responses;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("requesting the model API: {0}")]
    Http(#[from] reqwest::Error),
    #[error("the model API returned {status}: {body}")]
    Status { status: StatusCode, body: String },
    #[error("the model API returned no choices")]
    NoChoices,
    #[error("the model declined to respond")]
    Refused,
    #[error("the model API streamed a malformed event: {0}")]
    Malformed(serde_json::Error),
    #[error("the model API streamed invalid UTF-8")]
    Encoding,
    #[error("the model API reported an error: {0}")]
    Remote(String),
    #[error("the conversation no longer fits in the model's context window")]
    Overflow,
    #[error(transparent)]
    ClaudeCode(#[from] ClaudeCodeError),
}

const OVERFLOW_MARKERS: [&str; 6] = [
    "context_length_exceeded",
    "context window",
    "context length",
    "prompt is too long",
    "input is too long",
    "too many tokens",
];

impl LlmError {
    pub fn remote(message: String) -> Self {
        if overflows(&message) {
            Self::Overflow
        } else {
            Self::Remote(message)
        }
    }
}

fn overflows(message: &str) -> bool {
    let message = message.to_lowercase();
    OVERFLOW_MARKERS
        .iter()
        .any(|marker| message.contains(marker))
}

#[derive(Debug, Clone)]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
        images: Vec<Image>,
    },
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: String,
        images: Vec<Image>,
    },
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self::User {
            content: content.into(),
            images: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub media_type: String,
    pub data: String,
}

const IMAGE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

impl Image {
    pub fn supported(&self) -> bool {
        IMAGE_TYPES.contains(&self.media_type.as_str())
    }

    fn url(&self) -> String {
        format!("data:{};base64,{}", self.media_type, self.data)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolKind {
    #[default]
    Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: ToolKind,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: Cow<'static, str>,
    pub description: Cow<'static, str>,
    pub parameters: Schema,
}

#[derive(Debug)]
pub struct Reply {
    pub content: Option<String>,
    pub thinking: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub name: String,
    pub context: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
}

impl Effort {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Progress {
    Thinking { text: String },
    Text { text: String },
    ToolCall { name: String },
}

pub type OnProgress<'a> = dyn Fn(Progress) + Send + Sync + 'a;

pub trait Provider: Send + Sync {
    fn complete<'a>(
        &'a self,
        model: &'a str,
        effort: Option<Effort>,
        messages: &'a [Message],
        tools: &'a [ToolDefinition],
        progress: &'a OnProgress<'a>,
    ) -> BoxFuture<'a, Result<Reply, LlmError>>;

    fn models(&self) -> BoxFuture<'_, Result<Vec<Model>, LlmError>>;
}

pub struct Endpoint {
    pub http: Client,
    pub url: String,
    pub key: Option<String>,
}

impl Endpoint {
    fn bearer(&self, method: Method, path: &str) -> RequestBuilder {
        let request = self.http.request(method, format!("{}{path}", self.url));

        match &self.key {
            Some(key) => request.bearer_auth(key),
            None => request,
        }
    }
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<ListedModel>,
}

#[derive(Deserialize)]
struct ListedModel {
    id: String,
    max_input_tokens: Option<u64>,
    context_length: Option<u64>,
    context_window: Option<u64>,
}

async fn list_models(request: RequestBuilder) -> Result<Vec<Model>, LlmError> {
    let list: ModelList = send(request).await?.json().await?;
    let mut models: Vec<_> = list
        .data
        .into_iter()
        .map(|model| Model {
            context: model
                .max_input_tokens
                .or(model.context_length)
                .or(model.context_window),
            name: model.id,
        })
        .collect();
    models.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(models)
}

async fn send(request: RequestBuilder) -> Result<reqwest::Response, LlmError> {
    let response = request.send().await?;
    let status = response.status();

    if !status.is_success() {
        let body = response.text().await?;

        return Err(if overflows(&body) {
            LlmError::Overflow
        } else {
            LlmError::Status { status, body }
        });
    }

    Ok(response)
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
