mod agent;
mod api;
mod llm;
mod node;
mod rpc;
mod session;
mod settings;
mod vm;
mod workspace;

use std::env;
use std::io::{self, Write as _};
use std::net::SocketAddr;
use std::os::unix::fs::MetadataExt as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Context as _;
use clap::Parser;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use self::agent::Agent;
use self::api::AppState;
use self::session::Sessions;
use self::settings::SettingsStore;
use self::vm::Builder;

#[derive(Parser)]
#[command(
    name = "tsoup",
    version,
    about = "God I'm so fucking agentic, I love you Dario & Sam."
)]
struct Cli {
    #[arg(
        long,
        env = "TSOUP_LISTEN",
        default_value = "127.0.0.1:7878",
        help = "The address to serve the API on"
    )]
    listen: SocketAddr,

    #[arg(
        long,
        env = "TSOUP_DATA_DIR",
        default_value = "data",
        help = "Where sessions and their workspaces are stored"
    )]
    data_dir: PathBuf,

    #[arg(
        long,
        env = "TSOUP_RUNTIME_DIR",
        help = "Where VM sockets live, defaults to $XDG_RUNTIME_DIR/tsoup"
    )]
    runtime_dir: Option<PathBuf>,

    #[arg(
        long,
        env = "TSOUP_CLAUDE_CODE",
        default_value = "claude",
        help = "The Claude Code CLI that Claude Code providers run"
    )]
    claude_code: PathBuf,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(io::stderr)
        .init();

    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "error: {error:#}");
            ExitCode::FAILURE
        },
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let sessions_dir = cli.data_dir.join("sessions");
    tokio::fs::create_dir_all(&sessions_dir)
        .await
        .with_context(|| format!("creating {}", sessions_dir.display()))?;
    let sessions_dir = tokio::fs::canonicalize(&sessions_dir).await?;
    let uid = tokio::fs::metadata(&sessions_dir).await?.uid();

    let runtime_dir = cli
        .runtime_dir
        .or_else(|| env::var_os("XDG_RUNTIME_DIR").map(|dir| PathBuf::from(dir).join("tsoup")))
        .context("set --runtime-dir or XDG_RUNTIME_DIR")?;

    let claude_code_dir = cli.data_dir.join("claude-code");
    tokio::fs::create_dir_all(&claude_code_dir)
        .await
        .with_context(|| format!("creating {}", claude_code_dir.display()))?;
    let claude_code_dir = tokio::fs::canonicalize(&claude_code_dir).await?;

    let settings = Arc::new(
        SettingsStore::load(cli.data_dir.join("settings.json"))
            .await
            .context("loading settings")?,
    );
    let builder = Arc::new(Builder::new(uid));
    let sessions = Sessions::load(sessions_dir, runtime_dir, builder)
        .await
        .context("loading sessions")?;
    let claude_code = Arc::new(tsoup_claude_code::Cli::new(
        cli.claude_code,
        claude_code_dir,
    ));
    let agent = Agent::new(Arc::clone(&settings), Arc::clone(&claude_code));
    let shutdown = CancellationToken::new();
    let state = AppState::new(sessions, agent, settings, claude_code, shutdown.clone());

    let listener = TcpListener::bind(cli.listen)
        .await
        .with_context(|| format!("listening on {}", cli.listen))?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    tracing::info!(address = %cli.listen, "serving the API");

    axum::serve(listener, api::router(state))
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = interrupt.recv() => {},
                _ = terminate.recv() => {},
            }

            tracing::info!("shutting down");
            shutdown.cancel();
        })
        .await?;

    Ok(())
}
