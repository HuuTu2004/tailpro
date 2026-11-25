pub use crate::model::ingestion::Disposition;
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
/// Consumption acknowledgement for source-local physical units. Only a contiguous
/// terminal prefix may advance a checkpoint; neither outcome is a durable ACK.
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
    pub source_metrics: BTreeMap<String, crate::model::IngestionMetrics>,
    until: Instant,
    slots: Arc<AtomicUsize>,
}
impl Snapshot {
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
        if event.raw.bytes().len() > crate::model::FRAME_BYTES
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
        let raw_bytes = (event.raw.bytes().len() + event.ending.bytes().len()) as u64;
        let m = &mut self.sources.get_mut(&source).unwrap().metrics;
        m.admission_units = m.admission_units.saturating_add(1);
        m.observed_bytes = m.observed_bytes.saturating_add(raw_bytes);
        m.physical_records = m.physical_records.saturating_add(
            if !event.checkpoint_complete
                || matches!(
                    event.ending,
                    crate::model::RecordEnding::Fragment | crate::model::RecordEnding::Interrupted
                )
            {
                0
            } else {
                event.physical.len() as u64
            },
        );
        m.raw_fallback += u64::from(matches!(
            event.evidence.decoder,
            crate::model::evidence::Decoder::Raw
        ));
        m.malformed += u64::from(event.quality.malformed);
        m.encoding_loss += u64::from(event.quality.lossy_utf8);
        m.conflicts += u64::from(event.evidence.conflict);
        m.fragments += u64::from(event.fragment.is_some());
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
            let own = self.sources.get_mut(&source).unwrap();
            own.seal();
            let victim = if own.removable(SOURCE_FLOOR, ROW_FLOOR) {
                Some(source.clone())
            } else if own.resident_bytes() > own.live_bytes() {
                // Pinned bytes belong to the producer that incurred pressure.
                // Do not drain unrelated sources to finance its expired leases.
                None
            } else {
                // At a protected straddling segment, the producer cannot reclaim
                // even one unit of its floor. Reclaim the largest unpinned excess
                // borrower instead, preserving every source's protected suffix.
                self.sources
                    .iter()
                    .filter(|(_, store)| {
                        store.removable(SOURCE_FLOOR, ROW_FLOOR)
                            && store
                                .sealed
                                .front()
                                .is_some_and(|s| Arc::strong_count(s) == 1)
                    })
                    .max_by_key(|(_, store)| store.live_bytes().saturating_sub(SOURCE_FLOOR))
                    .map(|(id, _)| id.clone())
            };
            let Some(victim) = victim else {
                self.counters.discarded += event.physical.len() as u64;
                let m = &mut self.sources.get_mut(&source).unwrap().metrics;
                m.gap_units += 1;
                m.gap_bytes = m.gap_bytes.saturating_add(raw_bytes);
                self.change(&source, ChangeKind::Gap);
                return Ok(Ack {
                    generation,
                    physical: event.physical,
                    disposition: Disposition::Gap,
                });
            };
            let store = self.sources.get_mut(&victim).unwrap();
            let removed = store.sealed.pop_front().unwrap();
            store.rows -= removed.events.len();
            store.metrics.retention_evicted_bytes =
                store.metrics.retention_evicted_bytes.saturating_add(
                    removed
                        .events
                        .iter()
                        .map(|e| (e.raw.bytes().len() + e.ending.bytes().len()) as u64)
                        .sum::<u64>(),
                );
            self.counters.evicted += removed.events.len() as u64;
            drop(removed);
            self.change(&victim, ChangeKind::Evict);
        }
        let physical = event.physical.clone();
        let store = self.sources.get_mut(&source).unwrap();
        store.resident.fetch_add(bytes, Ordering::Relaxed);
        store.tail_bytes += bytes;
        store.metrics.accepted_units += 1;
        store.metrics.represented_bytes = store.metrics.represented_bytes.saturating_add(raw_bytes);
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
            source_metrics: self
                .sources
                .iter()
                .map(|(id, s)| (id.clone(), s.metrics.clone()))
                .collect(),
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
