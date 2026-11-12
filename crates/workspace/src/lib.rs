pub mod files;
pub mod mcp;
pub mod sandbox;
pub mod shell;

use std::io;
use std::path::Path;
use std::time::Duration;

use tsoup_proto::workspace::{Request, Response};

use self::sandbox::Sandbox;

pub async fn run(dir: &Path, sandbox: Option<&Sandbox>, request: Request) -> io::Result<Response> {
    Ok(match request {
        Request::Exec {
            command,
            timeout_secs,
        } => {
            let shell = match sandbox {
                Some(sandbox) => sandbox.attached(&command, dir).await?,
                None => shell::sh(&command, dir),
            };
            shell::exec(shell, Duration::from_secs(timeout_secs))
                .await?
                .map_or(Response::TimedOut, Response::Exited)
        },
        Request::Spawn { command } => {
            let shell = match sandbox {
                Some(sandbox) => sandbox.detached(&command, dir).await?,
                None => shell::sh(&command, dir),
            };
            shell::spawn(shell)?;
            Response::Done
        },
        Request::Read { path } => Response::File {
            content: match sandbox {
                Some(_) => files::read_within(dir, within(dir, &path)).await?,
                None => files::read(&dir.join(path)).await?,
            },
        },
        Request::Write { path, content } => {
            match sandbox {
                Some(_) => files::write_within(dir, within(dir, &path), content).await?,
                None => files::write(&dir.join(path), content).await?,
            }

            Response::Done
        },
    })
}

fn within<'a>(dir: &Path, path: &'a str) -> &'a Path {
    let path = Path::new(path);
    path.strip_prefix(dir).unwrap_or(path)
}
