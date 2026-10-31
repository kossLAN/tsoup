use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::llm::LlmError;
use crate::session::SessionError;
use crate::settings::SettingsError;
use crate::vm::VmError;

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
            SessionError::NotFound => StatusCode::NOT_FOUND,
            SessionError::Busy => StatusCode::CONFLICT,
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
        tracing::error!(%error, "saving settings failed");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
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

impl From<LlmError> for ApiError {
    fn from(error: LlmError) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, error.to_string())
    }
}
