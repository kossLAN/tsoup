use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::sync::CancellationToken;
use tsoup_proto::node::{self as protocol, Request, Response, StoredSession};
use tsoup_proto::rpc::SILENCE;
use tsoup_proto::{McpListing, McpTool, mcp, workspace};
use tsoup_rpc::{Connection, RpcError};
use uuid::Uuid;

use crate::session::{Record, Sessions};

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("the node {0}")]
    Rpc(#[from] RpcError),
    #[error("a session record is malformed: {0}")]
    Record(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeSummary {
    pub name: String,
    pub directory: String,
}

pub struct Node {
    pub name: String,
    pub directory: String,
    pub sandboxed: bool,
    pub tools: Vec<McpTool>,
    connection: Connection<protocol::Node>,
}

pub struct Folder {
    pub node: Arc<Node>,
    pub name: String,
}

impl Folder {
    pub fn path(&self) -> PathBuf {
        Path::new(&self.node.directory).join(&self.name)
    }
}

impl Node {
    async fn connect(
        name: String,
        connection: Connection<protocol::Node>,
    ) -> Result<(Self, Vec<StoredSession>), NodeError> {
        let Response::Hello {
            directory,
            sandboxed,
            sessions,
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

        let node = Self {
            name,
            directory,
            sandboxed,
            tools,
            connection,
        };
        Ok((node, sessions))
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

    pub async fn records(&self, session: Uuid) -> Result<Vec<Record>, NodeError> {
        match self.connection.request(Request::Load { session }).await? {
            Response::Records { records } => Ok(records
                .into_iter()
                .map(serde_json::from_value)
                .collect::<Result<_, _>>()?),
            _ => Err(RpcError::UnexpectedResponse.into()),
        }
    }

    pub async fn create(&self, session: Uuid, folder: &str) -> Result<(), NodeError> {
        let folder = folder.to_owned();
        self.done(Request::Create { session, folder }).await
    }

    pub async fn append(&self, session: Uuid, record: &Record) -> Result<(), NodeError> {
        let record = serde_json::to_value(record)?;
        self.done(Request::Append { session, record }).await
    }

    pub async fn delete(&self, session: Uuid) -> Result<(), NodeError> {
        self.done(Request::Delete { session }).await
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

    async fn done(&self, request: Request) -> Result<(), NodeError> {
        match self.connection.request(request).await? {
            Response::Done => Ok(()),
            _ => Err(RpcError::UnexpectedResponse.into()),
        }
    }
}

pub async fn serve<T>(io: T, name: String, sessions: Arc<Sessions>, shutdown: CancellationToken)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let connect = Node::connect(name, Connection::open(io));
    let (node, stored) = match tokio::time::timeout(SILENCE, connect).await {
        Ok(Ok((node, stored))) => (Arc::new(node), stored),
        Ok(Err(error)) => {
            tracing::warn!(%error, "a node failed to connect");
            return;
        },
        Err(_) => {
            tracing::warn!("a node did not answer within {} seconds", SILENCE.as_secs());
            return;
        },
    };

    if let Err(error) = sessions.attach(Arc::clone(&node), stored).await {
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
