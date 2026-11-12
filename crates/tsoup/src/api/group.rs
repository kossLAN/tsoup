use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

use super::{ApiError, AppState};
use crate::session::Group;

#[derive(Deserialize)]
pub struct Named {
    name: String,
}

#[derive(Deserialize)]
pub struct Placement {
    group: Option<Uuid>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(Named { name }): Json<Named>,
) -> Result<(StatusCode, Json<Group>), ApiError> {
    let group = state.sessions.create_group(&name).await?;
    Ok((StatusCode::CREATED, Json(group)))
}

pub async fn rename(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
    Json(Named { name }): Json<Named>,
) -> Result<StatusCode, ApiError> {
    state.sessions.rename_group(id, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.sessions.delete_group(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn place(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
    Json(Placement { group }): Json<Placement>,
) -> Result<StatusCode, ApiError> {
    state.sessions.place(id, group).await?;
    Ok(StatusCode::NO_CONTENT)
}
