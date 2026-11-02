use std::io;
use std::marker::PhantomData;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

pub const HEARTBEAT: Duration = Duration::from_secs(15);
pub const SILENCE: Duration = Duration::from_secs(45);
const MAX_FRAME: usize = 64 * 1024 * 1024;

pub trait Protocol: 'static {
    type Request: Serialize + DeserializeOwned + Send + 'static;
    type Response: Serialize + DeserializeOwned + Send + 'static;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame<T> {
    pub id: u64,
    pub body: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Call<T> {
    Ping,
    Request(T),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reply<T> {
    Pong,
    Response(T),
    Failed(String),
}

pub type CallerCodec<P> =
    JsonCodec<Frame<Reply<<P as Protocol>::Response>>, Frame<Call<<P as Protocol>::Request>>>;
pub type HandlerCodec<P> =
    JsonCodec<Frame<Call<<P as Protocol>::Request>>, Frame<Reply<<P as Protocol>::Response>>>;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("malformed message: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct JsonCodec<Incoming, Outgoing> {
    frames: LengthDelimitedCodec,
    messages: PhantomData<fn(Outgoing) -> Incoming>,
}

impl<Incoming, Outgoing> Default for JsonCodec<Incoming, Outgoing> {
    fn default() -> Self {
        Self {
            frames: LengthDelimitedCodec::builder()
                .max_frame_length(MAX_FRAME)
                .new_codec(),
            messages: PhantomData,
        }
    }
}

impl<Incoming: DeserializeOwned, Outgoing> Decoder for JsonCodec<Incoming, Outgoing> {
    type Item = Incoming;
    type Error = CodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let Some(frame) = self.frames.decode(src)? else {
            return Ok(None);
        };

        Ok(Some(serde_json::from_slice(&frame)?))
    }
}

impl<Incoming, Outgoing: Serialize> Encoder<Outgoing> for JsonCodec<Incoming, Outgoing> {
    type Error = CodecError;

    fn encode(&mut self, item: Outgoing, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let frame = Bytes::from(serde_json::to_vec(&item)?);
        self.frames.encode(frame, dst)?;
        Ok(())
    }
}
