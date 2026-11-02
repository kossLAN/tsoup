use std::sync::Arc;

use anyhow::bail;
use futures_util::{SinkExt as _, StreamExt as _};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::task::JoinSet;
use tokio::time::Instant;
use tokio_util::codec::Framed;
use tsoup_proto::rpc::{Call, Frame, HandlerCodec, Protocol, Reply, SILENCE};

pub trait Handler: Send + Sync + 'static {
    type Protocol: Protocol;

    fn handle(
        &self,
        request: <Self::Protocol as Protocol>::Request,
    ) -> impl Future<Output = anyhow::Result<<Self::Protocol as Protocol>::Response>> + Send;
}

pub async fn serve<T, H>(io: T, handler: Arc<H>) -> anyhow::Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
    H: Handler,
{
    let mut caller = Framed::new(io, HandlerCodec::<H::Protocol>::default());
    let mut handlers = JoinSet::new();
    let silence = tokio::time::sleep(SILENCE);
    tokio::pin!(silence);

    loop {
        tokio::select! {
            () = &mut silence => bail!("nothing arrived for {} seconds", SILENCE.as_secs()),
            frame = caller.next() => {
                let Some(frame) = frame else { return Ok(()) };
                let Frame { id, body } = frame?;
                silence.as_mut().reset(Instant::now() + SILENCE);

                match body {
                    Call::Ping => caller.send(Frame { id, body: Reply::Pong }).await?,
                    Call::Request(request) => {
                        let handler = Arc::clone(&handler);
                        handlers.spawn(async move {
                            let body = match handler.handle(request).await {
                                Ok(response) => Reply::Response(response),
                                Err(error) => Reply::Failed(format!("{error:#}")),
                            };
                            Frame { id, body }
                        });
                    },
                }
            },
            Some(reply) = handlers.join_next() => caller.send(reply?).await?,
        }
    }
}
