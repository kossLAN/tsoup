use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::http::header::HOST;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use uuid::Uuid;

use super::{ApiError, AppState};
use crate::vm::Protocol;

pub async fn route(State(state): State<AppState>, request: Request, next: Next) -> Response {
    match surface(&request) {
        Some((surface, id)) => proxy(state, id, &surface, request)
            .await
            .unwrap_or_else(IntoResponse::into_response),
        None => next.run(request).await,
    }
}

fn surface(request: &Request) -> Option<(String, Uuid)> {
    let host = request.headers().get(HOST)?.to_str().ok()?;
    let mut labels = host.split('.');
    let surface = labels.next()?.to_owned();
    let id = labels.next()?.parse().ok()?;
    Some((surface, id))
}

async fn proxy(
    state: AppState,
    id: Uuid,
    surface: &str,
    mut request: Request,
) -> Result<Response, ApiError> {
    let session = state.session(id).await?;
    let socket = session
        .vm
        .socket(surface, Protocol::Http)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "the VM serves no such surface"))?;
    let stream = UnixStream::connect(socket).await.map_err(|_| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "the VM surface is not available",
        )
    })?;

    let (mut sender, connection) = http1::handshake(TokioIo::new(stream))
        .await
        .map_err(bad_gateway)?;

    tokio::spawn(async move {
        if let Err(error) = connection.with_upgrades().await {
            tracing::debug!(%error, "a surface connection failed");
        }
    });

    let client = hyper::upgrade::on(&mut request);
    let mut response = sender.send_request(request).await.map_err(bad_gateway)?;

    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        let vm = hyper::upgrade::on(&mut response);
        let shutdown = state.shutdown.clone();

        tokio::spawn(async move {
            let (client, vm) = match tokio::try_join!(client, vm) {
                Ok(upgraded) => upgraded,
                Err(error) => {
                    tracing::debug!(%error, "upgrading a surface connection");
                    return;
                },
            };
            let mut client = TokioIo::new(client);
            let mut vm = TokioIo::new(vm);

            tokio::select! {
                () = shutdown.cancelled() => {},
                _ = tokio::io::copy_bidirectional(&mut client, &mut vm) => {},
            }
        });
    }

    Ok(response.map(Body::new))
}

fn bad_gateway(error: hyper::Error) -> ApiError {
    ApiError::new(
        StatusCode::BAD_GATEWAY,
        format!("the VM surface did not answer: {error}"),
    )
}
