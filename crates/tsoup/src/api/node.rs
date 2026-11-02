use std::sync::Arc;

use axum::Json;
use axum::extract::{Path as UrlPath, Request, State};
use axum::http::header::{CONNECTION, UPGRADE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse as _, Response};
use hyper_util::rt::TokioIo;
use tracing::Instrument as _;
use tsoup_proto::node::PROTOCOL;

use super::{ApiError, AppState};
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

    if !request.headers().get(UPGRADE).is_some_and(|protocol| {
        protocol
            .as_bytes()
            .eq_ignore_ascii_case(PROTOCOL.as_bytes())
    }) {
        return Err(ApiError::new(
            StatusCode::UPGRADE_REQUIRED,
            format!("nodes connect with the {PROTOCOL} protocol"),
        ));
    }

    if state.sessions.has_node(&name).await {
        return Err(SessionError::NodeConnected(name).into());
    }

    let upgrade = hyper::upgrade::on(&mut request);
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

    Ok((
        StatusCode::SWITCHING_PROTOCOLS,
        [
            (CONNECTION, HeaderValue::from_static("upgrade")),
            (UPGRADE, HeaderValue::from_static(PROTOCOL)),
        ],
    )
        .into_response())
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
