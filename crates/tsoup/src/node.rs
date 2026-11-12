use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tsoup_proto::node::{self as protocol, Request, Response};
use tsoup_proto::rpc::SILENCE;
use tsoup_proto::{McpListing, McpTool, mcp, workspace};
use tsoup_rpc::{Connection, RpcError};

use crate::session::{Placement, Sessions};

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("the node {0}")]
    Rpc(#[from] RpcError),
    #[error("the node {0} is not connected")]
    Offline(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeSummary {
    pub name: String,
    pub directory: String,
}

pub type Nodes = RwLock<BTreeMap<String, Arc<Node>>>;

pub struct Node {
    pub name: String,
    pub directory: String,
    pub sandboxed: bool,
    pub tools: Vec<McpTool>,
    connection: Connection<protocol::Node>,
}

pub struct Folder {
    pub placement: Placement,
    nodes: Arc<Nodes>,
}

impl Folder {
    pub fn new(nodes: Arc<Nodes>, placement: Placement) -> Self {
        Self { placement, nodes }
    }

    pub async fn connected(&self) -> Result<Arc<Node>, NodeError> {
        let name = &self.placement.name;

        self.nodes
            .read()
            .await
            .get(name)
            .cloned()
            .ok_or_else(|| NodeError::Offline(name.clone()))
    }
}

impl Node {
    async fn connect(
        name: String,
        connection: Connection<protocol::Node>,
    ) -> Result<Self, NodeError> {
        let Response::Hello {
            directory,
            sandboxed,
        } = connection.request(Request::Hello).await?
        else {
            return Err(RpcError::UnexpectedResponse.into());
        };

        let Response::Mcp(mcp::Response::Tools(McpListing { tools, .. })) = connection
            .request(Request::Mcp(mcp::Request::Tools))
            .await?
        else {
            return Err(RpcError::UnexpectedResponse.into());
        };

        Ok(Self {
            name,
            directory,
            sandboxed,
            tools,
            connection,
        })
    }

    pub fn path(&self, folder: &str) -> PathBuf {
        Path::new(&self.directory).join(folder)
    }

    pub fn summary(&self) -> NodeSummary {
        NodeSummary {
            name: self.name.clone(),
            directory: self.directory.clone(),
        }
    }

    pub async fn folders(&self) -> Result<Vec<String>, NodeError> {
        match self.connection.request(Request::Folders).await? {
            Response::Folders { folders } => Ok(folders),
            _ => Err(RpcError::UnexpectedResponse.into()),
        }
    }

    pub async fn create(&self, folder: &str) -> Result<(), NodeError> {
        let folder = folder.to_owned();

        match self.connection.request(Request::Create { folder }).await? {
            Response::Done => Ok(()),
            _ => Err(RpcError::UnexpectedResponse.into()),
        }
    }

    pub async fn workspace(
        &self,
        folder: &str,
        request: workspace::Request,
    ) -> Result<workspace::Response, NodeError> {
        let folder = folder.to_owned();

        match self
            .connection
            .request(Request::Workspace { folder, request })
            .await?
        {
            Response::Workspace(response) => Ok(response),
            _ => Err(RpcError::UnexpectedResponse.into()),
        }
    }

    pub async fn mcp(&self, request: mcp::Request) -> Result<mcp::Response, NodeError> {
        match self.connection.request(Request::Mcp(request)).await? {
            Response::Mcp(response) => Ok(response),
            _ => Err(RpcError::UnexpectedResponse.into()),
        }
    }
}

pub async fn serve<T>(io: T, name: String, sessions: Arc<Sessions>, shutdown: CancellationToken)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let connect = Node::connect(name, Connection::open(io));
    let node = match tokio::time::timeout(SILENCE, connect).await {
        Ok(Ok(node)) => Arc::new(node),
        Ok(Err(error)) => {
            tracing::warn!(%error, "a node failed to connect");
            return;
        },
        Err(_) => {
            tracing::warn!("a node did not answer within {} seconds", SILENCE.as_secs());
            return;
        },
    };

    if let Err(error) = sessions.attach(Arc::clone(&node)).await {
        tracing::warn!(%error, "refusing a node");
        return;
    }

    tracing::info!(directory = node.directory, "a node connected");

    tokio::select! {
        () = node.connection.closed() => {},
        () = shutdown.cancelled() => {},
    }

    sessions.detach(&node).await;
    tracing::info!("a node disconnected");
}
