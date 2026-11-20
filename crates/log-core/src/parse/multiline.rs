use crate::model::{Event, FRAME_BYTES, Level};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
enum Kind {
    Go,
    Python,
    Node,
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
    late: Option<(String, Kind, Instant)>,
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
        if text.starts_with("panic:") || text.starts_with("fatal error:") {
            Some(Kind::Go)
        } else if text.starts_with("Traceback (most recent call last):") {
            Some(Kind::Python)
        } else if text.contains("Error:") || text.starts_with("Error") {
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
            Kind::Node => {
                text.trim_start().starts_with("at ") || text.trim_start().starts_with("Caused by:")
            }
        }
    }
    pub fn push(&mut self, mut event: Event, now: Instant) -> Vec<Event> {
        let mut ready = self.flush_due(now);
        if let Some(p) = &mut self.pending {
            if Self::continuation(p.kind, &event.message)
                && p.lines < 200
                && p.event.raw.len() + event.raw.len() < self.limit
                && p.event.message.len() + event.message.len() < self.limit
            {
                p.event.raw.push('\n');
                p.event.raw.push_str(&event.raw);
                p.event.message.push('\n');
                p.event.message.push_str(&event.message);
                p.event.physical.extend(event.physical);
                p.event.quality.truncated |= event.quality.truncated;
                p.lines += 1;
                p.touched = now;
                return ready;
            }
            ready.extend(self.flush(now));
        }
        if let Some((id, kind, until)) = &self.late {
            if now <= *until && Self::continuation(*kind, &event.message) {
                event.group_id = Some(id.clone());
                event.level = Level::Error;
                event.quality.continuation = true;
                ready.push(event);
                return ready;
            }
            self.late = None;
        }
        let kind = Self::kind(&event.message);
        if let Some(kind) = kind {
            event.level = Level::Error;
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
            self.flush(now)
        } else {
            vec![]
        }
    }
    pub fn flush(&mut self, now: Instant) -> Vec<Event> {
        if let Some(p) = self.pending.take() {
            self.late = p
                .event
                .group_id
                .as_ref()
                .map(|id| (id.clone(), p.kind, now + Duration::from_secs(2)));
            vec![p.event]
        } else {
            vec![]
        }
    }
}
