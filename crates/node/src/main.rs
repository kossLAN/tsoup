mod connection;
mod mount;
mod node;
mod store;

use std::io::{self, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::{env, future};

use anyhow::{Context as _, anyhow};
use clap::Parser;
use reqwest::Url;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;
use tsoup_proto::node::PROTOCOL;
use tsoup_workspace::sandbox::Sandbox;

use self::node::Node;
use self::store::Store;

// fuser warns about every operation the mount leaves to the kernel.
const LOG_FILTER: &str = "info,fuser=error";

#[derive(Parser)]
#[command(
    name = "tsoup-node",
    version,
    about = "Connects this machine to a tsoup server so sessions can run on it"
)]
struct Cli {
    #[arg(
        long,
        env = "TSOUP_SERVER",
        help = "The URL of the tsoup server, e.g. http://soup:7878"
    )]
    server: Url,

    #[arg(
        long,
        env = "TSOUP_NODE_NAME",
        help = "The name sessions know this machine by, defaults to its hostname"
    )]
    name: Option<String>,

    #[arg(
        long,
        env = "TSOUP_NODE_DIRECTORY",
        default_value = ".",
        help = "The directory sessions work in"
    )]
    directory: PathBuf,

    #[arg(
        long,
        env = "TSOUP_NODE_DATA_DIR",
        help = "Where sessions are stored, defaults to $XDG_DATA_HOME/tsoup-node"
    )]
    data_dir: Option<PathBuf>,

    #[arg(
        long,
        env = "TSOUP_NODE_MCP_CONFIG",
        help = "A JSON file of MCP servers to offer sessions, e.g. {\"github\": {\"command\": [\"github-mcp-server\", \"stdio\"]}}"
    )]
    mcp_config: Option<PathBuf>,

    #[arg(
        long,
        env = "TSOUP_NODE_MOUNT",
        help = "Mount the workspaces of the server's sandbox sessions at this directory"
    )]
    mount: Option<PathBuf>,

    #[arg(
        long,
        env = "TSOUP_NODE_SANDBOX",
        help = "Confine each session to its folder with bubblewrap"
    )]
    sandbox: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(LOG_FILTER)),
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
    let name = match cli.name {
        Some(name) => name,
        None => gethostname::gethostname()
            .into_string()
            .map_err(|_| anyhow!("the hostname is not valid UTF-8, set --name"))?,
    };
    let directory = tokio::fs::canonicalize(&cli.directory)
        .await
        .with_context(|| format!("opening {}", cli.directory.display()))?;
    let data_dir = cli
        .data_dir
        .or_else(default_data_dir)
        .context("set --data-dir, XDG_DATA_HOME or HOME")?;
    let store = Store::open(data_dir.join("sessions"))
        .await
        .with_context(|| format!("opening {}", data_dir.display()))?;

    let url = endpoint(&cli.server, &["api", "nodes", &name])?;
    let share = endpoint(&cli.server, &["api", "share"])?;

    tracing::info!(name, directory = %directory.display(), "starting the node");
    let sandbox = cli
        .sandbox
        .then(|| Sandbox::new(data_dir.join("sandboxes")));
    let node = Arc::new(Node::start(directory, store, cli.mcp_config.as_deref(), sandbox).await);
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;

    let serving = connection::keep(&url, PROTOCOL, async |server| {
        tsoup_rpc::serve(server, Arc::clone(&node)).await
    });
    let mounting = async {
        if let Some(directory) = cli.mount
            && let Err(error) = mount::keep(directory, &share).await
        {
            tracing::warn!("cannot mount the server's workspaces: {error:#}");
        }

        future::pending().await
    };

    tokio::select! {
        result = serving => result,
        result = mounting => result,
        _ = interrupt.recv() => Ok(()),
        _ = terminate.recv() => Ok(()),
    }
}

fn endpoint(server: &Url, path: &[&str]) -> anyhow::Result<Url> {
    let mut url = server.clone();
    url.path_segments_mut()
        .map_err(|()| anyhow!("{server} cannot be a server URL"))?
        .pop_if_empty()
        .extend(path);
    Ok(url)
}

fn default_data_dir() -> Option<PathBuf> {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .map(|dir| dir.join("tsoup-node"))
}
