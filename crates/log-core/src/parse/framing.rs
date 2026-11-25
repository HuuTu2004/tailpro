use crate::model::{FRAME_BYTES, Fragment, RecordEnding};

pub struct Frame {
    pub bytes: Vec<u8>,
    pub end_offset: u64,
    /// Legacy compatibility: fragmentation preserves bytes rather than truncating.
    pub truncated: bool,
    pub guard: Vec<u8>,
    pub ending: RecordEnding,
    pub fragment: Option<Fragment>,
}
/// Retains one bounded fragment, emits before accepting the next payload byte.
/// Waiting for that byte keeps CRLF intact even at the capacity boundary.
pub struct Framer {
    buffer: Vec<u8>,
    limit: usize,
    offset: u64,
    record: u64,
    part_start: u64,
    fragmented: bool,
    guard: std::collections::VecDeque<u8>,
}
impl Default for Framer {
    fn default() -> Self {
        Self { buffer: vec![], limit: 64 * 1024, offset: 0, record: 0, part_start: 0, fragmented: false, guard: Default::default() }
    }
}
impl Framer {
    pub fn bounded(limit: usize) -> Self { Self { limit: limit.clamp(1, FRAME_BYTES), ..Self::default() } }
    pub fn at(offset: u64) -> Self { Self { offset, record: offset, ..Self::default() } }
    pub fn resume(offset: u64, record: u64, part_start: u64) -> Self {
        Self { record, part_start, fragmented: true, ..Self::at(offset) }
    }
    pub fn seed_guard(&mut self, bytes: &[u8]) {
        self.guard = bytes.iter().rev().take(64).copied().collect::<Vec<_>>().into_iter().rev().collect();
    }
    pub fn offset(&self) -> u64 { self.offset }
    fn frame(&mut self, ending: RecordEnding) -> Frame {
        let final_part = ending != RecordEnding::Fragment;
        let end = self.part_start + self.buffer.len() as u64;
        let fragment = self.fragmented.then_some(Fragment { record: self.record, start: self.part_start, end, final_part });
        let frame = Frame { bytes: std::mem::take(&mut self.buffer), end_offset: self.offset, truncated: false, guard: self.guard.iter().copied().collect(), ending, fragment };
        self.part_start = end;
        if final_part { self.record = self.offset; self.part_start = 0; self.fragmented = false; }
        frame
    }
    /// Overflow is terminal; no byte after the last representable position is ACKed.
    pub fn push(&mut self, chunk: &[u8], mut emit: impl FnMut(Frame)) -> Result<(), &'static str> {
        for &byte in chunk {
            if self.offset == u64::MAX { return Err("raw offset overflow"); }
            if byte != b'\n' && self.buffer.len() == self.limit {
                self.fragmented = true;
                emit(self.frame(RecordEnding::Fragment));
            }
            self.offset += 1;
            if self.guard.len() == 64 { self.guard.pop_front(); }
            self.guard.push_back(byte);
            if byte == b'\n' {
                let ending = if self.buffer.last() == Some(&b'\r') { self.buffer.pop(); RecordEnding::CrLf } else { RecordEnding::Lf };
                emit(self.frame(ending));
            } else { self.buffer.push(byte); }
        }
        Ok(())
    }
    pub fn finish(&mut self) -> Option<Frame> { self.finish_with(RecordEnding::Eof) }
    pub fn interrupt(&mut self) -> Option<Frame> { self.finish_with(RecordEnding::Interrupted) }
    fn finish_with(&mut self, ending: RecordEnding) -> Option<Frame> {
        if self.buffer.is_empty() && !self.fragmented { return None; }
        Some(self.frame(ending))
    }
}
