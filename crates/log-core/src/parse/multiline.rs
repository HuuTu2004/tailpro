use crate::model::{Event, FRAME_BYTES, Level};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
enum Kind {
    Go,
    Python,
    Node,
    Java,
    Php,
    Rust,
}
struct Pending {
    event: Event,
    kind: Kind,
    since: Instant,
    touched: Instant,
    lines: usize,
}
/// One assembler per (source, stream, generation). Never mixes stream positions.
pub struct Assembler {
    pending: Option<Pending>,
    limit: usize,
    late: Option<(String, Kind, Instant, String, u64)>,
}
impl Default for Assembler {
    fn default() -> Self {
        Self {
            pending: None,
            late: None,
            limit: FRAME_BYTES,
        }
    }
}
impl Assembler {
    pub fn bounded(limit: usize) -> Self {
        Self {
            limit: limit.clamp(1, FRAME_BYTES),
            ..Self::default()
        }
    }

    fn kind(text: &str) -> Option<Kind> {
        if text.starts_with("thread '") && text.contains("panicked at")
            || text == "stack backtrace:"
        {
            Some(Kind::Rust)
        } else if text.starts_with("PHP Fatal error:")
            || text.starts_with("PHP Stack trace:")
            || text.starts_with("Stack trace:")
        {
            Some(Kind::Php)
        } else if text.starts_with("Exception in thread ")
            || text
                .split_whitespace()
                .next()
                .is_some_and(|s| s.ends_with("Exception:") || s.ends_with("Error:"))
                && text.contains('.')
        {
            Some(Kind::Java)
        } else if text.starts_with("panic:") || text.starts_with("fatal error:") {
            Some(Kind::Go)
        } else if text.starts_with("Traceback (most recent call last):") {
            Some(Kind::Python)
        } else if text.starts_with("Error:")
            || text
                .split_whitespace()
                .next()
                .is_some_and(|s| s.ends_with("Error:"))
        {
            Some(Kind::Node)
        } else {
            None
        }
    }
    fn continuation(kind: Kind, text: &str) -> bool {
        if text.split_whitespace().next().is_some_and(|word| {
            [
                "TRACE", "DEBUG", "INFO", "WARN", "ERROR", "FATAL", "trace", "debug", "info",
                "warn", "error", "fatal",
            ]
            .contains(&word.trim_matches(['[', ']', ':']))
        }) {
            return false;
        }

        if text.trim_start().starts_with(['{', '[']) {
            return false;
        }
        match kind {
            Kind::Go => {
                text.starts_with('\t')
                    || text.starts_with("goroutine ")
                    || text.is_empty()
                    || (text.ends_with(')')
                        && !text.contains(" INFO ")
                        && !text.contains(" ERROR "))
            }
            Kind::Python => {
                text.starts_with("  ")
                    || text.starts_with("\t")
                    || text.contains("Error:")
                    || text.contains("Exception:")
                    || text.starts_with("During handling")
            }
            Kind::Rust => {
                let t = text.trim_start();
                t == "stack backtrace:"
                    || t.starts_with("at ")
                    || t.starts_with("note: run with ")
                    || t.split_once(':').is_some_and(|(n, _)| {
                        !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())
                    })
            }
            Kind::Java => {
                let t = text.trim_start();
                t.starts_with("at ")
                    || t.starts_with("Caused by:")
                    || t.starts_with("Suppressed:")
                    || t.starts_with("... ") && t.ends_with(" more")
            }
            Kind::Php => {
                let t = text.trim_start();
                t == "Stack trace:"
                    || t.starts_with("thrown in ")
                    || t.starts_with('#') && t.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
            }
            Kind::Node => {
                text.trim_start().starts_with("at ") || text.trim_start().starts_with("Caused by:")
            }
        }
    }
    pub fn push(&mut self, mut event: Event, now: Instant) -> Vec<Event> {
        let mut ready = self.flush_due(now);
        if event.fragment.is_some() || event.quality.incomplete {
            ready.extend(self.flush(now));
            self.late = None;
            ready.push(event);
            return ready;
        }
        if let Some(p) = &mut self.pending {
            if event.event_time.is_none()
                && !matches!(
                    event.evidence.decoder,
                    crate::model::evidence::Decoder::Json
                        | crate::model::evidence::Decoder::Logfmt
                        | crate::model::evidence::Decoder::Rule
                )
                && p.event.evidence.revision == event.evidence.revision
                && p.event.generation == event.generation
                && Self::continuation(p.kind, &event.message)
                && p.lines < 200
                && p.event.raw.bytes().len()
                    + p.event.ending.bytes().len()
                    + event.raw.bytes().len()
                    < self.limit
                && p.event.message.len() + event.message.len() < self.limit
            {
                let separator = p.event.ending.bytes();
                p.event.raw.append(&event.raw, separator);
                p.event.ending = event.ending;
                p.event.message.push('\n');
                p.event.message.push_str(&event.message);
                p.event.physical.extend(event.physical);
                p.event.quality.truncated |= event.quality.truncated;
                p.lines += 1;
                p.event.evidence.group_lines = p.lines as u16;
                p.touched = now;
                return ready;
            }
            if Self::continuation(p.kind, &event.message) {
                p.event.quality.incomplete = true;
            }
            let reason = if p.event.evidence.revision != event.evidence.revision {
                "revision"
            } else if p.event.generation != event.generation {
                "generation"
            } else if p.lines >= 200 {
                "line_cap"
            } else if Self::continuation(p.kind, &event.message) {
                "byte_cap_or_boundary"
            } else {
                "boundary"
            };
            ready.extend(self.flush_reason(now, reason));
        }
        if let Some((id, kind, until, generation, revision)) = &self.late {
            if now <= *until
                && event.generation == *generation
                && event.evidence.revision == *revision
                && event.event_time.is_none()
                && !matches!(
                    event.evidence.decoder,
                    crate::model::evidence::Decoder::Json
                        | crate::model::evidence::Decoder::Logfmt
                        | crate::model::evidence::Decoder::Rule
                )
                && Self::continuation(*kind, &event.message)
            {
                event.group_id = Some(id.clone());
                if event.level == Level::Unknown {
                    event.level = Level::Error;
                }
                event.evidence.group_flush = Some("late_link");
                event.quality.continuation = true;
                event.quality.incomplete = true;
                ready.push(event);
                return ready;
            }
            self.late = None;
        }
        let kind = Self::kind(&event.message);
        if let Some(kind) = kind {
            event.evidence.group_family = Some(match kind {
                Kind::Go => "go",
                Kind::Python => "python",
                Kind::Node => "node",
                Kind::Java => "java",
                Kind::Php => "php",
                Kind::Rust => "rust",
            });
            event.evidence.group_lines = 1;
            if event.level == Level::Unknown {
                event.level = Level::Error;
            }
            event.group_id = Some(format!(
                "group:{}:{}:{:?}:{}",
                event.source, event.generation, event.stream, event.physical[0]
            ));
            self.pending = Some(Pending {
                event,
                kind,
                since: now,
                touched: now,
                lines: 1,
            });
        } else {
            ready.push(event);
        }
        ready
    }
    pub fn flush_due(&mut self, now: Instant) -> Vec<Event> {
        if self.pending.as_ref().is_some_and(|p| {
            now.duration_since(p.touched) >= Duration::from_millis(50)
                || now.duration_since(p.since) >= Duration::from_millis(100)
        }) {
            let reason = if self
                .pending
                .as_ref()
                .is_some_and(|p| now.duration_since(p.since) >= Duration::from_millis(100))
            {
                "max_age"
            } else {
                "idle"
            };
            self.flush_reason(now, reason)
        } else {
            vec![]
        }
    }
    pub fn flush(&mut self, now: Instant) -> Vec<Event> {
        self.flush_reason(now, "stop_or_fence")
    }
    fn flush_reason(&mut self, now: Instant, reason: &'static str) -> Vec<Event> {
        if let Some(mut p) = self.pending.take() {
            p.event.evidence.group_flush = Some(reason);
            self.late = p.event.group_id.as_ref().map(|id| {
                (
                    id.clone(),
                    p.kind,
                    now + Duration::from_secs(2),
                    p.event.generation.clone(),
                    p.event.evidence.revision,
                )
            });
            vec![p.event]
        } else {
            vec![]
        }
    }
}
