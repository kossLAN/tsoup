use std::sync::Arc;

use axum::Json;
use axum::extract::{Path as UrlPath, Request, State};
use axum::http::StatusCode;
use axum::response::Response;
use hyper_util::rt::TokioIo;
use tracing::Instrument as _;
use tsoup_proto::node::PROTOCOL;

use super::{ApiError, AppState, upgrade};
use crate::node;
use crate::session::SessionError;

pub async fn connect(
    State(state): State<AppState>,
    UrlPath(name): UrlPath<String>,
    mut request: Request,
) -> Result<Response, ApiError> {
    if !valid(&name) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "a node name may only contain letters, digits, '.', '_' and '-'",
        ));
    }

    if state.sessions.has_node(&name).await {
        return Err(SessionError::NodeConnected(name).into());
    }

    let (upgrade, response) = upgrade::accept(&mut request, PROTOCOL)?;
    let sessions = Arc::clone(&state.sessions);
    let span = tracing::info_span!("node", name);

    tokio::spawn(
        async move {
            match upgrade.await {
                Ok(upgraded) => {
                    node::serve(TokioIo::new(upgraded), name, sessions, state.shutdown).await;
                },
                Err(error) => tracing::warn!(%error, "upgrading a node connection"),
            }
        }
        .instrument(span),
    );

    Ok(response)
}

fn valid(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '_' | '-'))
}

pub async fn folders(
    State(state): State<AppState>,
    UrlPath(name): UrlPath<String>,
) -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(state.sessions.folders(&name).await?))
}
