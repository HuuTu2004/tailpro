use crate::api::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileCheckpoint {
    pub generation: String,
    pub device: String,
    pub inode: String,
    pub next_offset: String,
    pub guard: String,
}
#[derive(Clone, Debug)]
pub enum Position {
    Docker(String),
    Journal(String),
    File(FileCheckpoint),
    Marker,
}
struct Pending {
    end: u64,
    position: Position,
    committed: bool,
}
/// ACKs are generation-fenced and can arrive out of assembly order.
pub struct Checkpoints {
    pub generation: u64,
    next: u64,
    committed: u64,
    pending: BTreeMap<u64, Pending>,
    pub position: Option<Position>,
    replay: VecDeque<(String, u64)>,
    replay_bytes: usize,
}
impl Checkpoints {
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            next: 0,
            committed: 0,
            pending: BTreeMap::new(),
            position: None,
            replay: VecDeque::new(),
            replay_bytes: 0,
        }
    }
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
    pub fn allocate(&mut self, position: Position) -> Result<u64> {
        if self.pending.len() >= 256 {
            return Err(Error::Limit("256 outstanding physical records"));
        }
        self.next += 1;
        self.pending.insert(
            self.next,
            Pending {
                end: self.next,
                position,
                committed: false,
            },
        );
        Ok(self.next)
    }
    pub fn ack(&mut self, generation: u64, physical: &[u64]) {
        if generation != self.generation {
            return;
        }
        for seq in physical {
            if let Some(p) = self.pending.get_mut(seq) {
                p.committed = true;
            }
        }
        // Compact only committed adjacent ranges. A pending stack still fences every
        // later ACK, while a busy second stream cannot fill the ledger with ACKed rows.
        let keys: Vec<_> = self.pending.keys().copied().collect();
        let mut previous = None;
        for key in keys {
            if self.pending.get(&key).is_none_or(|p| !p.committed) {
                previous = None;
                continue;
            }
            if let Some(prev) = previous {
                let next = self.pending.remove(&key).unwrap();
                let prior = self.pending.get_mut(&prev).unwrap();
                prior.end = next.end;
                if !matches!(next.position, Position::Marker) {
                    prior.position = next.position;
                }
            } else {
                previous = Some(key);
            }
        }
        while self
            .pending
            .get(&(self.committed + 1))
            .is_some_and(|p| p.committed)
        {
            let p = self.pending.remove(&(self.committed + 1)).unwrap();
            self.committed = p.end;
            if !matches!(p.position, Position::Marker) {
                self.position = Some(p.position);
            }
        }
    }
    pub fn replay_suspected(&mut self, timestamp: &str, hash: u64) -> bool {
        let repeated = self
            .replay
            .iter()
            .any(|(t, h)| t == timestamp && *h == hash);
        while self.replay.len() >= 4096 || self.replay_bytes + timestamp.len() + 32 > 512 * 1024 {
            if let Some((t, _)) = self.replay.pop_front() {
                self.replay_bytes -= t.len() + 32;
            } else {
                break;
            }
        }
        self.replay_bytes += timestamp.len() + 32;
        self.replay.push_back((timestamp.into(), hash));
        repeated
    }
    /// Uncommitted assemblies are replayed after reconnect; committed position survives.
    pub fn reconnect(&mut self) {
        self.pending.clear();
        self.next = self.committed;
    }
}
