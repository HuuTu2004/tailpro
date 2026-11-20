use crate::{
    model::{Counters, Event, MAX_SOURCES, MIB},
    store::{Segment, SourceStore},
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

const SOURCE_BYTES: usize = 352 * MIB;
const SOURCE_FLOOR: usize = 16 * MIB;
const ROW_FLOOR: usize = 10_000;
const ROW_CAP: usize = 500_000;
const SEGMENT_BYTES: usize = 256 * 1024;
const SEGMENT_ROWS: usize = 128;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("source limit/reservation unavailable")]
    SourceLimit,
    #[error("unknown source or stale generation")]
    Stale,
    #[error("snapshot lease limit")]
    LeaseLimit,
    #[error("snapshot expired")]
    Expired,
    #[error("event exceeds normalization bounds")]
    InvalidEvent,
}
#[derive(Clone, Debug, Serialize)]
pub struct Change {
    pub revision: u64,
    pub source: String,
    pub kind: ChangeKind,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Append,
    Evict,
    Clear,
    Gap,
}
#[derive(Serialize)]
pub struct CatchUp {
    pub changes: Vec<Change>,
    pub fence: String,
    pub complete: bool,
}
#[derive(Debug)]
pub enum Disposition {
    Accepted,
    Gap,
}
pub struct Ack {
    pub generation: u64,
    pub physical: Vec<u64>,
    pub disposition: Disposition,
}
#[derive(Serialize)]
pub struct Stats {
    pub counters: Counters,
    pub resident_bytes: usize,
    pub live_rows: usize,
    pub revision: String,
}

/// Leases share immutable segments. Expired handles still charge resident memory until dropped.
pub struct Snapshot {
    segments: Vec<Arc<Segment>>,
    pub fence: u64,
    pub watermark: u64,
    pub source_losses: BTreeMap<String, u64>,
    until: Instant,
    slots: Arc<AtomicUsize>,
}
impl Snapshot {
    /// Read a bounded suffix per source, then merge by local ingest order.
    pub fn recent(&self, limit: usize) -> Result<Vec<&Event>, Error> {
        if Instant::now() > self.until {
            return Err(Error::Expired);
        }
        if limit > 200 {
            return Err(Error::LeaseLimit);
        }
        let mut counts = BTreeMap::<&str, usize>::new();
        let mut events = Vec::new();
        for segment in self.segments.iter().rev() {
            let Some(first) = segment.events.first() else {
                continue;
            };
            let count = counts.entry(first.source.as_str()).or_default();
            if *count >= limit {
                continue;
            }
            for event in segment.events.iter().rev().take(limit - *count) {
                events.push(event);
                *count += 1;
            }
        }
        events.sort_unstable_by_key(|e| std::cmp::Reverse(e.sequence));
        events.truncate(limit);
        events.reverse();
        Ok(events)
    }
    pub fn events(&self) -> Result<impl Iterator<Item = &Event>, Error> {
        if Instant::now() > self.until {
            return Err(Error::Expired);
        }
        Ok(self.segments.iter().flat_map(|s| s.events.iter()))
    }
}
impl Drop for Snapshot {
    fn drop(&mut self) {
        self.slots.fetch_sub(1, Ordering::Relaxed);
    }
}

struct Consumer {
    active: AtomicBool,
    notice: AtomicU64,
    expires: Mutex<Instant>,
}
/// One coalesced metadata notice; consumers recover changes from the bounded journal.
pub struct Subscription {
    consumer: Arc<Consumer>,
    fence: u64,
}
impl Subscription {
    pub fn touch(&self) {
        *self.consumer.expires.lock().unwrap() = Instant::now() + Duration::from_secs(60);
    }
    pub fn notice(&self) -> Result<Option<String>, Error> {
        if !self.consumer.active.load(Ordering::Relaxed)
            || Instant::now() > *self.consumer.expires.lock().unwrap()
        {
            return Err(Error::Expired);
        }
        let revision = self.consumer.notice.load(Ordering::Acquire);
        Ok((revision > self.fence).then(|| revision.to_string()))
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.consumer.active.store(false, Ordering::Relaxed);
    }
}

/// Mutated only by its owner. I/O and queries never lock this object.
pub struct Engine {
    sources: BTreeMap<String, SourceStore>,
    counters: Counters,
    revision: u64,
    sequence: u64,
    source_losses: BTreeMap<String, u64>,
    journal: VecDeque<Change>,
    leases: Arc<AtomicUsize>,
    epoch: String,
    consumers: Vec<Arc<Consumer>>,
}
impl Engine {
    pub fn new(epoch: String) -> Self {
        Self {
            sources: BTreeMap::new(),
            counters: Counters::default(),
            revision: 0,
            sequence: 0,
            source_losses: BTreeMap::new(),
            journal: VecDeque::new(),
            leases: Arc::new(AtomicUsize::new(0)),
            epoch,
            consumers: vec![],
        }
    }
    pub fn register(&mut self, source: &str, generation: u64) -> Result<(), Error> {
        if self.sources.contains_key(source) {
            return Err(Error::Stale);
        }
        if self.sources.len() >= MAX_SOURCES || source.len() > 128 {
            return Err(Error::SourceLimit);
        }
        self.sources
            .insert(source.into(), SourceStore::new(generation));
        Ok(())
    }
    fn change(&mut self, source: &str, kind: ChangeKind) {
        self.revision += 1;
        if matches!(
            kind,
            ChangeKind::Evict | ChangeKind::Clear | ChangeKind::Gap
        ) {
            self.source_losses.insert(source.into(), self.revision);
        }
        if self.journal.len() >= 16_384 {
            self.journal.pop_front();
        }
        self.journal.push_back(Change {
            revision: self.revision,
            source: source.into(),
            kind,
        });
    }
    fn fits(&mut self, source: &str, extra: usize) -> bool {
        let mut charged = 0;
        let mut rows = 0;
        for (id, store) in &mut self.sources {
            let own = id == source;
            charged += (store.resident_bytes() + if own { extra } else { 0 }).max(SOURCE_FLOOR);
            rows += (store.rows + usize::from(own)).max(ROW_FLOOR);
        }
        charged <= SOURCE_BYTES && rows <= ROW_CAP
    }
    pub fn append(&mut self, mut event: Event, generation: u64) -> Result<Ack, Error> {
        if event.raw.len() > crate::model::FRAME_BYTES
            || event.message.len() > crate::model::FRAME_BYTES
            || event.physical.len() > 200
            || event.physical.is_empty()
            || event.generation.len() > 128
            || event.observed_at_ms.len() > 32
            || event.duration_ms.is_some_and(|n| !n.is_finite())
        {
            return Err(Error::InvalidEvent);
        }

        let source = event.source.clone();
        if self
            .sources
            .get(&source)
            .is_none_or(|s| s.generation != generation)
        {
            return Err(Error::Stale);
        }
        // Continuity markers must survive filtered reads as coverage metadata.
        // A level:error/trace predicate may exclude the marker itself.
        if event.stream == crate::model::Stream::System && event.quality.uncertain {
            self.change(&source, ChangeKind::Gap);
        }
        self.counters.received += event.physical.len() as u64;
        self.sequence += 1;
        event.sequence = self.sequence;
        event.id = format!("{}-{:x}", self.epoch, self.sequence);
        // Reserve slack for vector growth, Arc, deque/weak bookkeeping and journal metadata.
        let bytes = event.charged_bytes() + std::mem::size_of::<Event>() + 256;
        while !self.fits(&source, bytes) {
            let store = self.sources.get_mut(&source).unwrap();
            store.seal();
            if !store.removable(SOURCE_FLOOR, ROW_FLOOR) {
                self.counters.discarded += event.physical.len() as u64;
                self.change(&source, ChangeKind::Gap);
                return Ok(Ack {
                    generation,
                    physical: event.physical,
                    disposition: Disposition::Gap,
                });
            }
            let removed = store.sealed.pop_front().unwrap();
            store.rows -= removed.events.len();
            self.counters.evicted += removed.events.len() as u64;
            drop(removed);
            self.change(&source, ChangeKind::Evict);
        }
        let physical = event.physical.clone();
        let store = self.sources.get_mut(&source).unwrap();
        store.resident.fetch_add(bytes, Ordering::Relaxed);
        store.tail_bytes += bytes;
        store.tail.push(event);
        store.rows += 1;
        if store.tail_bytes >= SEGMENT_BYTES || store.tail.len() >= SEGMENT_ROWS {
            store.seal();
        }
        self.counters.accepted += 1;
        self.change(&source, ChangeKind::Append);
        Ok(Ack {
            generation,
            physical,
            disposition: Disposition::Accepted,
        })
    }
    pub fn inspect(&self, id: &str) -> Option<&Event> {
        self.sources
            .values()
            .flat_map(|s| {
                s.sealed
                    .iter()
                    .flat_map(|segment| segment.events.iter())
                    .chain(s.tail.iter())
            })
            .find(|e| e.id == id)
    }
    pub fn retire(&mut self, source: &str) -> bool {
        if let Some(store) = self.sources.get_mut(source) {
            store.seal();
            store.sealed.clear();
            store.rows = 0;
            if store.resident_bytes() != 0 {
                return false;
            }
        }
        self.sources.remove(source);
        self.source_losses.remove(source);
        self.change(source, ChangeKind::Clear);
        self.source_losses.remove(source);
        true
    }
    pub fn seal(&mut self) {
        for s in self.sources.values_mut() {
            s.seal();
        }
    }
    pub fn capture(&mut self) -> Result<(Snapshot, Subscription), Error> {
        self.consumers.retain(|c| {
            c.active.load(Ordering::Relaxed) && Instant::now() < *c.expires.lock().unwrap()
        });
        if self.consumers.len() >= 4 {
            return Err(Error::LeaseLimit);
        }
        let snapshot = self.snapshot()?;
        let consumer = Arc::new(Consumer {
            active: AtomicBool::new(true),
            notice: AtomicU64::new(snapshot.fence),
            expires: Mutex::new(Instant::now() + Duration::from_secs(60)),
        });
        self.consumers.push(consumer.clone());
        let fence = snapshot.fence;
        Ok((snapshot, Subscription { consumer, fence }))
    }
    pub fn catch_up_subscription(&self, subscription: &mut Subscription) -> Result<CatchUp, Error> {
        subscription.notice()?;
        let changes = self.catch_up(subscription.fence);
        subscription.fence = self.revision;
        Ok(changes)
    }
    /// Called on the publication tick, not once per incoming record.
    pub fn publish(&mut self) {
        self.consumers.retain(|c| {
            c.active.load(Ordering::Relaxed) && Instant::now() < *c.expires.lock().unwrap()
        });
        for consumer in &self.consumers {
            consumer.notice.store(self.revision, Ordering::Release);
        }
    }
    pub fn snapshot(&mut self) -> Result<Snapshot, Error> {
        if self.leases.load(Ordering::Relaxed) >= 8 {
            return Err(Error::LeaseLimit);
        }
        self.seal();
        let segments = self
            .sources
            .values()
            .flat_map(|s| s.sealed.iter().cloned())
            .collect();
        self.leases.fetch_add(1, Ordering::Relaxed);
        Ok(Snapshot {
            segments,
            fence: self.revision,
            watermark: self.sequence,
            source_losses: self.source_losses.clone(),
            until: Instant::now() + Duration::from_secs(60),
            slots: self.leases.clone(),
        })
    }
    /// Captured on the same single writer turn as the consumer's fence installation.
    pub fn catch_up(&self, after: u64) -> CatchUp {
        CatchUp {
            complete: after <= self.revision
                && self
                    .journal
                    .front()
                    .is_none_or(|c| after.saturating_add(1) >= c.revision),
            changes: self
                .journal
                .iter()
                .filter(|c| c.revision > after)
                .cloned()
                .collect(),
            fence: self.revision.to_string(),
        }
    }
    pub fn stats(&mut self) -> Stats {
        Stats {
            counters: self.counters.clone(),
            resident_bytes: self
                .sources
                .values_mut()
                .map(SourceStore::resident_bytes)
                .sum(),
            live_rows: self.sources.values().map(|s| s.rows).sum(),
            revision: self.revision.to_string(),
        }
    }
}
