mod anthropic;
mod completions;
mod responses;

use std::borrow::Cow;

use futures_util::future::BoxFuture;
use reqwest::{Client, Method, RequestBuilder, StatusCode};
use schemars::Schema;
use serde::{Deserialize, Serialize};

use self::anthropic::Anthropic;
use self::completions::ChatCompletions;
use self::responses::Responses;

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

pub trait Provider: Send + Sync {
    fn complete<'a>(
        &'a self,
        model: &'a str,
        effort: Option<Effort>,
        messages: &'a [Message],
        tools: &'a [ToolDefinition],
    ) -> BoxFuture<'a, Result<Reply, LlmError>>;

    fn models(&self) -> BoxFuture<'_, Result<Vec<String>, LlmError>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    ChatCompletions,
    Responses,
    Anthropic,
}

impl ProviderKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ChatCompletions => "OpenAI compatible",
            Self::Responses => "OpenAI Responses",
            Self::Anthropic => "Anthropic",
        }
    }

    pub fn connect(self, endpoint: Endpoint) -> Box<dyn Provider> {
        match self {
            Self::ChatCompletions => Box::new(ChatCompletions(endpoint)),
            Self::Responses => Box::new(Responses(endpoint)),
            Self::Anthropic => Box::new(Anthropic(endpoint)),
        }
    }
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

fn joined(parts: Vec<String>) -> Option<String> {
    Some(parts.join("\n\n")).filter(|content| !content.is_empty())
}
