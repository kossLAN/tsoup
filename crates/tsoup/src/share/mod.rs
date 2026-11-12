mod roots;
mod tree;

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;
use tsoup_proto::share::{self as protocol, Request, Response};
use tsoup_rpc::Handler;

use self::roots::Roots;
use self::tree::Tree;
use crate::session::Sessions;

struct Share(Arc<Tree>);

impl Handler for Share {
    type Protocol = protocol::Share;

    async fn handle(&self, request: Request) -> anyhow::Result<Response> {
        let tree = Arc::clone(&self.0);
        Ok(tokio::task::spawn_blocking(move || tree.answer(request)).await?)
    }
}

pub async fn serve<T>(io: T, sessions: Arc<Sessions>, shutdown: CancellationToken)
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let roots = Roots::new(sessions, Handle::current());
    let share = Arc::new(Share(Arc::new(Tree::new(roots))));
    tracing::info!("a node mounted the share");

    tokio::select! {
        served = tsoup_rpc::serve(io, share) => match served {
            Ok(()) => tracing::info!("a node unmounted the share"),
            Err(error) => tracing::warn!("lost a node that mounted the share: {error:#}"),
        },
        () = shutdown.cancelled() => {},
    }
}
