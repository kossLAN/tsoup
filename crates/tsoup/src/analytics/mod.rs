mod report;

use std::collections::HashMap;
use std::io;
use std::ops::AddAssign;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt as _;
use tokio::sync::Mutex;
use uuid::Uuid;

pub use self::report::{Query, Report};
use crate::llm::Usage;

const SLOT_SECS: u64 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Turn,
    Compaction,
    Title,
}

pub struct Request {
    pub session: Uuid,
    pub subagent: bool,
    pub purpose: Purpose,
    pub provider: String,
    pub model: String,
    pub completion: Option<Completion>,
}

pub struct Completion {
    pub duration: Duration,
    pub usage: Usage,
}

pub struct ToolCall {
    pub session: Uuid,
    pub name: String,
    pub duration: Duration,
    pub failed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct RequestKey {
    time: u64,
    session: Uuid,
    subagent: bool,
    purpose: Purpose,
    provider: String,
    model: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct RequestTotals {
    requests: u64,
    failed: u64,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
    reasoning: u64,
    duration_ms: u64,
}

impl AddAssign for RequestTotals {
    fn add_assign(&mut self, rhs: Self) {
        self.requests += rhs.requests;
        self.failed += rhs.failed;
        self.input += rhs.input;
        self.output += rhs.output;
        self.cache_read += rhs.cache_read;
        self.cache_write += rhs.cache_write;
        self.reasoning += rhs.reasoning;
        self.duration_ms += rhs.duration_ms;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct ToolKey {
    time: u64,
    session: Uuid,
    name: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct ToolTotals {
    calls: u64,
    failed: u64,
    duration_ms: u64,
}

impl AddAssign for ToolTotals {
    fn add_assign(&mut self, rhs: Self) {
        self.calls += rhs.calls;
        self.failed += rhs.failed;
        self.duration_ms += rhs.duration_ms;
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Line {
    Request {
        #[serde(flatten)]
        key: RequestKey,
        #[serde(flatten)]
        totals: RequestTotals,
    },
    Tool {
        #[serde(flatten)]
        key: ToolKey,
        #[serde(flatten)]
        totals: ToolTotals,
    },
}

#[derive(Default)]
struct Buckets {
    requests: HashMap<RequestKey, RequestTotals>,
    tools: HashMap<ToolKey, ToolTotals>,
}

impl Buckets {
    fn add(&mut self, line: Line) {
        match line {
            Line::Request { key, totals } => *self.requests.entry(key).or_default() += totals,
            Line::Tool { key, totals } => *self.tools.entry(key).or_default() += totals,
        }
    }

    fn len(&self) -> usize {
        self.requests.len() + self.tools.len()
    }

    fn lines(&self) -> impl Iterator<Item = Line> {
        let requests = self.requests.iter().map(|(key, totals)| Line::Request {
            key: key.clone(),
            totals: *totals,
        });
        let tools = self.tools.iter().map(|(key, totals)| Line::Tool {
            key: key.clone(),
            totals: *totals,
        });
        requests.chain(tools)
    }
}

struct State {
    log: File,
    buckets: Buckets,
}

pub struct Analytics {
    state: Mutex<State>,
}

impl Analytics {
    pub async fn load(path: PathBuf) -> io::Result<Self> {
        let stored = match tokio::fs::read_to_string(&path).await {
            Ok(stored) => stored,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error),
        };
        let mut buckets = Buckets::default();
        let mut lines = 0;

        for line in stored.lines() {
            lines += 1;

            match serde_json::from_str(line) {
                Ok(line) => buckets.add(line),
                Err(error) => tracing::warn!(%error, "skipping a malformed analytics line"),
            }
        }

        if lines != buckets.len() {
            rewrite(&path, &buckets).await?;
        }

        let log = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .await?;

        Ok(Self {
            state: Mutex::new(State { log, buckets }),
        })
    }

    pub async fn request(&self, request: Request) {
        let totals = match request.completion {
            Some(Completion { duration, usage }) => RequestTotals {
                requests: 1,
                failed: 0,
                input: usage.input,
                output: usage.output,
                cache_read: usage.cache_read,
                cache_write: usage.cache_write,
                reasoning: usage.reasoning,
                duration_ms: duration.as_millis() as u64,
            },
            None => RequestTotals {
                requests: 1,
                failed: 1,
                ..RequestTotals::default()
            },
        };

        self.record(Line::Request {
            key: RequestKey {
                time: slot(),
                session: request.session,
                subagent: request.subagent,
                purpose: request.purpose,
                provider: request.provider,
                model: request.model,
            },
            totals,
        })
        .await;
    }

    pub async fn tool(&self, call: ToolCall) {
        self.record(Line::Tool {
            key: ToolKey {
                time: slot(),
                session: call.session,
                name: call.name,
            },
            totals: ToolTotals {
                calls: 1,
                failed: u64::from(call.failed),
                duration_ms: call.duration.as_millis() as u64,
            },
        })
        .await;
    }

    pub async fn report(&self, query: Query) -> Report {
        let state = self.state.lock().await;
        Report::build(&state.buckets, query, now())
    }

    async fn record(&self, line: Line) {
        let mut state = self.state.lock().await;

        if let Err(error) = append(&mut state.log, &line).await {
            tracing::warn!(%error, "storing analytics");
        }

        state.buckets.add(line);
    }
}

async fn append(log: &mut File, line: &Line) -> io::Result<()> {
    let mut encoded = serde_json::to_vec(line)?;
    encoded.push(b'\n');
    log.write_all(&encoded).await?;
    log.flush().await
}

async fn rewrite(path: &Path, buckets: &Buckets) -> io::Result<()> {
    let mut encoded = Vec::new();

    for line in buckets.lines() {
        serde_json::to_writer(&mut encoded, &line)?;
        encoded.push(b'\n');
    }

    let staging = path.with_extension("jsonl.tmp");
    tokio::fs::write(&staging, encoded).await?;
    tokio::fs::rename(&staging, path).await
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn slot() -> u64 {
    let now = now();
    now - now % SLOT_SECS
}
