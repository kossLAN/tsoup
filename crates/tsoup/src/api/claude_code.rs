use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use tsoup_claude_code::Account;

use super::{ApiError, AppState};

#[derive(Serialize)]
pub struct LoginStarted {
    url: String,
}

#[derive(Deserialize)]
pub struct LoginCode {
    code: String,
}

pub async fn account(State(state): State<AppState>) -> Result<Json<Account>, ApiError> {
    Ok(Json(state.claude_code.account().await?))
}

pub async fn start_login(State(state): State<AppState>) -> Result<Json<LoginStarted>, ApiError> {
    let url = state.claude_code.start_login().await?;
    Ok(Json(LoginStarted { url }))
}

pub async fn finish_login(
    State(state): State<AppState>,
    Json(request): Json<LoginCode>,
) -> Result<Json<Account>, ApiError> {
    Ok(Json(state.claude_code.finish_login(&request.code).await?))
}
