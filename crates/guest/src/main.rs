use std::collections::HashMap;
use std::io;
use std::net::Ipv4Addr;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};

use anyhow::Context as _;
use tokio::fs::OpenOptions;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, OnceCell};
use tracing_subscriber::EnvFilter;
use tsoup_proto::guest::{Guest, PORT, Request, Response, WORKSPACE};
use tsoup_rpc::Handler;
use tsoup_workspace::mcp::Mcp;

const MCP_CONFIG: &str = "/etc/tsoup/mcp.json";

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
            tracing::error!("the guest agent stopped: {error:#}");
            ExitCode::FAILURE
        },
    }
}

async fn run() -> anyhow::Result<()> {
    let port = OpenOptions::new()
        .read(true)
        .write(true)
        .open(Path::new("/dev/virtio-ports").join(PORT))
        .await?;
    let host = tokio::io::join(port.try_clone().await?, port);

    tsoup_rpc::serve(host, Arc::new(Agent::default())).await
}

#[derive(Default)]
struct Agent {
    forwards: Mutex<HashMap<u16, Arc<AtomicU16>>>,
    mcp: OnceCell<Mcp>,
}

impl Handler for Agent {
    type Protocol = Guest;

    async fn handle(&self, request: Request) -> anyhow::Result<Response> {
        Ok(match request {
            Request::Workspace(request) => {
                Response::Workspace(tsoup_workspace::run(Path::new(WORKSPACE), request).await?)
            },
            Request::Mcp(request) => Response::Mcp(self.mcp().await.handle(request).await?),
            Request::Forward {
                from,
                to,
                require_listener,
            } => {
                self.forward(from, to, require_listener).await?;
                Response::Done
            },
        })
    }
}

impl Agent {
    async fn mcp(&self) -> &Mcp {
        self.mcp
            .get_or_init(|| Mcp::start(Path::new(MCP_CONFIG), Path::new(WORKSPACE)))
            .await
    }

    async fn forward(&self, from: u16, to: u16, require_listener: bool) -> anyhow::Result<()> {
        if require_listener {
            TcpStream::connect(("localhost", to))
                .await
                .with_context(|| format!("nothing accepts connections on port {to}"))?;
        }

        let mut forwards = self.forwards.lock().await;

        if let Some(target) = forwards.get(&from) {
            target.store(to, Ordering::Relaxed);
            return Ok(());
        }

        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, from))
            .await
            .with_context(|| format!("listening on port {from}"))?;
        let target = Arc::new(AtomicU16::new(to));
        tokio::spawn(relay(listener, Arc::clone(&target)));
        forwards.insert(from, target);
        Ok(())
    }
}

async fn relay(listener: TcpListener, target: Arc<AtomicU16>) {
    loop {
        let mut client = match listener.accept().await {
            Ok((client, _)) => client,
            Err(error) => {
                tracing::warn!(%error, "accepting a forwarded connection");
                continue;
            },
        };

        let to = target.load(Ordering::Relaxed);

        tokio::spawn(async move {
            match TcpStream::connect(("localhost", to)).await {
                Ok(mut server) => {
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut server).await;
                },
                Err(error) => tracing::debug!(%error, to, "connecting a forwarded connection"),
            }
        });
    }
}
