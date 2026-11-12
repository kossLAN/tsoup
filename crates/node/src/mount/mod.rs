mod filesystem;
mod inodes;

use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use fuser::{BackgroundSession, Config, MountOption};
use reqwest::Url;
use tokio::process::Command;
use tokio::runtime::Handle;
use tokio::sync::watch;
use tsoup_proto::share::{PROTOCOL, Share};
use tsoup_rpc::Connection;

use self::filesystem::{Filesystem, Link, Owner};
use crate::connection;

const NAME: &str = "tsoup";

struct Mount {
    link: watch::Sender<Option<Connection<Share>>>,
    // Unmounts when dropped.
    _session: BackgroundSession,
}

pub async fn keep(directory: PathBuf, url: &Url) -> anyhow::Result<()> {
    let mount = mount(directory).await?;

    connection::keep(url, PROTOCOL, async |server| {
        let connection = Connection::open(server);
        mount.link.send_replace(Some(connection.clone()));
        connection.closed().await;
        Ok(())
    })
    .await
}

async fn mount(directory: PathBuf) -> anyhow::Result<Mount> {
    release(&directory).await;
    tokio::fs::create_dir_all(&directory)
        .await
        .with_context(|| format!("creating {}", directory.display()))?;
    let metadata = tokio::fs::metadata(&directory).await?;
    let owner = Owner {
        uid: metadata.uid(),
        gid: metadata.gid(),
    };

    let (link, connections) = watch::channel(None);
    let session = tokio::task::spawn_blocking({
        let runtime = Handle::current();
        move || attach(&directory, runtime, connections, owner)
    })
    .await??;
    tracing::info!("mounted the server's workspaces");

    Ok(Mount {
        link,
        _session: session,
    })
}

fn attach(
    directory: &Path,
    runtime: Handle,
    link: Link,
    owner: Owner,
) -> anyhow::Result<BackgroundSession> {
    let mut config = Config::default();
    config.mount_options = vec![
        MountOption::FSName(NAME.to_owned()),
        MountOption::Subtype(NAME.to_owned()),
    ];

    fuser::spawn_mount(Filesystem::new(runtime, link, owner), directory, &config)
        .with_context(|| format!("mounting at {}", directory.display()))
}

// A node that was killed leaves its mount behind, dead, and nothing can be mounted over it.
async fn release(directory: &Path) {
    if let Err(error) = tokio::fs::metadata(directory).await
        && error.kind() == io::ErrorKind::NotConnected
    {
        let _ = Command::new("fusermount3")
            .args(["-u", "-z"])
            .arg(directory)
            .status()
            .await;
    }
}
