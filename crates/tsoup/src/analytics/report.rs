use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::ops::Range;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Buckets, Purpose, RequestKey, RequestTotals, ToolTotals};

const DAY_SECS: i64 = 86_400;
const HOUR_SECS: i64 = 3_600;
const EPOCH_WEEKDAY: i64 = 4;
const TOP_SESSIONS: usize = 50;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Query {
    since: u64,
    utc_offset_minutes: i64,
}

#[derive(Serialize)]
pub struct Report {
    totals: Totals,
    previous: Option<Totals>,
    ranking: Vec<PerModel>,
    models: Vec<Row<PerModel>>,
    days: Vec<Row<PerDay>>,
    hours: Vec<Row<PerHour>>,
    kinds: Vec<Row<PerKind>>,
    sessions: Vec<PerSession>,
    tools: Vec<Tool>,
}

#[derive(Default, Serialize)]
struct Totals {
    requests: RequestTotals,
    tools: ToolTotals,
    sessions: usize,
}

#[derive(Serialize)]
struct Row<K> {
    #[serde(flatten)]
    key: K,
    #[serde(flatten)]
    totals: RequestTotals,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
struct PerModel {
    provider: String,
    model: String,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
struct PerDay {
    day: i64,
    provider: String,
    model: String,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
struct PerHour {
    weekday: i64,
    hour: i64,
}

#[derive(PartialEq, Eq, Hash, Serialize)]
struct PerKind {
    purpose: Purpose,
    subagent: bool,
}

#[derive(Serialize)]
struct PerSession {
    session: Uuid,
    last: u64,
    #[serde(flatten)]
    totals: RequestTotals,
}

#[derive(Serialize)]
struct Tool {
    name: String,
    #[serde(flatten)]
    totals: ToolTotals,
}

type Requests<'a> = [(&'a RequestKey, &'a RequestTotals)];

impl RequestTotals {
    const fn tokens(&self) -> u64 {
        self.input + self.output
    }
}

impl Report {
    pub(super) fn build(buckets: &Buckets, query: Query, now: u64) -> Self {
        let Query {
            since,
            utc_offset_minutes,
        } = query;
        let local = |time: u64| time as i64 + utc_offset_minutes * 60;
        let all: Vec<_> = buckets.requests.iter().collect();
        let current: Vec<_> = all
            .iter()
            .copied()
            .filter(|(key, _)| key.time >= since)
            .collect();

        let mut days = grouped(&current, |key| PerDay {
            day: local(key.time).div_euclid(DAY_SECS),
            provider: key.provider.clone(),
            model: key.model.clone(),
        });
        days.sort_by(|left, right| left.key.cmp(&right.key));

        let mut hours = grouped(&current, |key| PerHour {
            weekday: (local(key.time).div_euclid(DAY_SECS) + EPOCH_WEEKDAY).rem_euclid(7),
            hour: local(key.time).rem_euclid(DAY_SECS) / HOUR_SECS,
        });
        hours.sort_by(|left, right| left.key.cmp(&right.key));

        let mut kinds = grouped(&current, |key| PerKind {
            purpose: key.purpose,
            subagent: key.subagent,
        });
        kinds.sort_by_key(|row| Reverse(row.totals.tokens()));

        let previous = (since > 0).then(|| {
            let start = since.saturating_sub(now.saturating_sub(since));
            Totals::within(buckets, start..since)
        });

        Self {
            totals: Totals::within(buckets, since..u64::MAX),
            previous,
            ranking: models(&all).into_iter().map(|row| row.key).collect(),
            models: models(&current),
            days,
            hours,
            kinds,
            sessions: sessions(&current),
            tools: tools(buckets, since),
        }
    }
}

impl Totals {
    fn within(buckets: &Buckets, range: Range<u64>) -> Self {
        let mut totals = Self::default();
        let mut sessions = HashSet::new();

        for (key, bucket) in &buckets.requests {
            if range.contains(&key.time) {
                totals.requests += *bucket;
                sessions.insert(key.session);
            }
        }

        for (key, bucket) in &buckets.tools {
            if range.contains(&key.time) {
                totals.tools += *bucket;
            }
        }

        totals.sessions = sessions.len();
        totals
    }
}

fn grouped<K>(requests: &Requests<'_>, key: impl Fn(&RequestKey) -> K) -> Vec<Row<K>>
where
    K: Hash + Eq,
{
    let mut groups: HashMap<K, RequestTotals> = HashMap::new();

    for (request, totals) in requests {
        *groups.entry(key(request)).or_default() += **totals;
    }

    groups
        .into_iter()
        .map(|(key, totals)| Row { key, totals })
        .collect()
}

fn models(requests: &Requests<'_>) -> Vec<Row<PerModel>> {
    let mut models = grouped(requests, |key| PerModel {
        provider: key.provider.clone(),
        model: key.model.clone(),
    });
    models.sort_by(|left, right| {
        (Reverse(left.totals.tokens()), &left.key)
            .cmp(&(Reverse(right.totals.tokens()), &right.key))
    });
    models
}

fn sessions(requests: &Requests<'_>) -> Vec<PerSession> {
    let mut sessions: HashMap<Uuid, PerSession> = HashMap::new();

    for (key, totals) in requests {
        let session = sessions.entry(key.session).or_insert_with(|| PerSession {
            session: key.session,
            last: key.time,
            totals: RequestTotals::default(),
        });
        session.last = session.last.max(key.time);
        session.totals += **totals;
    }

    let mut sessions: Vec<_> = sessions.into_values().collect();
    sessions.sort_by_key(|session| (Reverse(session.totals.tokens()), session.session));
    sessions.truncate(TOP_SESSIONS);
    sessions
}

fn tools(buckets: &Buckets, since: u64) -> Vec<Tool> {
    let mut tools: HashMap<&str, ToolTotals> = HashMap::new();

    for (key, totals) in &buckets.tools {
        if key.time >= since {
            *tools.entry(&key.name).or_default() += *totals;
        }
    }

    let mut tools: Vec<_> = tools
        .into_iter()
        .map(|(name, totals)| Tool {
            name: name.to_owned(),
            totals,
        })
        .collect();
    tools.sort_by(|left, right| {
        (Reverse(left.totals.calls), &left.name).cmp(&(Reverse(right.totals.calls), &right.name))
    });
    tools
}
