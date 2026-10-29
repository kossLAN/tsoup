use std::io;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::UnixStream;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{ApiError, AppState};

const READ_CHUNK: usize = 64 * 1024;

pub async fn display(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    upgrade: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let session = state.session(id).await?;
    let vnc = UnixStream::connect(session.vm.display_socket())
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "the VM display is not available",
            )
        })?;

    let shutdown = state.shutdown.clone();

    Ok(upgrade.on_upgrade(move |socket| async move {
        if let Err(error) = relay(socket, vnc, shutdown).await {
            tracing::debug!(%error, "a display connection failed");
        }
    }))
}

async fn relay(
    mut socket: WebSocket,
    mut vnc: UnixStream,
    shutdown: CancellationToken,
) -> io::Result<()> {
    let mut buffer = BytesMut::new();

    loop {
        buffer.reserve(READ_CHUNK);

        tokio::select! {
            () = shutdown.cancelled() => return Ok(()),
            message = socket.recv() => match message {
                Some(Ok(Message::Binary(data))) => vnc.write_all(&data).await?,
                Some(Ok(Message::Close(_))) | None => return Ok(()),
                Some(Ok(_)) => {},
                Some(Err(error)) => return Err(io::Error::other(error)),
            },
            read = vnc.read_buf(&mut buffer) => {
                if read? == 0 {
                    return Ok(());
                }

                socket
                    .send(Message::Binary(buffer.split().freeze()))
                    .await
                    .map_err(io::Error::other)?;
            },
        }
    }
}
