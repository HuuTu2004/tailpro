//! Ingestion schema v1 primitives. Payload ownership stays with the reader/store;
//! these views do not copy bytes or authorize an external projection.
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

/// A half-open range of payload bytes within one logical record, not a file cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PayloadRange {
    start: u64,
    end: u64,
}
impl PayloadRange {
    pub fn new(start: u64, end: u64) -> Option<Self> {
        (start <= end).then_some(Self { start, end })
    }
    pub fn start(self) -> u64 {
        self.start
    }
    pub fn end(self) -> u64 {
        self.end
    }
    pub fn len(self) -> u64 {
        self.end - self.start
    }
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
    /// The caller must first verify matching source, generation and record identity.
    pub fn followed_by(self, next: Self) -> bool {
        self.end == next.start
    }
}

/// Original bytes of one bounded payload unit. Terminators and transport positions
/// belong to the adapter; a fragment range is never a resume token by itself.
/// Deliberately not Serialize: raw data must pass application output policy.
#[derive(Debug)]
pub struct OriginalPayload<'a> {
    bytes: &'a [u8],
    range: PayloadRange,
}
impl<'a> OriginalPayload<'a> {
    pub fn new(bytes: &'a [u8], range: PayloadRange, byte_limit: usize) -> Option<Self> {
        (bytes.len() <= byte_limit.min(super::FRAME_BYTES)
            && u64::try_from(bytes.len()).ok()? == range.len())
        .then_some(Self { bytes, range })
    }
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
    pub fn range(&self) -> PayloadRange {
        self.range
    }
}

/// Canonical signed nanoseconds since Unix epoch. JSON is a decimal string, never
/// a floating-point number. Parsing source timestamp formats belongs to the decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct UnixNanos<'a>(&'a str);
impl<'a> UnixNanos<'a> {
    pub fn new(value: &'a str) -> Option<Self> {
        let digits = value.strip_prefix('-').unwrap_or(value);
        if value.len() > 40
            || digits.is_empty()
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || (digits.len() > 1 && digits.starts_with('0'))
            || value == "-0"
        {
            return None;
        }
        value.parse::<i128>().ok()?;
        Some(Self(value))
    }
    pub fn as_str(self) -> &'a str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeBasis {
    Event,
    Transport,
    Observed,
}

/// Distinct clocks; a present but unresolved event literal does not replace a
/// resolved transport timestamp. Source literals/provenance are retained separately.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct EventTimes<'a> {
    pub event: Option<UnixNanos<'a>>,
    pub transport: Option<UnixNanos<'a>>,
    pub observed: UnixNanos<'a>,
}
impl<'a> EventTimes<'a> {
    pub fn display(self) -> (UnixNanos<'a>, TimeBasis) {
        if let Some(time) = self.event {
            (time, TimeBasis::Event)
        } else if let Some(time) = self.transport {
            (time, TimeBasis::Transport)
        } else {
            (self.observed, TimeBasis::Observed)
        }
    }
}

/// Evidence strength, not a statistical probability or a guarantee of correctness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldOrigin {
    Explicit,
    Profile,
    Heuristic,
    Unknown,
}

/// A numeric severity is uninterpretable until its namespace/profile is known.
/// This type does not auto-detect or map any numeric level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeverityNamespace {
    Text,
    Pino,
    Syslog,
    OpenTelemetry,
    CustomProfile,
    Unknown,
}

/// Exactly one terminal outcome per atomic admission unit. Accepted means RAM
/// admission, not durable storage, full payload fidelity, or subsequent retention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    Accepted,
    Gap,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EpochUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
}
