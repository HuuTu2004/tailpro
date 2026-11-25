//! Original bytes with a display overlay only for invalid UTF-8.
use serde::{Serialize, Serializer};
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq)]
pub struct RawPayload {
    bytes: Vec<u8>,
    display: Option<String>,
}
impl RawPayload {
    pub fn new(bytes: Vec<u8>) -> Self {
        let display = std::str::from_utf8(&bytes).err().map(|_| String::from_utf8_lossy(&bytes).into_owned());
        Self { bytes, display }
    }
    pub fn bytes(&self) -> &[u8] { &self.bytes }
    pub fn charged_bytes(&self) -> usize {
        self.bytes.capacity() + self.display.as_ref().map_or(0, String::capacity)
    }
    pub fn append(&mut self, other: &Self, separator: &[u8]) {
        self.bytes.extend_from_slice(separator);
        self.bytes.extend_from_slice(other.bytes());
        self.display = std::str::from_utf8(&self.bytes).err().map(|_| String::from_utf8_lossy(&self.bytes).into_owned());
    }
}
impl Deref for RawPayload {
    type Target = str;
    fn deref(&self) -> &str {
        self.display.as_deref().unwrap_or_else(|| std::str::from_utf8(&self.bytes).expect("validated payload"))
    }
}
impl Serialize for RawPayload {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_str(self) }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordEnding {
    Lf,
    CrLf,
    Eof,
    Interrupted,
    Fragment,
    /// Unframed callers and decoded Journal MESSAGE values have no line delimiter.
    #[default]
    None,
}
impl RecordEnding {
    pub fn bytes(self) -> &'static [u8] {
        match self { Self::Lf => b"\n", Self::CrLf => b"\r\n", _ => b"" }
    }
}

/// Identity is scoped by Event source/generation/stream; record is an absolute
/// start offset for files and a reader-local monotonic offset for Docker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fragment {
    pub record: u64,
    pub start: u64,
    pub end: u64,
    pub final_part: bool,
}
