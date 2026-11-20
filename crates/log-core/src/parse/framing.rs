use crate::model::FRAME_BYTES;

pub struct Frame {
    pub bytes: Vec<u8>,
    pub end_offset: u64,
    pub truncated: bool,
    pub guard: Vec<u8>,
}
/// Retains at most one bounded record. Offset always counts original bytes.
pub struct Framer {
    buffer: Vec<u8>,
    limit: usize,
    offset: u64,
    truncated: bool,
    guard: std::collections::VecDeque<u8>,
}
impl Default for Framer {
    fn default() -> Self {
        Self {
            buffer: vec![],
            limit: FRAME_BYTES,
            offset: 0,
            truncated: false,
            guard: Default::default(),
        }
    }
}
impl Framer {
    pub fn bounded(limit: usize) -> Self {
        Self {
            limit: limit.clamp(1, FRAME_BYTES),
            ..Self::default()
        }
    }

    pub fn at(offset: u64) -> Self {
        Self {
            offset,
            ..Self::default()
        }
    }
    pub fn seed_guard(&mut self, bytes: &[u8]) {
        self.guard = bytes
            .iter()
            .rev()
            .take(64)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
    }
    pub fn push(&mut self, chunk: &[u8], mut emit: impl FnMut(Frame)) {
        for &byte in chunk {
            self.offset += 1;
            if self.guard.len() == 64 {
                self.guard.pop_front();
            }
            self.guard.push_back(byte);
            if byte == b'\n' {
                emit(Frame {
                    bytes: std::mem::take(&mut self.buffer),
                    end_offset: self.offset,
                    truncated: std::mem::take(&mut self.truncated),
                    guard: self.guard.iter().copied().collect(),
                });
            } else if self.buffer.len() < self.limit {
                self.buffer.push(byte);
            } else {
                self.truncated = true;
            }
        }
    }
    pub fn finish(&mut self) -> Option<Frame> {
        if self.buffer.is_empty() && !self.truncated {
            return None;
        }
        Some(Frame {
            bytes: std::mem::take(&mut self.buffer),
            end_offset: self.offset,
            truncated: std::mem::take(&mut self.truncated),
            guard: self.guard.iter().copied().collect(),
        })
    }
}
