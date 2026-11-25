use super::decode::{ParseProfile, decode};
use crate::model::{Event, Level, Quality, RawPayload, RecordEnding, Stream};

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
pub fn normalize(
    bytes: &[u8],
    source: &str,
    generation: u64,
    stream: Stream,
    physical: u64,
    observed_ms: u64,
) -> Event {
    normalize_with(
        bytes,
        source,
        generation,
        stream,
        physical,
        observed_ms,
        &ParseProfile::default(),
    )
}
#[allow(clippy::too_many_arguments)]
pub fn normalize_with(
    bytes: &[u8],
    source: &str,
    generation: u64,
    stream: Stream,
    physical: u64,
    observed_ms: u64,
    profile: &ParseProfile,
) -> Event {
    let mut event = raw(
        bytes.to_vec(),
        source,
        generation,
        stream,
        physical,
        observed_ms,
    );
    if event.quality.lossy_utf8 {
        return event;
    }
    let mut fields = decode(&event.raw, profile);
    event.message = fields.message.unwrap_or_else(|| strip_ansi(&event.raw));
    if event.message.len() > crate::model::FRAME_BYTES {
        let mut end = crate::model::FRAME_BYTES;
        while !event.message.is_char_boundary(end) {
            end -= 1;
        }
        event.message.truncate(end);
        event.quality.truncated = true;
    }
    event.level = fields.level;
    fields.evidence.event_time_ns = fields.time.clone();
    event.event_time = fields
        .time
        .as_ref()
        .and_then(|s| s.parse::<i128>().ok())
        .and_then(|n| time::OffsetDateTime::from_unix_timestamp_nanos(n).ok())
        .and_then(|t| {
            t.format(&time::format_description::well_known::Rfc3339)
                .ok()
        });
    event.trace_id = fields.trace;
    event.order_id = fields.order;
    event.user_id = fields.user;
    event.duration_ms = fields.duration;
    event.evidence = fields.evidence;
    event.quality.malformed = fields.malformed;
    event
}
/// Preserve a bounded uninterpreted payload; used for fragments and raw-only mode.
pub fn raw(
    bytes: Vec<u8>,
    source: &str,
    generation: u64,
    stream: Stream,
    physical: u64,
    observed_ms: u64,
) -> Event {
    let raw = RawPayload::new(bytes);
    Event {
        id: String::new(),
        source: source.into(),
        generation: generation.to_string(),
        stream,
        group_id: None,
        observed_at_ms: observed_ms.to_string(),
        event_time: None,
        transport_time: None,
        evidence: Default::default(),
        level: Level::Unknown,
        message: strip_ansi(&raw),
        ending: RecordEnding::None,
        fragment: None,
        trace_id: None,
        order_id: None,
        user_id: None,
        duration_ms: None,
        quality: Quality {
            lossy_utf8: std::str::from_utf8(raw.bytes()).is_err(),
            ..Quality::default()
        },
        physical: vec![physical],
        sequence: 0,
        checkpoint_complete: true,
        raw,
    }
}
