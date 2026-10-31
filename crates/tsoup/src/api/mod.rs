mod error;
mod proxy;
mod web;

use std::sync::Arc;

use axum::extract::{Path as UrlPath, Query, State};
use axum::http::StatusCode;
use axum::response::Sse;
use axum::response::sse::{self, KeepAlive};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use futures_util::{Stream, StreamExt as _, stream};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub use self::error::ApiError;
use crate::agent::Agent;
use crate::session::{EntryPage, Session, SessionSummary, Sessions};
use crate::settings::{SettingsStore, SettingsUpdate, SettingsView};

#[derive(Clone)]
pub struct AppState {
    sessions: Arc<Sessions>,
    agent: Arc<Agent>,
    settings: Arc<SettingsStore>,
    shutdown: CancellationToken,
}

impl AppState {
    pub fn new(
        sessions: Sessions,
        agent: Agent,
        settings: Arc<SettingsStore>,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            sessions: Arc::new(sessions),
            agent: Arc::new(agent),
            settings,
            shutdown,
        }
    }

    async fn session(&self, id: Uuid) -> Result<Arc<Session>, ApiError> {
        Ok(self.sessions.get(id).await?)
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/settings", get(get_settings).put(update_settings))
        .route("/api/models", get(list_models))
        .route("/api/events", get(list_events))
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}", delete(delete_session))
        .route("/api/sessions/{id}/events", get(events))
        .route("/api/sessions/{id}/entries", get(list_entries))
        .route("/api/sessions/{id}/turns", post(start_turn))
        .route("/api/sessions/{id}/turn", delete(cancel_turn))
        .route("/api/sessions/{id}/vm", post(start_vm).delete(destroy_vm))
        .route("/api/sessions/{id}/actions", post(run_action))
        .fallback(web::serve)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            proxy::route,
        ))
        .with_state(state)
}

#[derive(Deserialize)]
struct TurnRequest {
    content: String,
}

#[derive(Deserialize)]
struct EntriesQuery {
    before: usize,
}

#[derive(Deserialize)]
struct ActionRequest {
    label: String,
}

async fn get_settings(State(state): State<AppState>) -> Json<SettingsView> {
    Json(state.settings.view().await)
}

async fn update_settings(
    State(state): State<AppState>,
    Json(update): Json<SettingsUpdate>,
) -> Result<Json<SettingsView>, ApiError> {
    Ok(Json(state.settings.update(update).await?))
}

async fn list_models(State(state): State<AppState>) -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(state.agent.models().await?))
}

async fn list_sessions(State(state): State<AppState>) -> Json<Vec<SessionSummary>> {
    Json(state.sessions.list().await)
}

async fn list_events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<sse::Event, axum::Error>>> {
    let (snapshot, receiver) = state.sessions.subscribe().await;
    stream_events(snapshot, receiver, state.shutdown)
}

async fn create_session(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<SessionSummary>), ApiError> {
    let session = state.sessions.create().await?;
    Ok((StatusCode::CREATED, Json(session.summary().await)))
}

async fn delete_session(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.sessions.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn events(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<sse::Event, axum::Error>>>, ApiError> {
    let session = state.session(id).await?;
    let (snapshot, receiver) = session.subscribe().await;
    Ok(stream_events(snapshot, receiver, state.shutdown))
}

fn stream_events<T>(
    snapshot: T,
    receiver: broadcast::Receiver<T>,
    shutdown: CancellationToken,
) -> Sse<impl Stream<Item = Result<sse::Event, axum::Error>>>
where
    T: Serialize + Clone + Send + 'static,
{
    let live = stream::unfold(receiver, |mut receiver| async move {
        let event = receiver.recv().await.ok()?;
        Some((event, receiver))
    });
    let events = stream::iter([snapshot])
        .chain(live)
        .map(|event| sse::Event::default().json_data(event))
        .take_until(shutdown.cancelled_owned());
    Sse::new(events).keep_alive(KeepAlive::default())
}

async fn list_entries(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
    Query(query): Query<EntriesQuery>,
) -> Result<Json<EntryPage>, ApiError> {
    Ok(Json(state.session(id).await?.page(query.before).await))
}

async fn start_turn(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
    Json(request): Json<TurnRequest>,
) -> Result<StatusCode, ApiError> {
    let session = state.session(id).await?;
    let turn = Arc::clone(&state.agent).run_turn(Arc::clone(&session), request.content);
    session.start_turn(turn).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn cancel_turn(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.session(id).await?.cancel_turn().await;
    Ok(StatusCode::NO_CONTENT)
}

async fn start_vm(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    let session = state.session(id).await?;

    tokio::spawn(async move {
        if let Err(error) = session.vm.run().await {
            tracing::warn!(%id, %error, "starting a VM");
        }
    });

    Ok(StatusCode::ACCEPTED)
}

async fn destroy_vm(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.session(id).await?.vm.destroy().await;
    Ok(StatusCode::NO_CONTENT)
}

async fn run_action(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
    Json(request): Json<ActionRequest>,
) -> Result<StatusCode, ApiError> {
    let session = state.session(id).await?;
    let action = session
        .action(&request.label)
        .await
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "no such action"))?;
    session.vm.spawn(action.command).await?;
    session.vm.restore_previews().await?;
    Ok(StatusCode::ACCEPTED)
}
