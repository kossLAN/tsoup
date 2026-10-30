use std::io;
use std::marker::PhantomData;

use bytes::{Bytes, BytesMut};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

pub const PORT: &str = "tsoup.agent";
pub const WORKSPACE: &str = "/workspace";
pub const MAX_OUTPUT: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame<T> {
    pub id: u64,
    pub body: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Exec {
        command: String,
        timeout_secs: u64,
    },
    Spawn {
        command: String,
    },
    Forward {
        from: u16,
        to: u16,
    },
    McpTools,
    McpCall {
        server: String,
        tool: String,
        arguments: Map<String, Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Pong,
    Exited {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    TimedOut,
    Spawned,
    Forwarded,
    McpTools {
        tools: Vec<McpTool>,
        failures: Vec<McpFailure>,
    },
    McpOutput {
        output: String,
        error: bool,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub server: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpFailure {
    pub server: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("malformed message: {0}")]
    Json(#[from] serde_json::Error),
}

pub type HostCodec = JsonCodec<Frame<Response>, Frame<Request>>;
pub type GuestCodec = JsonCodec<Frame<Request>, Frame<Response>>;

pub struct JsonCodec<Incoming, Outgoing> {
    frames: LengthDelimitedCodec,
    messages: PhantomData<fn(Outgoing) -> Incoming>,
}

impl<Incoming, Outgoing> Default for JsonCodec<Incoming, Outgoing> {
    fn default() -> Self {
        Self {
            frames: LengthDelimitedCodec::new(),
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
