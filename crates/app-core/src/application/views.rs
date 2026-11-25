use super::*;
use log_core::query::QueryPlan;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

struct JobSlot {
    generation: u32,
    cancel: CancellationToken,
}
struct View {
    page: JobSlot,
    detail: JobSlot,
    touched: Instant,
}
#[derive(Clone)]
pub(super) struct Views {
    entries: Arc<Mutex<HashMap<String, View>>>,
    pending: Arc<Semaphore>,
}
impl Views {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            pending: Arc::new(Semaphore::new(4)),
        }
    }
    pub fn cancel_all(&self) {
        for (_, view) in self.entries.lock().unwrap().drain() {
            view.page.cancel.cancel();
            view.detail.cancel.cancel();
        }
    }
    pub(super) fn begin(
        &self,
        id: &str,
        generation: u32,
        detail: bool,
    ) -> Result<CancellationToken> {
        let mut views = self.entries.lock().unwrap();
        views.retain(|_, v| {
            if v.touched.elapsed() < Duration::from_secs(60) {
                true
            } else {
                v.page.cancel.cancel();
                v.detail.cancel.cancel();
                false
            }
        });
        let view = views
            .get_mut(id)
            .ok_or_else(|| Error::Invalid("View expired; reopen viewer".into()))?;
        let slot = if detail {
            &mut view.detail
        } else {
            &mut view.page
        };
        if generation <= slot.generation {
            return Err(Error::Cancelled);
        }
        slot.cancel.cancel();
        slot.generation = generation;
        slot.cancel = CancellationToken::new();
        view.touched = Instant::now();
        Ok(slot.cancel.clone())
    }
    fn touch(&self, id: &str) -> Result<()> {
        let mut entries = self.entries.lock().unwrap();
        let v = entries
            .get_mut(id)
            .ok_or_else(|| Error::Invalid("View expired; reopen viewer".into()))?;
        if v.touched.elapsed() >= Duration::from_secs(60) {
            return Err(Error::Invalid("View expired; reopen viewer".into()));
        }
        v.touched = Instant::now();
        Ok(())
    }
    async fn permit(
        &self,
        engine: &EngineWorker,
        cancel: &CancellationToken,
        deadline: Instant,
    ) -> Result<OwnedSemaphorePermit> {
        let _pending = self
            .pending
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Limit("4 pending CPU jobs"))?;
        tokio::select! {
            _ = cancel.cancelled() => Err(Error::Cancelled),
            _ = tokio::time::sleep_until(deadline.into()) => Err(Error::Timeout),
            p = engine.cpu.clone().acquire_owned() => p.map_err(|_| Error::Cancelled),
        }
    }
}
fn check(cancel: &CancellationToken, deadline: Instant) -> Result<()> {
    if cancel.is_cancelled() {
        Err(Error::Cancelled)
    } else if Instant::now() >= deadline {
        Err(Error::Timeout)
    } else {
        Ok(())
    }
}
fn bounded(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
fn query_error(e: log_core::query::QueryError) -> Error {
    Error::Invalid(e.to_string())
}

impl AppHandle {
    pub async fn open_view(&self) -> Result<String> {
        let id = format!("view-{:x}", rand::random::<u64>());
        // Reserve application and owner slots together without holding a lock across await.
        {
            let mut entries = self.views.entries.lock().unwrap();
            entries.retain(|_, v| {
                if v.touched.elapsed() < Duration::from_secs(60) {
                    true
                } else {
                    v.page.cancel.cancel();
                    v.detail.cancel.cancel();
                    false
                }
            });
            if entries.len() >= 4 {
                return Err(Error::Limit("4 desktop views"));
            }
            entries.insert(
                id.clone(),
                View {
                    page: JobSlot {
                        generation: 0,
                        cancel: CancellationToken::new(),
                    },
                    detail: JobSlot {
                        generation: 0,
                        cancel: CancellationToken::new(),
                    },
                    touched: Instant::now(),
                },
            );
        }
        if let Err(e) = self.engine.open_view(id.clone()).await {
            self.views.entries.lock().unwrap().remove(&id);
            return Err(e);
        }
        Ok(id)
    }
    pub async fn close_view(&self, view_id: &str) -> Result<()> {
        if let Some(view) = self.views.entries.lock().unwrap().remove(view_id) {
            view.page.cancel.cancel();
            view.detail.cancel.cancel();
        }
        self.engine.close_view(view_id.into()).await
    }
    pub fn cancel_page(&self, view_id: &str, generation: u32) -> Result<()> {
        let cancel = self.views.begin(view_id, generation, false)?;
        cancel.cancel();
        Ok(())
    }
    pub fn cancel_detail(&self, view_id: &str, generation: u32) -> Result<()> {
        let cancel = self.views.begin(view_id, generation, true)?;
        cancel.cancel();
        Ok(())
    }
    pub async fn view_notice(&self, view_id: &str, ack: Option<String>) -> Result<ViewNotice> {
        self.views.touch(view_id)?;
        self.engine.view_notice(view_id.into(), ack).await
    }
    pub async fn view_page(&self, request: ViewRequest) -> Result<ViewPage> {
        let started = Instant::now();
        let deadline = started + Duration::from_secs(2);
        let cancel = self
            .views
            .begin(&request.view_id, request.generation, false)?;
        if request.sources.len() > 8
            || request.sources.iter().any(|s| s.len() > 128)
            || request.before.as_ref().is_some_and(|s| s.len() > 128)
            || request.anchor.as_ref().is_some_and(|s| s.len() > 128)
        {
            return Err(Error::Limit("view request"));
        }
        let plan = QueryPlan::compile(&request.query).map_err(query_error)?;
        let hosts: HashMap<_, _> = self
            .state
            .lock()
            .unwrap()
            .contexts
            .iter()
            .map(|(id, c)| (id.clone(), c.host.clone()))
            .collect();
        let permit = self.views.permit(&self.engine, &cancel, deadline).await?;
        let snapshot = self.engine.snapshot().await?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            check(&cancel, deadline)?;
            let result = log_core::query::scan(
                &snapshot,
                &plan,
                log_core::query::ScanRequest {
                    before: request.before.as_deref(),
                    anchor: request.anchor.as_deref(),
                    sources: &request.sources,
                    deadline,
                },
                |source| hosts.get(source).map(|h| h.target.as_str()),
                || cancel.is_cancelled(),
            )?;
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let mut bytes = 4096;
            let mut events = Vec::with_capacity(result.events.len());
            let mut truncated = false;
            let mut complete = result.complete;
            // Project newest first, then restore ingest order. Byte cap keeps an older cursor contiguous.
            for e in result.events.iter().rev() {
                if cancel.is_cancelled() {
                    return Err(Error::Cancelled);
                }
                if Instant::now() >= deadline {
                    complete = false;
                    break;
                }
                let Some(host) = hosts.get(&e.source) else {
                    continue;
                };
                let message = bounded(&e.message, 2048);
                let trace_id = e.trace_id.as_ref().map(|s| bounded(s, 256));
                let mut quality = format!("{:?}", e.quality);
                if message.len() < e.message.len()
                    || e.trace_id
                        .as_ref()
                        .zip(trace_id.as_ref())
                        .is_some_and(|(full, preview)| full.len() > preview.len())
                {
                    quality.push_str("; table preview truncated; inspect raw");
                }
                let row = ViewRow {
                    id: e.id.clone(),
                    source: e.source.clone(),
                    server: bounded(&host.target, 256),
                    environment: host.environment.clone(),
                    level: format!("{:?}", e.level),
                    message,
                    time: bounded(e.event_time.as_deref().or(e.transport_time.as_deref()).unwrap_or(&e.observed_at_ms), 128),
                    time_basis: if e.event_time.is_some() {
                        "event_time"
                    } else if e.transport_time.is_some() {
                        "transport_time"
                    } else {
                        "local_observed_at"
                    }
                    .into(),
                    trace_id,
                    duration_ms: e.duration_ms,
                    quality,
                };
                let size = serde_json::to_vec(&row)?.len();
                if bytes + size > 512 * 1024 {
                    truncated = true;
                    break;
                }
                bytes += size;
                events.push(row);
            }
            events.reverse();
            let older_cursor = if result.has_older || truncated {
                events.first().map(|e| e.id.clone())
            } else {
                None
            };
            Ok(ViewPage {
                schema_version: 1,
                generation: request.generation,
                revision: snapshot.fence.to_string(),
                events,
                older_cursor,
                complete,
                truncated,
                anchor_expired: result.anchor_expired,
                scanned: result.scanned as u32,
                elapsed_us: started.elapsed().as_micros().to_string(),
            })
        })
        .await
        .map_err(|_| Error::Cancelled)?
    }
    pub async fn event_detail(&self, request: DetailRequest) -> Result<EventDetail> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let cancel = self
            .views
            .begin(&request.view_id, request.generation, true)?;
        if request.event_id.len() > 128 || request.path.len() > 64 || request.offset > 262_144 {
            return Err(Error::Limit("Inspector request"));
        }
        let permit = self.views.permit(&self.engine, &cancel, deadline).await?;
        let snapshot = self.engine.snapshot().await?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            check(&cancel, deadline)?;
            let mut event = None;
            for (i, e) in snapshot.events()?.enumerate() {
                if i % 128 == 0 {
                    check(&cancel, deadline)?;
                }
                if e.id == request.event_id {
                    event = Some(e);
                    break;
                }
            }
            let event = event
                .ok_or_else(|| Error::Invalid("Event expired from retention; requery".into()))?;
            let path: Vec<_> = request.path.iter().map(|n| *n as usize).collect();
            let children = log_core::inspect::children(&event.raw, &path, request.offset as usize);
            check(&cancel, deadline)?;
            let (nodes, more, json_error) = match children {
                Ok((nodes, more)) => (
                    nodes
                        .into_iter()
                        .map(|node| {
                            let value = bounded(node.raw, 512);
                            let key = bounded(&node.key, 256);
                            let truncated =
                                value.len() < node.raw.len() || key.len() < node.key.len();
                            InspectorNode {
                                index: node.index as u32,
                                key,
                                value,
                                kind: log_core::inspect::kind(node.raw).into(),
                                truncated,
                            }
                        })
                        .collect(),
                    more,
                    None,
                ),
                Err(e) => (vec![], false, Some(e)),
            };
            let (nodes,more,json_error)=if matches!(event.evidence.decoder,log_core::model::evidence::Decoder::Logfmt) && path.is_empty() {
                let mut fields=Vec::new();let mut count=0;
                let decoded=log_core::parse::decode::pairs(&event.raw,|key,value|{
                    if count>=request.offset as usize && fields.len()<200 {fields.push(InspectorNode{index:count as u32,key:bounded(key,256),value:serde_json::to_string(&bounded(&value,512)).unwrap_or_default(),kind:"string".into(),truncated:key.len()>256||value.len()>512});}
                    count+=1;
                });
                match decoded{Ok(())=>(fields,count>request.offset as usize+200,None),Err(e)=>(vec![],false,Some(e.into()))}
            } else {(nodes,more,json_error)};
            let mut group_parts = Vec::new();
            let mut group_parts_truncated = false;
            if let Some(group) = &event.group_id {
                for (i, e) in snapshot.events()?.enumerate() {
                    if i % 128 == 0 {
                        check(&cancel, deadline)?;
                    }
                    if e.group_id.as_ref() == Some(group) && e.source == event.source {
                        if group_parts.len() >= 200 {
                            group_parts_truncated = true;
                            break;
                        }
                        group_parts.push(e.id.clone());
                    }
                }
            }
            let mut detail = EventDetail {
                schema_version: 1,
                generation: request.generation,
                event_id: event.id.clone(),
                raw: request.include_raw.then(|| event.raw.to_string()),
                raw_hex: request.include_raw.then(||event.raw.bytes().iter().chain(event.ending.bytes()).take(16*1024).map(|b|format!("{b:02x}")).collect()),
                raw_bytes: (event.raw.bytes().len()+event.ending.bytes().len()).to_string(),
                evidence_json: serde_json::json!({"parser":event.evidence,"event_time":event.event_time,"transport_time":event.transport_time,"observed_time_ms":event.observed_at_ms,"ending":event.ending,"fragment":event.fragment.map(|f|serde_json::json!({"record":f.record.to_string(),"start":f.start.to_string(),"end":f.end.to_string(),"final":f.final_part})),"coverage":"Retained parts only; neighbors can be evicted. Fragment bodies are withheld from external consumers."}).to_string(),
                quality: format!("{:?}", event.quality),
                group_id: event.group_id.clone(),
                group_parts,
                group_parts_truncated,
                nodes,
                more,
                json_error,
            };
            // JSON escaping can expand control bytes by 6x. Bound exact IPC bytes too.
            if serde_json::to_vec(&detail)?.len() > 512 * 1024 {
                detail.raw = request.include_raw.then(|| bounded(&event.raw, 32 * 1024));
                detail
                    .quality
                    .push_str("; raw display truncated to IPC budget");
            }
            check(&cancel, deadline)?;
            Ok(detail)
        })
        .await
        .map_err(|_| Error::Cancelled)?
    }
    pub fn saved_views(&self) -> Result<Vec<SavedView>> {
        Ok(self.settings.lock().unwrap().load()?.saved_views)
    }
    pub fn save_view(&self, view: SavedView) -> Result<()> {
        if view.name.trim().is_empty()
            || view.name.len() > 80
            || view.sources.len() > 8
            || view.sources.iter().any(|s| s.len() > 128)
        {
            return Err(Error::Limit("Saved View"));
        }
        QueryPlan::compile(&view.query).map_err(query_error)?;
        let settings = self.settings.lock().unwrap();
        let mut saved = settings.load()?;
        saved.saved_views.retain(|v| v.name != view.name);
        saved.saved_views.push(view);
        settings.save(&saved)
    }
    pub fn delete_view(&self, name: &str) -> Result<()> {
        let settings = self.settings.lock().unwrap();
        let mut saved = settings.load()?;
        saved.saved_views.retain(|v| v.name != name);
        settings.save(&saved)
    }
}
