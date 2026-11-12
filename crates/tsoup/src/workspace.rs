use std::io;
use std::path::Path;

use serde_json::{Map, Value};
use tsoup_proto::guest::WORKSPACE;
use tsoup_proto::workspace::{Request, Response};
use tsoup_proto::{Exit, McpOutput, McpTool, mcp};
use tsoup_rpc::RpcError;
use tsoup_workspace::files;

use crate::node::{Folder, NodeError};
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
                content: files::read_within(&self.workspace(), relative(&path)).await?,
            }),
            Request::Write { path, content } => {
                files::write_within(&self.workspace(), relative(&path), content).await?;
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
        Ok(self
            .connected()
            .await?
            .workspace(&self.placement.folder, request)
            .await?)
    }

    async fn mcp(&self, request: mcp::Request) -> Result<mcp::Response, WorkspaceError> {
        Ok(self.connected().await?.mcp(request).await?)
    }

    async fn mcp_tools(&self) -> Vec<McpTool> {
        self.connected()
            .await
            .map(|node| node.tools.clone())
            .unwrap_or_default()
    }
}

fn relative(path: &str) -> &Path {
    let path = Path::new(path);
    path.strip_prefix(WORKSPACE).unwrap_or(path)
}
