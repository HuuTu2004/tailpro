//! Bounded alpha grammar and scans. No runtime, transport or global index.
use crate::{engine::Snapshot, model::Event};
use std::{cmp::Reverse, collections::BinaryHeap, time::Instant};

#[derive(Clone, Debug)]
enum Predicate {
    Literal(Needle),
    Field(String, String),
    Duration(Comparison, f64),
    Time(Comparison, i128),
}
#[derive(Clone, Debug)]
struct Needle {
    text: String,
    prefix: Vec<usize>,
}
impl Needle {
    fn new(text: String) -> Self {
        let mut prefix = vec![0; text.len()];
        let bytes = text.as_bytes();
        let mut j = 0;
        for i in 1..bytes.len() {
            while j > 0 && bytes[i] != bytes[j] {
                j = prefix[j - 1];
            }
            if bytes[i] == bytes[j] {
                j += 1;
            }
            prefix[i] = j;
        }
        Self { text, prefix }
    }
    fn feed(&self, bytes: &[u8], progress: &mut usize) -> bool {
        if self.text.is_empty() {
            return true;
        }
        let mut found = false;
        for byte in bytes {
            while *progress > 0 && *byte != self.text.as_bytes()[*progress] {
                *progress = self.prefix[*progress - 1];
            }
            if *byte == self.text.as_bytes()[*progress] {
                *progress += 1;
            }
            if *progress == self.text.len() {
                found = true;
                *progress = self.prefix[*progress - 1];
            }
        }
        found
    }
    fn contains(&self, bytes: &[u8]) -> bool {
        std::str::from_utf8(bytes)
            .map_or_else(|_| self.feed(bytes, &mut 0), |s| s.contains(&self.text))
    }
}
/// Local scans only. External consumers use matches() on sanitized projections.
struct FragmentState {
    source: String,
    generation: String,
    stream: crate::model::Stream,
    record: u64,
    end: u64,
    progress: Vec<usize>,
    seen: Vec<bool>,
}
pub struct LocalMatcher<'a> {
    plan: &'a QueryPlan,
    states: Vec<FragmentState>,
}
impl<'a> LocalMatcher<'a> {
    pub fn new(plan: &'a QueryPlan) -> Self {
        Self {
            plan,
            states: Vec::new(),
        }
    }
    pub fn matches(&mut self, event: &Event, server: &str) -> bool {
        let Some(fragment) = event.fragment else {
            self.states
                .retain(|s| s.source != event.source || s.stream != event.stream);
            return self.plan.matches(event, server);
        };
        let index = self
            .states
            .iter()
            .position(|s| s.source == event.source && s.stream == event.stream);
        let valid = index.is_some_and(|i| {
            let s = &self.states[i];
            s.generation == event.generation
                && s.record == fragment.record
                && s.end == fragment.start
        });
        let i = if valid {
            index.unwrap()
        } else {
            if let Some(i) = index {
                self.states.swap_remove(i);
            }
            if self.states.len() >= crate::model::MAX_SOURCES * 5 {
                self.states.remove(0);
            }
            self.states.push(FragmentState {
                source: event.source.clone(),
                generation: event.generation.clone(),
                stream: event.stream,
                record: fragment.record,
                end: fragment.start,
                progress: vec![0; self.plan.predicates.len()],
                seen: vec![false; self.plan.predicates.len()],
            });
            self.states.len() - 1
        };
        let state = &mut self.states[i];
        let mut matched = true;
        let mut fresh = false;
        let mut has_literal = false;
        for (j, p) in self.plan.predicates.iter().enumerate() {
            let hit = match p {
                Predicate::Literal(n) => {
                    has_literal = true;
                    let hit = n.feed(event.raw.bytes(), &mut state.progress[j]);
                    fresh |= hit;
                    state.seen[j] |= hit;
                    state.seen[j]
                }
                _ => self.plan.matches_predicate(p, event, server),
            };
            matched &= hit;
        }
        state.end = fragment.end;
        if fragment.final_part {
            self.states.swap_remove(i);
        }
        matched && (fresh || !has_literal)
    }
}
#[derive(Clone, Debug)]
enum Comparison {
    Equal,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}
#[derive(Clone, Debug)]
pub struct QueryPlan {
    predicates: Vec<Predicate>,
}
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct QueryError(pub String);

impl QueryPlan {
    pub fn compile(input: &str) -> Result<Self, QueryError> {
        let invalid = |s: &str| QueryError(s.into());
        if input.len() > 4096 {
            return Err(invalid("Query exceeds 4 KiB"));
        }
        let mut tokens = Vec::new();
        let mut token = String::new();
        let mut quoted = false;
        let mut escaped = false;
        for c in input.chars() {
            if escaped {
                token.push(c);
                escaped = false;
                continue;
            }
            if quoted && c == '\\' {
                escaped = true;
                continue;
            }
            if c == '"' {
                quoted = !quoted;
                token.push(c);
            } else if c.is_whitespace() && !quoted {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
            } else {
                token.push(c);
            }
        }
        if quoted || escaped {
            return Err(invalid("Unclosed quoted literal"));
        }
        if !token.is_empty() {
            tokens.push(token);
        }
        if tokens.len() > 63 {
            return Err(invalid("Query exceeds 32 clauses"));
        }
        let mut predicates = Vec::new();
        let mut need_clause = false;
        for token in tokens {
            if token == "AND" {
                if predicates.is_empty() || need_clause {
                    return Err(invalid("AND requires two clauses"));
                }
                need_clause = true;
                continue;
            }
            if !token.starts_with('"')
                && (matches!(token.as_str(), "OR" | "NOT")
                    || token.contains(['(', ')', '|'])
                    || token.starts_with('/'))
            {
                return Err(invalid(
                    "OR, NOT, regex and grouping are unsupported; quote literal text",
                ));
            }
            let unquote = |s: &str| -> Result<String, QueryError> {
                if let Some(s) = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                    if s.is_empty() {
                        return Err(invalid("Empty literal"));
                    }
                    Ok(s.into())
                } else if s.contains('"') || s.is_empty() {
                    Err(invalid("Malformed literal; use a complete quoted value"))
                } else {
                    Ok(s.into())
                }
            };
            let predicate = if token.starts_with('"') {
                Predicate::Literal(Needle::new(unquote(&token)?))
            } else if let Some(rest) = token
                .strip_prefix("time")
                .filter(|rest| rest.starts_with([':', '=', '<', '>']))
            {
                let (op, value) = [
                    (">=", Comparison::GreaterEqual),
                    ("<=", Comparison::LessEqual),
                    (">", Comparison::Greater),
                    ("<", Comparison::Less),
                    ("=", Comparison::Equal),
                    (":", Comparison::Equal),
                ]
                .into_iter()
                .find_map(|(s, op)| rest.strip_prefix(s).map(|v| (op, v)))
                .ok_or_else(|| invalid("Time requires a comparison and RFC3339 timestamp"))?;
                let timestamp = time::OffsetDateTime::parse(
                    &unquote(value)?,
                    &time::format_description::well_known::Rfc3339,
                )
                .map_err(|_| invalid("Time requires an RFC3339 timestamp with a timezone"))?;
                Predicate::Time(op, timestamp.unix_timestamp_nanos())
            } else if let Some(rest) = token
                .strip_prefix("duration_ms")
                .or_else(|| token.strip_prefix("duration"))
                .filter(|rest| rest.starts_with([':', '=', '<', '>']))
            {
                let (op, value) = [
                    (">=", Comparison::GreaterEqual),
                    ("<=", Comparison::LessEqual),
                    (">", Comparison::Greater),
                    ("<", Comparison::Less),
                    ("=", Comparison::Equal),
                    (":", Comparison::Equal),
                ]
                .into_iter()
                .find_map(|(s, op)| rest.strip_prefix(s).map(|v| (op, v)))
                .ok_or_else(|| invalid("Duration requires a comparison and ms/s unit"))?;
                let (number, factor) = if let Some(n) = value.strip_suffix("ms") {
                    (n, 1.)
                } else if let Some(n) = value.strip_suffix('s') {
                    (n, 1000.)
                } else {
                    return Err(invalid("Duration requires ms or s"));
                };
                let n = number
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite() && *n >= 0.)
                    .ok_or_else(|| invalid("Invalid duration"))?
                    * factor;
                if !n.is_finite() {
                    return Err(invalid("Invalid duration"));
                }
                Predicate::Duration(op, n)
            } else if let Some((field, value)) = token.split_once([':', '=']) {
                if !matches!(
                    field,
                    "level" | "source" | "server" | "message" | "trace_id" | "order_id" | "user_id"
                ) && !(field.starts_with("attr.") && field.len() <= 133)
                {
                    return Err(invalid("Unknown field"));
                }
                if !value.starts_with('"')
                    && ((value.starts_with('/') && value.ends_with('/')) || value.contains('*'))
                {
                    return Err(invalid(
                        "Regex and wildcard syntax are unsupported; quote literal text",
                    ));
                }
                if field == "level"
                    && !matches!(
                        unquote(value)?.to_ascii_lowercase().as_str(),
                        "trace" | "debug" | "info" | "warn" | "error" | "unknown"
                    )
                {
                    return Err(invalid("Unknown level"));
                }
                Predicate::Field(field.into(), unquote(value)?)
            } else {
                if token.contains(['<', '>', '*']) {
                    return Err(invalid("Unsupported operator; quote literal text"));
                }
                Predicate::Literal(Needle::new(unquote(&token)?))
            };
            predicates.push(predicate);
            need_clause = false;
            if predicates.len() > 32 {
                return Err(invalid("Query exceeds 32 clauses"));
            }
        }
        if need_clause {
            return Err(invalid("AND requires a following clause"));
        }
        Ok(Self { predicates })
    }
    pub fn matches(&self, event: &Event, server: &str) -> bool {
        self.predicates
            .iter()
            .all(|p| self.matches_predicate(p, event, server))
    }
    fn matches_predicate(&self, p: &Predicate, event: &Event, server: &str) -> bool {
        match p {
            Predicate::Literal(s) => {
                event.message.contains(&s.text) || s.contains(event.raw.bytes())
            }
            Predicate::Field(field, value) => match field.as_str() {
                "level" => event.level.as_str().eq_ignore_ascii_case(value),
                "source" => event.source == *value,
                "server" => server == value,
                "message" => event.message.contains(value),
                "trace_id" => event.trace_id.as_ref() == Some(value),
                "order_id" => event.order_id.as_ref() == Some(value),
                "user_id" => event.user_id.as_ref() == Some(value),
                name if name.starts_with("attr.") => {
                    crate::inspect::attribute(&event.raw, &name[5..]).as_ref() == Some(value)
                }
                _ => false,
            },
            Predicate::Duration(op, n) => event.duration_ms.is_some_and(|v| match op {
                Comparison::Equal => v == *n,
                Comparison::Greater => v > *n,
                Comparison::GreaterEqual => v >= *n,
                Comparison::Less => v < *n,
                Comparison::LessEqual => v <= *n,
            }),
            Predicate::Time(op, n) => display_time_ns(event).is_some_and(|v| match op {
                Comparison::Equal => v == *n,
                Comparison::Greater => v > *n,
                Comparison::GreaterEqual => v >= *n,
                Comparison::Less => v < *n,
                Comparison::LessEqual => v <= *n,
            }),
        }
    }
}

/// Use the same clock precedence as the viewer's Time column.
fn display_time_ns(event: &Event) -> Option<i128> {
    if let Some(value) = event.event_time.as_deref() {
        return event
            .evidence
            .event_time_ns
            .as_deref()
            .and_then(|value| value.parse().ok())
            .or_else(|| rfc3339_ns(value));
    }
    if let Some(value) = event.transport_time.as_deref() {
        return rfc3339_ns(value);
    }
    event
        .observed_at_ms
        .parse::<i128>()
        .ok()?
        .checked_mul(1_000_000)
}

fn rfc3339_ns(value: &str) -> Option<i128> {
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|value| value.unix_timestamp_nanos())
}

pub struct Scan<'a> {
    pub events: Vec<&'a Event>,
    pub complete: bool,
    pub has_older: bool,
    pub scanned: usize,
    pub anchor_expired: bool,
}
/// Keep only one page of references. A page cursor is an opaque retained event ID.
/// The snapshot is released by the caller after projection, never retained by a view.
pub struct ScanRequest<'a> {
    pub before: Option<&'a str>,
    pub anchor: Option<&'a str>,
    pub sources: &'a [String],
    pub deadline: Instant,
}
pub fn scan<'a, 'h>(
    snapshot: &'a Snapshot,
    plan: &QueryPlan,
    request: ScanRequest<'_>,
    server: impl Fn(&str) -> Option<&'h str>,
    cancelled: impl Fn() -> bool,
) -> Result<Scan<'a>, crate::engine::Error> {
    let ScanRequest {
        before,
        anchor,
        sources,
        deadline,
    } = request;
    let mut boundary = None;
    let mut anchor_sequence = None;
    // Resolve IDs inside this capture, so expired cursors cannot silently skip retention.
    if before.is_some() || anchor.is_some() {
        for (i, e) in snapshot.events()?.enumerate() {
            if i % 128 == 0 && (cancelled() || Instant::now() >= deadline) {
                return Ok(Scan {
                    events: vec![],
                    complete: false,
                    has_older: false,
                    scanned: i,
                    anchor_expired: false,
                });
            }
            if Some(e.id.as_str()) == before {
                boundary = Some(e.sequence);
            }
            if Some(e.id.as_str()) == anchor {
                anchor_sequence = Some(e.sequence);
            }
        }
        if (before.is_some() && boundary.is_none())
            || (anchor.is_some() && anchor_sequence.is_none())
        {
            return Ok(Scan {
                events: vec![],
                complete: false,
                has_older: false,
                scanned: 0,
                anchor_expired: true,
            });
        }
    }
    let mut heap = BinaryHeap::new();
    let mut selected = Vec::with_capacity(201);
    let mut scanned = 0;
    let mut matched = 0;
    let mut complete = true;
    let mut matcher = LocalMatcher::new(plan);
    for e in snapshot.events()? {
        if scanned % 128 == 0 && (cancelled() || Instant::now() >= deadline) {
            complete = false;
            break;
        }
        scanned += 1;
        if boundary.is_some_and(|s| e.sequence >= s)
            || anchor_sequence.is_some_and(|s| e.sequence > s)
        {
            continue;
        }
        if !sources.is_empty() && !sources.contains(&e.source) {
            continue;
        }
        let Some(host) = server(&e.source) else {
            continue;
        };
        if !matcher.matches(e, host) {
            continue;
        }
        matched += 1;
        if selected.len() < 200 {
            let slot = selected.len();
            selected.push(e);
            heap.push(Reverse((e.sequence, slot)));
        } else if let Some(Reverse((seq, slot))) = heap.peek().copied()
            && e.sequence > seq
        {
            heap.pop();
            selected[slot] = e;
            heap.push(Reverse((e.sequence, slot)));
        }
    }
    selected.sort_unstable_by_key(|e| e.sequence);
    Ok(Scan {
        events: selected,
        complete,
        has_older: matched > 200,
        scanned,
        anchor_expired: false,
    })
}
