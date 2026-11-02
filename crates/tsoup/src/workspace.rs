use std::future;
use std::io;
use std::path::{Path, PathBuf};

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use serde_json::{Map, Value};
use tsoup_proto::guest::WORKSPACE;
use tsoup_proto::workspace::{Request, Response};
use tsoup_proto::{Exit, McpOutput, McpTool, mcp};

use crate::node::{Folder, NodeError};
use crate::rpc::RpcError;
use crate::session::Sandbox;
use crate::vm::VmError;

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    Vm(#[from] VmError),
    #[error(transparent)]
    Node(#[from] NodeError),
    #[error("the workspace {0}")]
    Rpc(#[from] RpcError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub trait Workspace: Sync {
    fn run(
        &self,
        request: Request,
    ) -> impl Future<Output = Result<Response, WorkspaceError>> + Send;

    fn mcp(
        &self,
        request: mcp::Request,
    ) -> impl Future<Output = Result<mcp::Response, WorkspaceError>> + Send;

    fn mcp_tools(&self) -> impl Future<Output = Vec<McpTool>> + Send;

    fn exec(
        &self,
        command: String,
        timeout_secs: u64,
    ) -> impl Future<Output = Result<Option<Exit>, WorkspaceError>> + Send {
        async move {
            match self
                .run(Request::Exec {
                    command,
                    timeout_secs,
                })
                .await?
            {
                Response::Exited(exit) => Ok(Some(exit)),
                Response::TimedOut => Ok(None),
                _ => Err(RpcError::UnexpectedResponse.into()),
            }
        }
    }

    fn spawn(&self, command: String) -> impl Future<Output = Result<(), WorkspaceError>> + Send {
        async move {
            match self.run(Request::Spawn { command }).await? {
                Response::Done => Ok(()),
                _ => Err(RpcError::UnexpectedResponse.into()),
            }
        }
    }

    fn read(
        &self,
        path: &str,
    ) -> impl Future<Output = Result<Option<String>, WorkspaceError>> + Send {
        let path = path.to_owned();

        async move {
            match self.run(Request::Read { path }).await? {
                Response::File { content } => Ok(content),
                _ => Err(RpcError::UnexpectedResponse.into()),
            }
        }
    }

    fn write(
        &self,
        path: &str,
        content: String,
    ) -> impl Future<Output = Result<(), WorkspaceError>> + Send {
        let path = path.to_owned();

        async move {
            match self.run(Request::Write { path, content }).await? {
                Response::Done => Ok(()),
                _ => Err(RpcError::UnexpectedResponse.into()),
            }
        }
    }

    fn mcp_call(
        &self,
        server: String,
        tool: String,
        arguments: Map<String, Value>,
    ) -> impl Future<Output = Result<McpOutput, WorkspaceError>> + Send {
        async move {
            match self
                .mcp(mcp::Request::Call {
                    server,
                    tool,
                    arguments,
                })
                .await?
            {
                mcp::Response::Output(output) => Ok(output),
                mcp::Response::Tools(_) => Err(RpcError::UnexpectedResponse.into()),
            }
        }
    }
}

impl Workspace for Sandbox {
    async fn run(&self, request: Request) -> Result<Response, WorkspaceError> {
        match request {
            Request::Read { path } => Ok(Response::File {
                content: read(self.workspace(), path).await?,
            }),
            Request::Write { path, content } => {
                write(self.workspace(), path, content).await?;
                Ok(Response::Done)
            },
            request => Ok(self.vm.workspace(request).await?),
        }
    }

    async fn mcp(&self, request: mcp::Request) -> Result<mcp::Response, WorkspaceError> {
        Ok(self.vm.mcp(request).await?)
    }

    async fn mcp_tools(&self) -> Vec<McpTool> {
        self.vm.mcp_tools().await
    }
}

impl Workspace for Folder {
    async fn run(&self, request: Request) -> Result<Response, WorkspaceError> {
        Ok(self.node.workspace(&self.name, request).await?)
    }

    async fn mcp(&self, request: mcp::Request) -> Result<mcp::Response, WorkspaceError> {
        Ok(self.node.mcp(request).await?)
    }

    fn mcp_tools(&self) -> impl Future<Output = Vec<McpTool>> + Send {
        future::ready(self.node.tools.clone())
    }
}

async fn read(workspace: PathBuf, path: String) -> io::Result<Option<String>> {
    blocking(
        move || match open(&workspace)?.read_to_string(relative(&path)) {
            Ok(content) => Ok(Some(content)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        },
    )
    .await
}

async fn write(workspace: PathBuf, path: String, content: String) -> io::Result<()> {
    blocking(move || {
        let dir = open(&workspace)?;
        let file = relative(&path);

        if let Some(parent) = file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            dir.create_dir_all(parent)?;
        }

        dir.write(file, content)
    })
    .await
}

async fn blocking<T, F>(work: F) -> io::Result<T>
where
    F: FnOnce() -> io::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(io::Error::other)?
}

fn open(workspace: &Path) -> io::Result<Dir> {
    Dir::open_ambient_dir(workspace, ambient_authority())
}

fn relative(path: &str) -> &Path {
    let path = Path::new(path);
    path.strip_prefix(WORKSPACE).unwrap_or(path)
}
