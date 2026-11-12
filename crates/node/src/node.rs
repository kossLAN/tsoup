use std::io;
use std::path::{Path, PathBuf};

use anyhow::ensure;
use tsoup_proto::node::{self as protocol, Request, Response, valid_folder};
use tsoup_rpc::Handler;
use tsoup_workspace::mcp::Mcp;
use tsoup_workspace::sandbox::Sandbox;

use crate::store::Store;

pub struct Node {
    directory: PathBuf,
    store: Store,
    mcp: Mcp,
    sandbox: Option<Sandbox>,
}

impl Node {
    pub async fn start(
        directory: PathBuf,
        store: Store,
        mcp_config: Option<&Path>,
        sandbox: Option<Sandbox>,
    ) -> Self {
        let mcp = match mcp_config {
            Some(config) => Mcp::start(config, &directory).await,
            None => Mcp::default(),
        };

        Self {
            directory,
            store,
            mcp,
            sandbox,
        }
    }

    fn folder(&self, name: &str) -> anyhow::Result<PathBuf> {
        ensure!(valid_folder(name), "{name:?} is not a valid folder name");
        Ok(self.directory.join(name))
    }

    async fn folders(&self) -> io::Result<Vec<String>> {
        let mut folders = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.directory).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Ok(name) = entry.file_name().into_string()
                && valid_folder(&name)
                && tokio::fs::metadata(entry.path())
                    .await
                    .is_ok_and(|metadata| metadata.is_dir())
            {
                folders.push(name);
            }
        }

        folders.sort_unstable();
        Ok(folders)
    }
}

impl Handler for Node {
    type Protocol = protocol::Node;

    async fn handle(&self, request: Request) -> anyhow::Result<Response> {
        Ok(match request {
            Request::Hello => Response::Hello {
                directory: self.directory.display().to_string(),
                sandboxed: self.sandbox.is_some(),
                sessions: self.store.list().await?,
            },
            Request::Folders => Response::Folders {
                folders: self.folders().await?,
            },
            Request::Load { session } => Response::Records {
                records: self.store.load(session).await?,
            },
            Request::Create { session, folder } => {
                tokio::fs::create_dir_all(self.folder(&folder)?).await?;
                self.store.create(session, folder).await?;
                Response::Done
            },
            Request::Append { session, record } => {
                self.store.append(session, &record).await?;
                Response::Done
            },
            Request::Delete { session } => {
                self.store.delete(session).await?;
                Response::Done
            },
            Request::Workspace { folder, request } => {
                let dir = self.folder(&folder)?;
                let sandbox = self.sandbox.as_ref();
                Response::Workspace(tsoup_workspace::run(&dir, sandbox, request).await?)
            },
            Request::Mcp(request) => Response::Mcp(self.mcp.handle(request).await?),
        })
    }
}
