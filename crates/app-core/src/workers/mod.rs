pub(crate) mod parser;
use crate::api::{Error, Result, ViewNotice};
use log_core::{
    engine::{Ack, Engine, Snapshot},
    model::Event,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use tokio::sync::{Semaphore, oneshot};
use tokio_util::sync::CancellationToken;

struct Queued {
    event: Event,
    ack: oneshot::Sender<Result<Ack>>,
    _bytes: tokio::sync::OwnedSemaphorePermit,
}
struct Lane {
    source: String,
    generation: u64,
    queue: mpsc::Receiver<Queued>,
    deficit: usize,
    head: Option<Queued>,
    retire_at: Option<Instant>,
}
enum Control {
    Wake,
    Register(
        String,
        u64,
        mpsc::Receiver<Queued>,
        oneshot::Sender<Result<()>>,
    ),
    Snapshot(oneshot::Sender<Result<(Snapshot, log_core::engine::Stats)>>),
    OpenView(String, oneshot::Sender<Result<()>>),
    CloseView(String),
    Notice(String, Option<String>, oneshot::Sender<Result<ViewNotice>>),
}
#[derive(Clone)]
pub struct EngineWorker {
    control: mpsc::SyncSender<Control>,
    cancel: CancellationToken,
    pub(crate) cpu: Arc<Semaphore>,
    pub(crate) background: Arc<Semaphore>,
    retired: Arc<std::sync::Mutex<Vec<String>>>,
}
#[derive(Clone)]
pub struct Ingest {
    source: String,
    generation: u64,
    cpu: Arc<Semaphore>,
    background: Arc<Semaphore>,
    wake: mpsc::SyncSender<Control>,
    sender: mpsc::SyncSender<Queued>,
    bytes: Arc<Semaphore>,
    cancel: CancellationToken,
    pub(crate) parser: Arc<std::sync::RwLock<Arc<parser::Parser>>>,
}
impl Ingest {
    /// Fragment mode never interprets a suffix as a fresh structured record.
    pub fn raw(&self, bytes: Vec<u8>, stream: log_core::model::Stream, physical: u64) -> Event {
        log_core::parse::raw(
            bytes,
            &self.source,
            self.generation,
            stream,
            physical,
            crate::application::now_ms(),
        )
    }
    pub async fn normalize(
        &self,
        bytes: Vec<u8>,
        stream: log_core::model::Stream,
        physical: u64,
        cancel: &CancellationToken,
    ) -> Result<Event> {
        if bytes.len() > log_core::model::FRAME_BYTES {
            return Err(Error::Limit("normalization frame"));
        }
        let observed = crate::application::now_ms();
        let parser = self.parser.read().unwrap().clone();
        if bytes.len() < 4096 {
            let mut event = log_core::parse::normalize_with(
                &bytes,
                &self.source,
                self.generation,
                stream,
                physical,
                observed,
                &parser.profile,
            );
            parser.apply(&mut event);
            return Ok(event);
        }
        let background = tokio::select! {
            _=cancel.cancelled()=>return Err(Error::Cancelled),
            p=self.background.clone().acquire_owned()=>p.map_err(|_|Error::Cancelled)?
        };
        let permit = tokio::select! {
            _=cancel.cancelled()=>return Err(Error::Cancelled),
            p=self.cpu.clone().acquire_owned()=>p.map_err(|_|Error::Cancelled)?
        };
        let source = self.source.clone();
        let generation = self.generation;
        let cancel = cancel.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _background = background;
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let mut event = log_core::parse::normalize_with(
                &bytes,
                &source,
                generation,
                stream,
                physical,
                observed,
                &parser.profile,
            );
            parser.apply(&mut event);
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            Ok(event)
        })
        .await
        .map_err(|_| Error::Cancelled)?
    }
    pub async fn append(&self, event: Event) -> Result<Ack> {
        let charge = event.charged_bytes() + 1024;
        if charge > 1024 * 1024 {
            return Err(Error::Limit("source event working bytes"));
        }
        let permit = tokio::select! { _ = self.cancel.cancelled() => return Err(Error::Cancelled), p = self.bytes.clone().acquire_many_owned(charge as u32) => p.map_err(|_| Error::Cancelled)? };
        let (tx, rx) = oneshot::channel();
        self.sender
            .try_send(Queued {
                event,
                ack: tx,
                _bytes: permit,
            })
            .map_err(|_| Error::Limit("source fair queue"))?;
        let _ = self.wake.try_send(Control::Wake);
        tokio::select! { _ = self.cancel.cancelled() => Err(Error::Cancelled), ack = rx => ack.map_err(|_| Error::Cancelled)? }
    }
}
impl EngineWorker {
    pub fn start(cancel: CancellationToken) -> Self {
        let (tx, rx) = mpsc::sync_channel(16);
        let stop = cancel.clone();
        let retired = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let retired_worker = retired.clone();
        std::thread::Builder::new()
            .name("tailpro-engine".into())
            .spawn(move || {
                let mut engine = Engine::new(format!("{:x}", rand::random::<u64>()));
                let mut lanes = VecDeque::<Lane>::new();
                let mut published = Instant::now();
                let mut views = HashMap::<
                    String,
                    (log_core::engine::Subscription, Option<ViewNotice>, Instant),
                >::new();
                while !stop.is_cancelled() {
                    let control = if lanes.iter().all(|l| l.head.is_none()) {
                        rx.recv_timeout(Duration::from_millis(2)).ok()
                    } else {
                        rx.try_recv().ok()
                    };
                    if let Some(control) = control {
                        match control {
                            Control::Wake => {}
                            Control::OpenView(id, reply) => {
                                views.retain(|_, (_, _, touched)| {
                                    touched.elapsed() < Duration::from_secs(60)
                                });
                                let result = if views.len() >= 4 {
                                    Err(Error::Limit("4 desktop views"))
                                } else {
                                    engine
                                        .capture()
                                        .map(|(snapshot, subscription)| {
                                            drop(snapshot);
                                            views.insert(id, (subscription, None, Instant::now()));
                                        })
                                        .map_err(Error::from)
                                };
                                let _ = reply.send(result);
                            }
                            Control::CloseView(id) => {
                                views.remove(&id);
                            }
                            Control::Notice(id, ack, reply) => {
                                let result = if let Some((subscription, pending, touched)) =
                                    views.get_mut(&id)
                                {
                                    *touched = Instant::now();
                                    subscription.touch();
                                    if pending.as_ref().is_some_and(|n| {
                                        Some(n.revision.as_str()) != ack.as_deref()
                                    }) {
                                        Ok(pending.as_ref().unwrap().clone())
                                    } else {
                                        engine
                                            .catch_up_subscription(subscription)
                                            .map(|changes| {
                                                let stats = engine.stats();
                                                let mut gaps: Vec<String> = changes
                                                    .changes
                                                    .iter()
                                                    .filter(|c| {
                                                        matches!(
                                                            c.kind,
                                                            log_core::engine::ChangeKind::Gap
                                                        )
                                                    })
                                                    .map(|c| c.source.clone())
                                                    .collect();
                                                gaps.sort();
                                                gaps.dedup();
                                                let notice = ViewNotice {
                                                    schema_version: 1,
                                                    revision: changes.fence,
                                                    accepted: stats.counters.accepted.to_string(),
                                                    discarded: stats.counters.discarded.to_string(),
                                                    evicted: stats.counters.evicted.to_string(),
                                                    retained_rows: stats.live_rows.to_string(),
                                                    resident_bytes: stats
                                                        .resident_bytes
                                                        .to_string(),
                                                    complete: changes.complete,
                                                    gap_sources: gaps,
                                                };
                                                *pending = Some(notice.clone());
                                                notice
                                            })
                                            .map_err(Error::from)
                                    }
                                } else {
                                    Err(Error::Invalid("View expired; reopen viewer".into()))
                                };
                                let _ = reply.send(result);
                            }
                            Control::Register(source, generation, queue, reply) => {
                                let result =
                                    engine.register(&source, generation).map_err(Error::from);
                                if result.is_ok() {
                                    lanes.push_back(Lane {
                                        source,
                                        generation,
                                        queue,
                                        deficit: 0,
                                        head: None,
                                        retire_at: None,
                                    });
                                }
                                let _ = reply.send(result);
                            }
                            Control::Snapshot(reply) => {
                                let snapshot = engine
                                    .snapshot()
                                    .map(|s| (s, engine.stats()))
                                    .map_err(Error::from);
                                let _ = reply.send(snapshot);
                            }
                        }
                    }
                    for source in retired_worker.lock().unwrap().drain(..) {
                        if let Some(lane) = lanes.iter_mut().find(|l| l.source == source) {
                            lane.retire_at = Some(Instant::now() + Duration::from_secs(60));
                        }
                    }
                    let count = lanes.len();
                    for _ in 0..count {
                        let mut lane = lanes.pop_front().unwrap();
                        lane.deficit = (lane.deficit + 64 * 1024).min(2 * 1024 * 1024);
                        if lane.head.is_none() {
                            lane.head = lane.queue.try_recv().ok();
                        }
                        for _ in 0..32 {
                            let Some(next) = lane.head.as_ref() else {
                                break;
                            };
                            let bytes = next.event.charged_bytes() + 1024;
                            if bytes > lane.deficit {
                                break;
                            }
                            let next = lane.head.take().unwrap();
                            lane.deficit -= bytes;
                            debug_assert_eq!(next.event.source, lane.source);
                            let result = engine
                                .append(next.event, lane.generation)
                                .map_err(Error::from);
                            let _ = next.ack.send(result);
                            lane.head = lane.queue.try_recv().ok();
                        }
                        if lane.retire_at.is_some_and(|at| Instant::now() >= at)
                            && engine.retire(&lane.source)
                        {
                            continue;
                        }
                        lanes.push_back(lane);
                    }
                    if published.elapsed() >= Duration::from_millis(50) {
                        views.retain(|_, (_, _, touched)| {
                            touched.elapsed() < Duration::from_secs(60)
                        });
                        engine.seal();
                        engine.publish();
                        published = Instant::now();
                    }
                }
            })
            .expect("engine thread");
        Self {
            control: tx,
            cancel,
            cpu: Arc::new(Semaphore::new(2)),
            background: Arc::new(Semaphore::new(1)),
            retired,
        }
    }
    pub async fn register(&self, source: &str, generation: u64) -> Result<Ingest> {
        let (tx, rx) = mpsc::sync_channel(256);
        let (reply, done) = oneshot::channel();
        self.control
            .try_send(Control::Register(source.into(), generation, rx, reply))
            .map_err(|_| Error::Limit("engine control"))?;
        done.await.map_err(|_| Error::Cancelled)??;
        Ok(Ingest {
            source: source.into(),
            generation,
            cpu: self.cpu.clone(),
            background: self.background.clone(),
            wake: self.control.clone(),
            sender: tx,
            bytes: Arc::new(Semaphore::new(1024 * 1024)),
            cancel: self.cancel.clone(),
            parser: Arc::new(std::sync::RwLock::new(Arc::new(parser::Parser::default()))),
        })
    }
    pub fn retire(&self, source: &str) -> Result<()> {
        let mut retired = self.retired.lock().unwrap();
        if retired.iter().any(|id| id == source) {
            return Ok(());
        }
        if retired.len() >= 8 {
            return Err(Error::Limit("source cleanup mailbox"));
        }
        retired.push(source.into());
        let _ = self.control.try_send(Control::Wake);
        Ok(())
    }
    pub async fn snapshot(&self) -> Result<Snapshot> {
        let (tx, rx) = oneshot::channel();
        self.control
            .try_send(Control::Snapshot(tx))
            .map_err(|_| Error::Limit("engine control"))?;
        Ok(rx.await.map_err(|_| Error::Cancelled)??.0)
    }
    pub async fn open_view(&self, id: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.control
            .try_send(Control::OpenView(id, tx))
            .map_err(|_| Error::Limit("engine control"))?;
        rx.await.map_err(|_| Error::Cancelled)?
    }
    pub async fn close_view(&self, id: String) -> Result<()> {
        self.control
            .try_send(Control::CloseView(id))
            .map_err(|_| Error::Limit("engine control"))
    }
    pub async fn view_notice(&self, id: String, ack: Option<String>) -> Result<ViewNotice> {
        let (tx, rx) = oneshot::channel();
        self.control
            .try_send(Control::Notice(id, ack, tx))
            .map_err(|_| Error::Limit("engine control"))?;
        rx.await.map_err(|_| Error::Cancelled)?
    }
}
