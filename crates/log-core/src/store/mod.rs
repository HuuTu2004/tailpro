use crate::model::Event;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

pub(crate) struct Segment {
    pub events: Vec<Event>,
    pub bytes: usize,
    resident: Arc<AtomicUsize>,
}
impl Drop for Segment {
    fn drop(&mut self) {
        self.resident.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}
pub(crate) struct SourceStore {
    pub generation: u64,
    pub metrics: crate::model::IngestionMetrics,
    pub sealed: VecDeque<Arc<Segment>>,
    pub tail: Vec<Event>,
    pub tail_bytes: usize,
    pub rows: usize,
    pub resident: Arc<AtomicUsize>,
}
impl SourceStore {
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            metrics: Default::default(),
            sealed: VecDeque::new(),
            tail: vec![],
            tail_bytes: 0,
            rows: 0,
            resident: Arc::new(AtomicUsize::new(0)),
        }
    }
    pub fn seal(&mut self) {
        if self.tail.is_empty() {
            return;
        }
        let events = std::mem::take(&mut self.tail);
        let bytes = std::mem::take(&mut self.tail_bytes);
        let segment = Arc::new(Segment {
            events,
            bytes,
            resident: self.resident.clone(),
        });
        self.sealed.push_back(segment);
    }
    pub fn resident_bytes(&mut self) -> usize {
        self.resident.load(Ordering::Relaxed)
    }
    pub fn live_bytes(&self) -> usize {
        self.tail_bytes + self.sealed.iter().map(|s| s.bytes).sum::<usize>()
    }
    pub fn removable(&self, byte_floor: usize, row_floor: usize) -> bool {
        self.sealed.front().is_some_and(|s| {
            // Preserve the newest suffix fitting BOTH floor constraints; keep straddling segments.
            let after_bytes = self.live_bytes().saturating_sub(s.bytes);
            let after_rows = self.rows.saturating_sub(s.events.len());
            after_bytes >= byte_floor || after_rows >= row_floor
        })
    }
}
