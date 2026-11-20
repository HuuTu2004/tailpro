use crate::model::{Event, Level, Quality, Stream};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::fmt;

/// Discard CSI, OSC and other terminal control sequences; never interpret them.
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('\x40'..='\x7e').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\x07' || (c == '\x1b' && chars.next() == Some('\\')) {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if c == '\n' || c == '\t' || !c.is_control() {
            out.push(c);
        }
    }
    out
}
#[derive(Default)]
struct Fields {
    message: Option<String>,
    level: Option<String>,
    time: Option<String>,
    trace: Option<String>,
    order: Option<String>,
    user: Option<String>,
    duration: Option<f64>,
}
struct Select<'a> {
    fields: &'a mut Fields,
    depth: usize,
    key: String,
}
impl<'de> DeserializeSeed<'de> for Select<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        if self.depth > 64 {
            return Err(de::Error::custom("JSON depth cap"));
        }
        d.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Select<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            map.next_value_seed(Select {
                fields: self.fields,
                depth: self.depth + 1,
                key,
            })?;
        }
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq
            .next_element_seed(Select {
                fields: self.fields,
                depth: self.depth + 1,
                key: String::new(),
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<(), E> {
        let slot = match self.key.as_str() {
            "message" | "msg" => &mut self.fields.message,
            "level" | "severity" => &mut self.fields.level,
            "time" | "timestamp" | "@timestamp" => &mut self.fields.time,
            "trace_id" | "traceId" => &mut self.fields.trace,
            "order_id" => &mut self.fields.order,
            "user_id" => &mut self.fields.user,
            "duration_ms" => {
                self.fields.duration = value.parse::<f64>().ok().filter(|n| n.is_finite());
                return Ok(());
            }
            _ => return Ok(()),
        };
        if slot.is_none() {
            *slot = Some(strip_ansi(value));
        }
        Ok(())
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<(), E> {
        self.visit_str(&value)
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<(), E> {
        if self.key == "duration_ms" && value.is_finite() {
            self.fields.duration = Some(value);
        }
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<(), E> {
        self.visit_f64(value as f64)
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<(), E> {
        self.visit_f64(value as f64)
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
}
fn level(s: &str) -> Level {
    match s.to_ascii_lowercase().as_str() {
        "trace" => Level::Trace,
        "debug" => Level::Debug,
        "info" | "information" => Level::Info,
        "warn" | "warning" => Level::Warn,
        "error" | "fatal" | "panic" | "critical" => Level::Error,
        _ => Level::Unknown,
    }
}
pub fn normalize(
    bytes: &[u8],
    source: &str,
    generation: u64,
    stream: Stream,
    physical: u64,
    observed_ms: u64,
) -> Event {
    let raw = String::from_utf8_lossy(bytes).into_owned();
    let text = strip_ansi(&raw);
    let mut fields = Fields::default();
    let mut quality = Quality {
        lossy_utf8: std::str::from_utf8(bytes).is_err(),
        ..Quality::default()
    };
    if text.trim_start().starts_with(['{', '[']) {
        let mut decoder = serde_json::Deserializer::from_str(&text);
        quality.malformed = Select {
            fields: &mut fields,
            depth: 0,
            key: String::new(),
        }
        .deserialize(&mut decoder)
        .and_then(|()| decoder.end())
        .is_err();
        if quality.malformed {
            fields = Fields::default();
        }
    }
    let message = fields.message.unwrap_or(text);
    if fields.time.is_none()
        && let Some(first) = message.split_whitespace().next()
    {
        let candidate = first.trim_matches(['[', ']']);
        if candidate.len() <= 40
            && time::OffsetDateTime::parse(
                candidate,
                &time::format_description::well_known::Rfc3339,
            )
            .is_ok()
        {
            fields.time = Some(candidate.into());
        }
    }
    for part in message.split_whitespace().take(32) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if value.len() > 1024 {
            continue;
        }
        match key {
            "trace_id" if fields.trace.is_none() => fields.trace = Some(value.into()),
            "order_id" if fields.order.is_none() => fields.order = Some(value.into()),
            "user_id" if fields.user.is_none() => fields.user = Some(value.into()),
            "duration" if fields.duration.is_none() => {
                fields.duration = value
                    .strip_suffix("ms")
                    .and_then(|s| s.parse::<f64>().ok())
                    .or_else(|| {
                        value
                            .strip_suffix('s')
                            .and_then(|s| s.parse::<f64>().ok())
                            .map(|n| n * 1000.)
                    })
                    .filter(|n| n.is_finite());
            }
            _ => {}
        }
    }
    let level = fields.level.as_deref().map(level).unwrap_or_else(|| {
        let first = message.split_whitespace().take(3).collect::<Vec<_>>();
        first
            .iter()
            .map(|s| level(s.trim_matches(['[', ']', ':'])))
            .find(|l| *l != Level::Unknown)
            .unwrap_or(
                if (message.starts_with("panic:")
                    || message.starts_with("Traceback (most recent call last):"))
                    || message.contains("Error:")
                {
                    Level::Error
                } else {
                    Level::Unknown
                },
            )
    });
    Event {
        id: String::new(),
        source: source.into(),
        generation: generation.to_string(),
        stream,
        group_id: None,
        observed_at_ms: observed_ms.to_string(),
        event_time: fields.time,
        level,
        message,
        raw,
        trace_id: fields.trace,
        order_id: fields.order,
        user_id: fields.user,
        duration_ms: fields.duration,
        quality,
        physical: vec![physical],
        sequence: 0,
    }
}
