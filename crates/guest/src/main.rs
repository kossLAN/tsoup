use std::io;
use std::path::Path;
use std::process::{ExitCode, Stdio};
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use tokio::fs::OpenOptions;
use tokio::process::Command;
use tokio_util::codec::Framed;
use tracing_subscriber::EnvFilter;
use tsoup_proto::{CodecError, Frame, GuestCodec, MAX_OUTPUT, PORT, Request, Response, WORKSPACE};

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(io::stderr)
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "the guest agent stopped");
            ExitCode::FAILURE
        },
    }
}

async fn run() -> Result<(), CodecError> {
    let port = OpenOptions::new()
        .read(true)
        .write(true)
        .open(Path::new("/dev/virtio-ports").join(PORT))
        .await?;
    let mut host = Framed::new(port, GuestCodec::default());

    while let Some(frame) = host.next().await {
        let Frame { id, body } = frame?;
        let body = handle(body).await;
        host.send(Frame { id, body }).await?;
    }

    Ok(())
}

async fn handle(request: Request) -> Response {
    match request {
        Request::Ping => Response::Pong,
        Request::Exec {
            command,
            timeout_secs,
        } => exec(&command, Duration::from_secs(timeout_secs)).await,
        Request::Spawn { command } => spawn(&command),
    }
}

fn shell(command: &str) -> Command {
    let mut shell = Command::new("sh");
    shell
        .arg("-c")
        .arg(command)
        .current_dir(WORKSPACE)
        .stdin(Stdio::null());
    shell
}

async fn exec(command: &str, timeout: Duration) -> Response {
    let output = shell(command).kill_on_drop(true).output();

    match tokio::time::timeout(timeout, output).await {
        Ok(Ok(output)) => Response::Exited {
            code: output.status.code(),
            stdout: tail(&output.stdout),
            stderr: tail(&output.stderr),
        },
        Ok(Err(error)) => Response::Failed {
            message: error.to_string(),
        },
        Err(_) => Response::TimedOut,
    }
}

fn spawn(command: &str) -> Response {
    let child = shell(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();

    match child {
        Ok(_) => Response::Spawned,
        Err(error) => Response::Failed {
            message: error.to_string(),
        },
    }
}

fn tail(output: &[u8]) -> String {
    let output = String::from_utf8_lossy(output);
    let start = output.len().saturating_sub(MAX_OUTPUT);
    let start = (start..output.len())
        .find(|&index| output.is_char_boundary(index))
        .unwrap_or(output.len());

    output[start..].to_owned()
}
