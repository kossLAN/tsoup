mod claude_code;
mod error;
mod node;
mod proxy;
mod web;

use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Path as UrlPath, Query, State};
use axum::http::StatusCode;
use axum::response::Sse;
use axum::response::sse::{self, KeepAlive};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use futures_util::{Stream, StreamExt as _, stream};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tsoup_claude_code::Cli;
use uuid::Uuid;

pub use self::error::ApiError;
use crate::agent::Agent;
use crate::llm::Image;
use crate::session::{Entry, EntryPage, NewEnvironment, Session, SessionSummary, Sessions};
use crate::settings::{Selection, SettingsPatch, SettingsStore, SettingsView};
use crate::workspace::Workspace as _;

const BODY_LIMIT: usize = 64 * 1024 * 1024;
const IMAGE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

#[derive(Clone)]
pub struct AppState {
    sessions: Arc<Sessions>,
    agent: Arc<Agent>,
    settings: Arc<SettingsStore>,
    claude_code: Arc<Cli>,
    shutdown: CancellationToken,
}

impl AppState {
    pub fn new(
        sessions: Sessions,
        agent: Agent,
        settings: Arc<SettingsStore>,
        claude_code: Arc<Cli>,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            sessions: Arc::new(sessions),
            agent: Arc::new(agent),
            settings,
            claude_code,
            shutdown,
        }
    }

    async fn session(&self, id: Uuid) -> Result<Arc<Session>, ApiError> {
        Ok(self.sessions.get(id).await?)
    }

    async fn begin_turn(&self, session: &Arc<Session>, prompt: Entry) -> Result<(), ApiError> {
        let turn = Arc::clone(&self.agent).run_turn(Arc::clone(session), prompt);
        Ok(session.start_turn(turn).await?)
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/settings", get(get_settings).patch(update_settings))
        .route("/api/providers/{id}/models", get(list_models))
        .route("/api/claude-code/account", get(claude_code::account))
        .route("/api/claude-code/usage", get(claude_code::usage))
        .route("/api/claude-code/login", post(claude_code::start_login))
        .route(
            "/api/claude-code/login/code",
            post(claude_code::finish_login),
        )
        .route("/api/events", get(list_events))
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route(
            "/api/sessions/{id}",
            patch(update_session).delete(delete_session),
        )
        .route("/api/sessions/{id}/events", get(events))
        .route(
            "/api/sessions/{id}/entries",
            get(list_entries).delete(clear_entries),
        )
        .route("/api/sessions/{id}/turns", post(start_turn))
        .route("/api/sessions/{id}/turn", delete(cancel_turn))
        .route("/api/sessions/{id}/compact", post(compact))
        .route("/api/sessions/{id}/regenerate", post(regenerate))
        .route("/api/sessions/{id}/agents/{agent}", delete(stop_agent))
        .route(
            "/api/sessions/{id}/agents/{agent}/messages",
            post(steer_agent),
        )
        .route(
            "/api/sessions/{id}/questions/{question}",
            post(answer_question),
        )
        .route("/api/sessions/{id}/vm", post(start_vm).delete(destroy_vm))
        .route("/api/sessions/{id}/actions", post(run_action))
        .route("/api/nodes/{name}", get(node::connect))
        .route("/api/nodes/{name}/folders", get(node::folders))
        .fallback(web::serve)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            proxy::route,
        ))
        .with_state(state)
}

#[derive(Deserialize)]
struct TurnRequest {
    content: String,
    #[serde(default)]
    images: Vec<Image>,
}

#[derive(Deserialize)]
struct EntriesQuery {
    before: usize,
    agent: Option<Uuid>,
}

#[derive(Deserialize)]
struct AnswerRequest {
    answer: String,
}

#[derive(Deserialize)]
struct ActionRequest {
    label: String,
}

#[derive(Deserialize)]
struct NewSession {
    environment: NewEnvironment,
    selection: Option<Selection>,
    message: Option<String>,
    #[serde(default)]
    images: Vec<Image>,
}

#[derive(Deserialize)]
struct SessionPatch {
    selection: Option<Selection>,
    title: Option<String>,
}

fn prompt(text: String, images: Vec<Image>) -> Result<Entry, ApiError> {
    if let Some(image) = images
        .iter()
        .find(|image| !IMAGE_TYPES.contains(&image.media_type.as_str()))
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("{} images are not supported", image.media_type),
        ));
    }

    Ok(Entry::User { text, images })
}

async fn get_settings(State(state): State<AppState>) -> Json<SettingsView> {
    Json(state.settings.view().await)
}

async fn update_settings(
    State(state): State<AppState>,
    Json(patch): Json<SettingsPatch>,
) -> Result<Json<SettingsView>, ApiError> {
    Ok(Json(state.settings.update(patch).await?))
}

async fn list_models(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<Json<Vec<String>>, ApiError> {
    let settings = state.settings.get().await;
    let provider = settings
        .provider(id)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "no such provider"))?;
    Ok(Json(state.agent.models(provider).await?))
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
    Json(request): Json<NewSession>,
) -> Result<(StatusCode, Json<SessionSummary>), ApiError> {
    let settings = state.settings.get().await;
    let requested = request.selection.unwrap_or(Selection {
        model: None,
        effort: settings.last_effort,
    });
    let selection = Selection {
        model: requested.model.or_else(|| settings.default_model.clone()),
        effort: requested.effort,
    };

    if let Some(model) = &selection.model {
        settings.check(model)?;
    }

    let effort = selection.effort;
    let session = state
        .sessions
        .create(request.environment, selection)
        .await?;
    state.settings.remember_effort(effort).await?;

    if let Some(text) = request.message {
        let prompt = prompt(text, request.images)?;
        state.begin_turn(&session, prompt).await?;
    }

    Ok((StatusCode::CREATED, Json(session.summary().await)))
}

async fn update_session(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
    Json(patch): Json<SessionPatch>,
) -> Result<StatusCode, ApiError> {
    let session = state.session(id).await?;

    if let Some(selection) = patch.selection {
        if let Some(model) = &selection.model {
            state.settings.get().await.check(model)?;
        }

        let effort = selection.effort;
        session.select(selection).await?;
        state.settings.remember_effort(effort).await?;
    }

    if let Some(title) = patch.title {
        session.rename(&title).await?;
    }

    Ok(StatusCode::NO_CONTENT)
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
    let session = state.session(id).await?;
    let agent = query.agent.unwrap_or(session.id);
    Ok(Json(session.page(agent, query.before).await?))
}

async fn clear_entries(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.session(id).await?.clear().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn start_turn(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
    Json(request): Json<TurnRequest>,
) -> Result<StatusCode, ApiError> {
    let session = state.session(id).await?;

    let prompt = prompt(request.content, request.images)?;

    if session.steer(session.id, prompt.clone()).await.is_err() {
        state.begin_turn(&session, prompt).await?;
    }

    Ok(StatusCode::ACCEPTED)
}

async fn cancel_turn(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.session(id).await?.cancel_turn().await;
    Ok(StatusCode::NO_CONTENT)
}

async fn compact(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    let session = state.session(id).await?;
    let compaction = Arc::clone(&state.agent).run_compaction(Arc::clone(&session));
    session.start_turn(compaction).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn regenerate(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    let session = state.session(id).await?;
    let regeneration = Arc::clone(&state.agent).run_regeneration(Arc::clone(&session));
    session.start_turn(regeneration).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn steer_agent(
    State(state): State<AppState>,
    UrlPath((id, agent)): UrlPath<(Uuid, Uuid)>,
    Json(request): Json<TurnRequest>,
) -> Result<StatusCode, ApiError> {
    let session = state.session(id).await?;
    session
        .steer(agent, prompt(request.content, request.images)?)
        .await?;
    Ok(StatusCode::ACCEPTED)
}

async fn stop_agent(
    State(state): State<AppState>,
    UrlPath((id, agent)): UrlPath<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    state.session(id).await?.stop_agent(agent).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn answer_question(
    State(state): State<AppState>,
    UrlPath((id, question)): UrlPath<(Uuid, Uuid)>,
    Json(request): Json<AnswerRequest>,
) -> Result<StatusCode, ApiError> {
    let session = state.session(id).await?;
    session.answer(question, request.answer).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn start_vm(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    let sandbox = Arc::clone(state.session(id).await?.sandbox()?);

    tokio::spawn(async move {
        if let Err(error) = sandbox.vm.run().await {
            tracing::warn!(%id, %error, "starting a VM");
        }
    });

    Ok(StatusCode::ACCEPTED)
}

async fn destroy_vm(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.session(id).await?.sandbox()?.vm.destroy().await;
    Ok(StatusCode::NO_CONTENT)
}

async fn run_action(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<Uuid>,
    Json(request): Json<ActionRequest>,
) -> Result<StatusCode, ApiError> {
    let session = state.session(id).await?;
    let sandbox = session.sandbox()?;
    let action = session
        .action(&request.label)
        .await
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "no such action"))?;
    sandbox.spawn(action.command).await?;
    sandbox.vm.restore_previews().await?;
    Ok(StatusCode::ACCEPTED)
}
