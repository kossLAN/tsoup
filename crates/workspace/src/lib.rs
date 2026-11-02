pub mod files;
pub mod mcp;
pub mod rpc;
pub mod shell;

use std::io;
use std::path::Path;
use std::time::Duration;

use tsoup_proto::workspace::{Request, Response};

pub async fn run(dir: &Path, request: Request) -> io::Result<Response> {
    Ok(match request {
        Request::Exec {
            command,
            timeout_secs,
        } => shell::exec(&command, dir, Duration::from_secs(timeout_secs))
            .await?
            .map_or(Response::TimedOut, Response::Exited),
        Request::Spawn { command } => {
            shell::spawn(&command, dir)?;
            Response::Done
        },
        Request::Read { path } => Response::File {
            content: files::read(&dir.join(path)).await?,
        },
        Request::Write { path, content } => {
            files::write(&dir.join(path), content).await?;
            Response::Done
        },
    })
}
