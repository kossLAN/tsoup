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
    #[error(transparent)]
    ClaudeCode(#[from] ClaudeCodeError),
}

#[derive(Debug, Clone)]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
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

    fn models(&self) -> BoxFuture<'_, Result<Vec<String>, LlmError>>;
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
    data: Vec<Model>,
}

#[derive(Deserialize)]
struct Model {
    id: String,
}

async fn list_models(request: RequestBuilder) -> Result<Vec<String>, LlmError> {
    let list: ModelList = send(request).await?.json().await?;
    let mut models: Vec<_> = list.data.into_iter().map(|model| model.id).collect();
    models.sort();
    Ok(models)
}

async fn send(request: RequestBuilder) -> Result<reqwest::Response, LlmError> {
    let response = request.send().await?;
    let status = response.status();

    if !status.is_success() {
        return Err(LlmError::Status {
            status,
            body: response.text().await?,
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
