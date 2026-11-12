use std::collections::HashMap;

use futures_util::{SinkExt as _, StreamExt as _};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, MissedTickBehavior};
use tokio_util::codec::Framed;
use tokio_util::sync::CancellationToken;
use tracing::Instrument as _;
use tsoup_proto::rpc::{Call, CallerCodec, Frame, HEARTBEAT, Protocol, Reply, SILENCE};

const QUEUE: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("disconnected")]
    Disconnected,
    #[error("failed: {0}")]
    Failed(String),
    #[error("sent an unexpected response")]
    UnexpectedResponse,
}

type Outcome<P> = Result<<P as Protocol>::Response, String>;
type Pending<P> = (<P as Protocol>::Request, oneshot::Sender<Outcome<P>>);

pub struct Connection<P: Protocol> {
    calls: mpsc::Sender<Pending<P>>,
    closed: CancellationToken,
}

impl<P: Protocol> Clone for Connection<P> {
    fn clone(&self) -> Self {
        Self {
            calls: self.calls.clone(),
            closed: self.closed.clone(),
        }
    }
}

impl<P: Protocol> Connection<P> {
    pub fn open<T>(io: T) -> Self
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (calls, queue) = mpsc::channel(QUEUE);
        let closed = CancellationToken::new();
        let framed = Framed::new(io, CallerCodec::<P>::default());
        tokio::spawn(drive::<P, T>(framed, queue, closed.clone()).in_current_span());
        Self { calls, closed }
    }

    pub async fn request(&self, request: P::Request) -> Result<P::Response, RpcError> {
        let (reply, response) = oneshot::channel();
        self.calls
            .send((request, reply))
            .await
            .map_err(|_| RpcError::Disconnected)?;

        response
            .await
            .map_err(|_| RpcError::Disconnected)?
            .map_err(RpcError::Failed)
    }

    pub async fn closed(&self) {
        self.closed.cancelled().await;
    }

    pub fn is_closed(&self) -> bool {
        self.closed.is_cancelled()
    }
}

async fn drive<P, T>(
    mut framed: Framed<T, CallerCodec<P>>,
    mut queue: mpsc::Receiver<Pending<P>>,
    closed: CancellationToken,
) where
    P: Protocol,
    T: AsyncRead + AsyncWrite + Unpin,
{
    let mut pending = HashMap::new();
    let mut last_id = 0;
    let mut heard: Option<Instant> = None;
    let mut heartbeat = tokio::time::interval(HEARTBEAT);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        let outgoing = tokio::select! {
            call = queue.recv() => {
                let Some((request, reply)) = call else { break };
                last_id += 1;
                pending.insert(last_id, reply);
                Call::Request(request)
            },
            frame = framed.next() => {
                let Frame { id, body } = match frame {
                    Some(Ok(frame)) => frame,
                    Some(Err(error)) => {
                        tracing::warn!(%error, "reading from the connection");
                        break;
                    },
                    None => break,
                };
                heard = Some(Instant::now());

                let outcome = match body {
                    Reply::Pong => continue,
                    Reply::Response(response) => Ok(response),
                    Reply::Failed(message) => Err(message),
                };

                if let Some(reply) = pending.remove(&id) {
                    let _ = reply.send(outcome);
                }
                continue;
            },
            _ = heartbeat.tick() => {
                if heard.is_some_and(|heard| heard.elapsed() > SILENCE) {
                    tracing::warn!("the connection went silent");
                    break;
                }
                last_id += 1;
                Call::Ping
            },
        };

        if let Err(error) = framed
            .send(Frame {
                id: last_id,
                body: outgoing,
            })
            .await
        {
            tracing::warn!(%error, "writing to the connection");
            break;
        }
    }

    closed.cancel();
}
