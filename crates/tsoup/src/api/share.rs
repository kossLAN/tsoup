use std::sync::Arc;

use axum::extract::{Request, State};
use axum::response::Response;
use hyper_util::rt::TokioIo;
use tracing::Instrument as _;
use tsoup_proto::share::PROTOCOL;

use super::{ApiError, AppState, upgrade};
use crate::share;

pub async fn mount(
    State(state): State<AppState>,
    mut request: Request,
) -> Result<Response, ApiError> {
    let (upgrade, response) = upgrade::accept(&mut request, PROTOCOL)?;
    let sessions = Arc::clone(&state.sessions);

    tokio::spawn(
        async move {
            match upgrade.await {
                Ok(upgraded) => {
                    share::serve(TokioIo::new(upgraded), sessions, state.shutdown).await;
                },
                Err(error) => tracing::warn!(%error, "upgrading a share connection"),
            }
        }
        .instrument(tracing::info_span!("share")),
    );

    Ok(response)
}
