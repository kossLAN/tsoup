use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
#[serde(tag = "type", rename = "user")]
pub struct UserInput<T> {
    message: UserMessage<T>,
    parent_tool_use_id: Option<String>,
    session_id: String,
}

#[derive(Serialize)]
struct UserMessage<T> {
    role: &'static str,
    content: T,
}

#[derive(Serialize)]
#[serde(tag = "type", rename = "control_request")]
pub struct ControlRequestInput<'a> {
    pub request_id: &'a str,
    pub request: Request<'a>,
}

#[derive(Serialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum Request<'a> {
    Initialize {
        #[serde(rename = "sdkMcpServers")]
        sdk_mcp_servers: &'a [&'a str],
    },
    ClaudeAuthenticate {
        #[serde(rename = "loginWithClaudeAi")]
        login_with_claude_ai: bool,
    },
    ClaudeOauthCallback {
        #[serde(rename = "authorizationCode")]
        authorization_code: &'a str,
        state: &'a str,
    },
}

#[derive(Serialize)]
#[serde(tag = "type", rename = "control_response")]
pub struct ControlReply<'a, T> {
    response: ControlSuccess<'a, T>,
}

#[derive(Serialize)]
#[serde(tag = "subtype", rename = "success")]
struct ControlSuccess<'a, T> {
    request_id: &'a str,
    response: T,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Output<E> {
    #[serde(rename = "result")]
    Finished(Finished),
    StreamEvent {
        event: E,
    },
    ControlRequest {
        request_id: String,
        request: ControlRequest,
    },
    ControlResponse {
        response: ControlResponse,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub struct Finished {
    subtype: String,
    is_error: bool,
    result: Option<String>,
    #[serde(default)]
    errors: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlRequest {
    McpMessage {
        message: RpcRequest,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub struct RpcRequest {
    pub id: Option<RpcId>,
    pub method: Option<String>,
    #[serde(default)]
    pub params: RpcParams,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcId {
    Number(i64),
    Text(String),
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcParams {
    pub protocol_version: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlResponse {
    Success {
        request_id: String,
        #[serde(default)]
        response: Value,
    },
    Error {
        request_id: String,
        error: String,
    },
}

#[derive(Deserialize)]
pub struct Initialized {
    #[serde(default)]
    pub models: Vec<ModelInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub value: String,
    pub resolved_model: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub logged_in: bool,
    pub email: Option<String>,
    pub subscription_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Authentication {
    pub manual_url: String,
}

impl<T> UserInput<T> {
    pub const fn new(content: T) -> Self {
        Self {
            message: UserMessage {
                role: "user",
                content,
            },
            parent_tool_use_id: None,
            session_id: String::new(),
        }
    }
}

impl<'a, T> ControlReply<'a, T> {
    pub const fn new(request_id: &'a str, response: T) -> Self {
        Self {
            response: ControlSuccess {
                request_id,
                response,
            },
        }
    }
}

impl Finished {
    pub fn failure(self) -> Option<String> {
        if !self.is_error {
            return None;
        }

        let message = match self.result.filter(|result| !result.is_empty()) {
            Some(result) => result,
            None if self.errors.is_empty() => self.subtype,
            None => self.errors.join("\n"),
        };

        Some(message)
    }
}

#[derive(Deserialize)]
pub struct Reported {
    pub usage_report: Option<UsageReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub rate_limits: Option<RateLimits>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    pub limits: Option<Vec<Limit>>,
    pub extra_usage: Option<ExtraUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limit {
    pub kind: String,
    pub percent: f64,
    pub resets_at: Option<String>,
    pub scope: Option<LimitScope>,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitScope {
    pub model: Option<Named>,
    pub surface: Option<Named>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Named {
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraUsage {
    pub is_enabled: bool,
    pub monthly_limit: Option<f64>,
    pub used_credits: Option<f64>,
    pub currency: Option<String>,
}
