use serde::{Deserialize, Serialize};

pub const FRAME_BYTES: usize = 256 * 1024;
pub const MAX_SOURCES: usize = 8;
pub const MIB: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stream {
    Stdout,
    Stderr,
    File,
    Journal,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Unknown,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Quality {
    pub malformed: bool,
    pub truncated: bool,
    pub lossy_utf8: bool,
    pub continuation: bool,
    pub uncertain: bool,
    pub replay_suspected: bool,
}

#[derive(Debug, Serialize)]
pub struct Event {
    pub id: String,
    pub source: String,
    pub generation: String,
    pub stream: Stream,
    pub group_id: Option<String>,
    pub observed_at_ms: String,
    pub event_time: Option<String>,
    pub level: Level,
    pub message: String,
    pub raw: String,
    pub trace_id: Option<String>,
    pub order_id: Option<String>,
    pub user_id: Option<String>,
    pub duration_ms: Option<f64>,
    pub quality: Quality,
    #[serde(skip)]
    pub physical: Vec<u64>,
    #[serde(skip)]
    pub sequence: u64,
}
impl Event {
    pub fn charged_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.id.capacity()
            + self.source.capacity()
            + self.generation.capacity()
            + self.group_id.as_ref().map_or(0, String::capacity)
            + self.observed_at_ms.capacity()
            + self.event_time.as_ref().map_or(0, String::capacity)
            + self.message.capacity()
            + self.raw.capacity()
            + self.trace_id.as_ref().map_or(0, String::capacity)
            + self.order_id.as_ref().map_or(0, String::capacity)
            + self.user_id.as_ref().map_or(0, String::capacity)
            + self.physical.capacity() * 8
    }
}

#[derive(Default, Clone, Debug, Serialize)]
pub struct Counters {
    pub received: u64,
    pub accepted: u64,
    pub discarded: u64,
    pub evicted: u64,
    pub delivered: u64,
}
