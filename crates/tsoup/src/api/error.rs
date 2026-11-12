use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tsoup_claude_code::ClaudeCodeError;

use crate::llm::LlmError;
use crate::session::SessionError;
use crate::settings::SettingsError;
use crate::vm::VmError;
use crate::workspace::WorkspaceError;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

impl From<SessionError> for ApiError {
    fn from(error: SessionError) -> Self {
        let status = match error {
            SessionError::InvalidFolder(_) | SessionError::UnnamedGroup => StatusCode::BAD_REQUEST,
            SessionError::NotFound
            | SessionError::NoSuchNode
            | SessionError::NoSuchGroup
            | SessionError::NoSuchAgent
            | SessionError::NoSuchQuestion => StatusCode::NOT_FOUND,
            SessionError::Busy
            | SessionError::AgentIdle
            | SessionError::AgentRunning
            | SessionError::NodeConnected(_)
            | SessionError::NotSandbox => StatusCode::CONFLICT,
            SessionError::Node(_) => StatusCode::BAD_GATEWAY,
            SessionError::Io(_) | SessionError::Json(_) => {
                tracing::error!(%error, "a session request failed");
                StatusCode::INTERNAL_SERVER_ERROR
            },
        };

        Self::new(status, error.to_string())
    }
}

impl From<SettingsError> for ApiError {
    fn from(error: SettingsError) -> Self {
        let status = match error {
            SettingsError::UnknownProvider | SettingsError::MissingUrl => StatusCode::BAD_REQUEST,
            SettingsError::Io(_) | SettingsError::Json(_) => {
                tracing::error!(%error, "saving settings failed");
                StatusCode::INTERNAL_SERVER_ERROR
            },
        };

        Self::new(status, error.to_string())
    }
}

impl From<VmError> for ApiError {
    fn from(error: VmError) -> Self {
        let status = match error {
            VmError::NotRunning => StatusCode::CONFLICT,
            _ => StatusCode::BAD_GATEWAY,
        };

        Self::new(status, error.to_string())
    }
}

impl From<WorkspaceError> for ApiError {
    fn from(error: WorkspaceError) -> Self {
        match error {
            WorkspaceError::Vm(error) => error.into(),
            error => Self::new(StatusCode::BAD_GATEWAY, error.to_string()),
        }
    }
}

impl From<ClaudeCodeError> for ApiError {
    fn from(error: ClaudeCodeError) -> Self {
        let status = match error {
            ClaudeCodeError::InvalidCode => StatusCode::BAD_REQUEST,
            ClaudeCodeError::NoLogin => StatusCode::CONFLICT,
            ClaudeCodeError::NoUsage => StatusCode::NOT_FOUND,
            _ => StatusCode::BAD_GATEWAY,
        };

        Self::new(status, error.to_string())
    }
}

impl From<LlmError> for ApiError {
    fn from(error: LlmError) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, error.to_string())
    }
}
