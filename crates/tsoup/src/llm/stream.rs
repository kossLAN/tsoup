use std::collections::BTreeMap;
use std::{mem, str};

use reqwest::Response;
use serde::de::DeserializeOwned;

use super::{FunctionCall, LlmError, OnProgress, Progress, Reply, ToolCall, ToolKind};

const DONE: &str = "[DONE]";

pub struct Events {
    response: Response,
    buffer: Vec<u8>,
}

impl Events {
    pub const fn new(response: Response) -> Self {
        Self {
            response,
            buffer: Vec::new(),
        }
    }

    pub async fn next<T>(&mut self) -> Result<Option<Data<T>>, LlmError>
    where
        T: DeserializeOwned,
    {
        loop {
            if let Some(end) = self.buffer.windows(2).position(|pair| pair == b"\n\n") {
                let block: Vec<u8> = self.buffer.drain(..end + 2).collect();

                let Some(data) = data(&block)? else {
                    continue;
                };

                if data == DONE {
                    return Ok(Some(Data::Done));
                }

                let event = serde_json::from_str(&data).map_err(LlmError::Malformed)?;
                return Ok(Some(Data::Event(event)));
            }

            match self.response.chunk().await? {
                Some(chunk) => self
                    .buffer
                    .extend(chunk.iter().copied().filter(|&byte| byte != b'\r')),
                None => return Ok(None),
            }
        }
    }
}

pub enum Data<T> {
    Event(T),
    Done,
}

fn data(block: &[u8]) -> Result<Option<String>, LlmError> {
    let text = str::from_utf8(block).map_err(|_| LlmError::Encoding)?;
    let lines: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|data| data.strip_prefix(' ').unwrap_or(data))
        .collect();

    Ok((!lines.is_empty()).then(|| lines.join("\n")))
}

pub struct Collector<'a> {
    progress: &'a OnProgress<'a>,
    text: Part,
    thinking: Part,
    tool_calls: BTreeMap<usize, ToolCall>,
}

#[derive(Default)]
struct Part {
    content: String,
    split: bool,
}

impl Part {
    fn push(&mut self, delta: &str) -> Option<String> {
        if delta.is_empty() {
            return None;
        }

        let mut pushed = String::new();

        if mem::take(&mut self.split) && !self.content.is_empty() {
            pushed.push_str("\n\n");
        }

        pushed.push_str(delta);
        self.content.push_str(&pushed);
        Some(pushed)
    }

    fn finish(self) -> Option<String> {
        Some(self.content).filter(|content| !content.trim().is_empty())
    }
}

impl<'a> Collector<'a> {
    pub fn new(progress: &'a OnProgress<'a>) -> Self {
        Self {
            progress,
            text: Part::default(),
            thinking: Part::default(),
            tool_calls: BTreeMap::new(),
        }
    }

    pub const fn start_text(&mut self) {
        self.text.split = true;
    }

    pub fn start_thinking(&mut self) {
        if !mem::replace(&mut self.thinking.split, true) {
            (self.progress)(Progress::Thinking {
                text: String::new(),
            });
        }
    }

    pub fn text(&mut self, delta: &str) {
        if let Some(text) = self.text.push(delta) {
            (self.progress)(Progress::Text { text });
        }
    }

    pub fn thinking(&mut self, delta: &str) {
        if let Some(text) = self.thinking.push(delta) {
            (self.progress)(Progress::Thinking { text });
        }
    }

    pub fn tool_call(&mut self, key: usize, id: String, name: String) {
        if self.tool_calls.contains_key(&key) {
            return;
        }

        (self.progress)(Progress::ToolCall { name: name.clone() });
        self.tool_calls.insert(
            key,
            ToolCall {
                id,
                kind: ToolKind::Function,
                function: FunctionCall {
                    name,
                    arguments: String::new(),
                },
            },
        );
    }

    pub fn tool_arguments(&mut self, key: usize, delta: &str) {
        if let Some(call) = self.tool_calls.get_mut(&key) {
            call.function.arguments.push_str(delta);
        }
    }

    pub fn finish(self) -> Reply {
        Reply {
            content: self.text.finish(),
            thinking: self.thinking.finish(),
            tool_calls: self.tool_calls.into_values().collect(),
        }
    }
}
