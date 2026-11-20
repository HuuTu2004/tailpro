//! One fail-closed, bounded projection for all external consumers.
use log_core::model::Event;
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

pub const MASK: &str = "[REDACTED]";
const CAP: usize = 64 * 1024;
static USERINFO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([a-z][a-z0-9+.-]*://)[^/\s?#]*@").unwrap());
static BEARER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\bbearer\s+[^\s,;"']+"#).unwrap());
static TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]*){2,}\b").unwrap());
static QUERY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([?&;])([^?&;=\s]+)=([^&;\s#]*)"#).unwrap());
static PAIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)([a-z0-9_.-]+)\s*[:=]\s*(?:"[^"\n]*"|'[^'\n]*'|[^\s&;,}\]"']*)"#).unwrap()
});
fn sensitive(key: &str) -> bool {
    let k: String = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect();
    [
        "password",
        "passwd",
        "pass",
        "pwd",
        "secret",
        "token",
        "apikey",
        "authorization",
        "cookie",
        "privatekey",
        "credential",
        "signature",
        "sig",
        "sessionkey",
    ]
    .iter()
    .any(|part| k.contains(part))
}
#[derive(Debug)]
pub struct Sanitized {
    pub text: String,
    pub incomplete: bool,
}
fn masked() -> Sanitized {
    Sanitized {
        text: MASK.into(),
        incomplete: true,
    }
}
fn decode(input: &str) -> Option<String> {
    let mut value = input.to_owned();
    for _ in 0..2 {
        if !value.contains('%') {
            return Some(value);
        }
        let b = value.as_bytes();
        let mut out = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' {
                let h = |c: u8| (c as char).to_digit(16).map(|n| n as u8);
                out.push(h(*b.get(i + 1)?)? * 16 + h(*b.get(i + 2)?)?);
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        value = String::from_utf8(out).ok()?;
    }
    (!value.contains('%')).then_some(value)
}
fn text(input: &str) -> Sanitized {
    if input.len() > CAP || input.contains(['\\', '{', '}', '"']) {
        return masked();
    }
    let Some(decoded) = decode(input) else {
        return masked();
    };
    // Decoding separators can turn a secret value into apparent new parameters.
    // Mask the complete encoded candidate rather than lose value boundaries.
    if input.contains('%') {
        return Sanitized {
            text: MASK.into(),
            incomplete: false,
        };
    }
    let clean = log_core::parse::strip_ansi(&decoded);
    let clean = USERINFO.replace_all(&clean, "${1}[REDACTED]@");
    let clean = BEARER.replace_all(&clean, "Bearer [REDACTED]");
    let clean = TOKEN.replace_all(&clean, MASK);
    let clean = QUERY.replace_all(&clean, |c: &regex::Captures<'_>| {
        if sensitive(&c[2]) {
            format!("{}{}={MASK}", &c[1], &c[2])
        } else {
            c[0].to_owned()
        }
    });
    if PAIR.captures_iter(&clean).any(|c| sensitive(&c[1])) {
        return Sanitized {
            text: MASK.into(),
            incomplete: false,
        };
    }
    let clean = PAIR.replace_all(&clean, |c: &regex::Captures<'_>| {
        if sensitive(&c[1]) {
            format!("{}={MASK}", &c[1])
        } else {
            c[0].to_owned()
        }
    });
    Sanitized {
        text: clean.into_owned(),
        incomplete: false,
    }
}
fn walk(v: &mut Value, depth: usize, nodes: &mut usize) -> bool {
    *nodes += 1;
    if depth > 32 || *nodes > 4096 {
        *v = Value::String(MASK.into());
        return true;
    }
    match v {
        Value::Object(map) => {
            let mut incomplete = false;
            let old = std::mem::take(map);
            for (key, mut value) in old {
                let Some(key) = decode(&key) else {
                    return true;
                };
                let key = text(&key);
                incomplete |= key.incomplete;
                if sensitive(&key.text) {
                    value = Value::String(MASK.into());
                } else {
                    incomplete |= walk(&mut value, depth + 1, nodes);
                }
                map.insert(key.text, value);
            }
            incomplete
        }
        Value::Array(a) => a.iter_mut().any(|v| walk(v, depth + 1, nodes)),
        Value::String(s) => {
            let safe = if s.trim_start().starts_with(['{', '[']) {
                sanitize_inner(s, depth + 1, nodes)
            } else {
                text(s)
            };
            *s = safe.text;
            safe.incomplete
        }
        _ => false,
    }
}
fn sanitize_inner(input: &str, depth: usize, nodes: &mut usize) -> Sanitized {
    if input.len() > CAP || depth > 32 {
        return masked();
    }
    if input.trim_start().starts_with(['{', '[', '"']) {
        let Ok(mut value) = serde_json::from_str::<Value>(input) else {
            return masked();
        };
        let incomplete = walk(&mut value, depth, nodes);
        // Any ambiguous or capped structured input is masked as a whole.
        if incomplete {
            return masked();
        }
        match serde_json::to_string(&value) {
            Ok(text) if text.len() <= CAP => Sanitized { text, incomplete },
            _ => masked(),
        }
    } else {
        text(input)
    }
}
pub fn sanitize(input: &str) -> Sanitized {
    sanitize_inner(input, 0, &mut 0)
}
/// Predicates only see this event. Never scan the original event before projection.
pub fn project(event: &Event) -> (Event, bool) {
    let safe = sanitize(&event.raw);
    let mut projected = log_core::parse::normalize(
        safe.text.as_bytes(),
        &event.source,
        0,
        event.stream,
        0,
        event.observed_at_ms.parse().unwrap_or(0),
    );
    if event.stream == log_core::model::Stream::Journal {
        if projected.level == log_core::model::Level::Unknown {
            projected.level = event.level;
        }
        projected.event_time = event.event_time.as_ref().map(|t| sanitize(t).text);
    }
    projected.sequence = event.sequence;
    projected.generation = event.generation.clone();
    projected.id = event.id.clone();
    (projected, safe.incomplete)
}
