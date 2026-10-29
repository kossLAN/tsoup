use reqwest::{Client, RequestBuilder, StatusCode};
use schemars::Schema;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("requesting the model API: {0}")]
    Http(#[from] reqwest::Error),
    #[error("the model API returned {status}: {body}")]
    Status { status: StatusCode, body: String },
    #[error("the model API returned no choices")]
    NoChoices,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: Option<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
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
    #[serde(rename = "type")]
    pub kind: ToolKind,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Schema,
}

#[derive(Debug)]
pub struct Reply {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    tools: &'a [ToolDefinition],
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

#[derive(Deserialize)]
struct ModelList {
    data: Vec<Model>,
}

#[derive(Deserialize)]
struct Model {
    id: String,
}

#[derive(Debug, Clone, Copy)]
pub struct Endpoint<'a> {
    pub url: &'a str,
    pub key: Option<&'a str>,
}

#[derive(Default)]
pub struct LlmClient {
    http: Client,
}

impl LlmClient {
    pub async fn complete(
        &self,
        endpoint: Endpoint<'_>,
        model: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Reply, LlmError> {
        let request = self
            .http
            .post(format!("{}/chat/completions", endpoint.url))
            .json(&ChatRequest {
                model,
                messages,
                tools,
            });
        let response: ChatResponse = send(request, endpoint).await?.json().await?;
        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or(LlmError::NoChoices)?;

        Ok(Reply {
            content: choice.message.content.filter(|content| !content.is_empty()),
            tool_calls: choice.message.tool_calls.unwrap_or_default(),
        })
    }

    pub async fn models(&self, endpoint: Endpoint<'_>) -> Result<Vec<String>, LlmError> {
        let request = self.http.get(format!("{}/models", endpoint.url));
        let list: ModelList = send(request, endpoint).await?.json().await?;
        let mut models: Vec<_> = list.data.into_iter().map(|model| model.id).collect();
        models.sort();
        Ok(models)
    }
}

async fn send(
    request: RequestBuilder,
    endpoint: Endpoint<'_>,
) -> Result<reqwest::Response, LlmError> {
    let request = match endpoint.key {
        Some(key) => request.bearer_auth(key),
        None => request,
    };

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
