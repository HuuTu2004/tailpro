use super::*;
use app_protocol::*;
use log_core::query::QueryPlan;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(super) fn opaque() -> String {
    format!(
        "{:032x}{:032x}",
        rand::random::<u128>(),
        rand::random::<u128>()
    )
}
pub(super) fn denied() -> Error {
    Error::NeedsAction("Authorization expired or unavailable; reselect scope and grant".into())
}
pub(super) fn check(cancel: &CancellationToken, deadline: Instant) -> Result<()> {
    if cancel.is_cancelled() {
        Err(Error::Cancelled)
    } else if Instant::now() >= deadline {
        Err(Error::Timeout)
    } else {
        Ok(())
    }
}
pub(super) struct Grant {
    pub desktop: bool,
    pub info: GrantInfo,
    pub token: String,
    pub until: Instant,
    pub cancel: CancellationToken,
    pub calls: VecDeque<Instant>,
    pub active: usize,
}
#[derive(Clone)]
pub(super) struct TraceReference {
    pub request: ReadRequest,
    pub generations: Vec<(String, String)>,
}
pub(super) struct Reference {
    pub trace: Option<TraceReference>,
    pub handle: String,
    pub grant: String,
    pub event: String,
    pub source: String,
    pub generation: String,
    pub until: Instant,
}
impl Reference {
    pub fn charged_bytes(&self) -> usize {
        256 + self.handle.capacity()
            + self.grant.capacity()
            + self.event.capacity()
            + self.source.capacity()
            + self.generation.capacity()
            + self.trace.as_ref().map_or(0, |t| {
                let r = &t.request;
                let correlation = r.correlation.as_ref().map_or(0, |c| {
                    let (Correlation::TraceId(s) | Correlation::OrderId(s)) = c;
                    s.capacity()
                });
                512 + r.query.capacity()
                    + correlation
                    + r.sources.capacity() * std::mem::size_of::<String>()
                    + r.sources.iter().map(String::capacity).sum::<usize>()
                    + [&r.cursor, &r.evidence_ref, &r.service, &r.from_ms, &r.to_ms]
                        .into_iter()
                        .map(|v| v.as_ref().map_or(0, String::capacity))
                        .sum::<usize>()
                    + t.generations
                        .iter()
                        .map(|(s, g)| s.capacity() + g.capacity() + 64)
                        .sum::<usize>()
            })
    }
}
#[derive(Clone)]
pub(super) struct Cursor {
    pub grant: String,
    pub sources: Vec<String>,
    pub query: String,
    pub generations: Vec<(String, String)>,
    pub position: u64,
    pub revision: u64,
    pub fence: Option<u64>,
    pub backwards: bool,
    pub incomplete: bool,
    pub until: Instant,
    pub busy: bool,
}
impl Cursor {
    pub fn charged_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + 128
            + self.grant.capacity()
            + self.query.capacity()
            + self.sources.capacity() * std::mem::size_of::<String>()
            + self.sources.iter().map(String::capacity).sum::<usize>()
            + self.generations.capacity() * std::mem::size_of::<(String, String)>()
            + self
                .generations
                .iter()
                .map(|(a, b)| a.capacity() + b.capacity())
                .sum::<usize>()
    }
}
pub(super) struct Prepared {
    pub preview: PreparedPreview,
    pub until: Instant,
    pub epoch: u64,
    pub provider: Option<ExplainProvider>,
    pub evidence: Vec<String>,
}
pub(super) struct OutputState {
    pub grants: HashMap<String, Grant>,
    pub refs: VecDeque<Reference>,
    pub cursors: HashMap<String, Cursor>,
    pub export: Option<Prepared>,
    pub ai: Option<Prepared>,
    pub provider: Option<ExplainProvider>,
    pub epoch: u64,
    pub ai_cancel: CancellationToken,
    maintenance_started: bool,
}
impl OutputState {
    pub fn authorized(&self, grant: &str, token: &str) -> Result<&Grant> {
        self.grants
            .get(grant)
            .filter(|g| g.until > Instant::now() && g.token == token && !g.cancel.is_cancelled())
            .ok_or_else(denied)
    }
    pub fn prune(&mut self) {
        let now = Instant::now();
        let previous_grants = self.grants.len();
        self.grants.retain(|_, g| {
            let keep = g.until > now;
            if !keep {
                g.cancel.cancel();
            }
            keep
        });
        if self.grants.len() != previous_grants {
            self.export = None;
            self.ai = None;
            self.epoch += 1;
            self.ai_cancel.cancel();
            self.ai_cancel = CancellationToken::new();
        }
        self.refs
            .retain(|r| r.until > now && self.grants.contains_key(&r.grant));
        self.cursors
            .retain(|_, c| c.until > now && self.grants.contains_key(&c.grant));
        if self.export.as_ref().is_some_and(|p| p.until <= now) {
            self.export = None;
        }
        if self.ai.as_ref().is_some_and(|p| p.until <= now) {
            self.ai = None;
        }
    }
}
#[derive(Clone)]
pub(super) struct Outputs {
    pub state: Arc<Mutex<OutputState>>,
    pub lane: Arc<Semaphore>,
    pub prepare: Arc<Semaphore>,
    pub ai_lane: Arc<Semaphore>,
    pub session: String,
}
impl Outputs {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(OutputState {
                grants: HashMap::new(),
                refs: VecDeque::new(),
                cursors: HashMap::new(),
                export: None,
                ai: None,
                provider: None,
                epoch: 0,
                ai_cancel: CancellationToken::new(),
                maintenance_started: false,
            })),
            lane: Arc::new(Semaphore::new(1)),
            prepare: Arc::new(Semaphore::new(1)),
            ai_lane: Arc::new(Semaphore::new(1)),
            session: opaque(),
        }
    }
    pub fn maintain(&self, cancel: CancellationToken) {
        let mut state = self.state.lock().unwrap();
        if state.maintenance_started {
            return;
        }
        state.maintenance_started = true;
        let weak = Arc::downgrade(&self.state);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {_=cancel.cancelled()=>break,_=tick.tick()=>{}}
                let Some(state) = weak.upgrade() else {
                    break;
                };
                state.lock().unwrap().prune();
            }
        });
    }
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        for g in state.grants.values() {
            g.cancel.cancel();
        }
        state.grants.clear();
        state.refs.clear();
        state.cursors.clear();
        state.export = None;
        state.ai = None;
        state.epoch += 1;
        state.ai_cancel.cancel();
        state.ai_cancel = CancellationToken::new();
    }
}
pub(super) struct QueryLease {
    pub _lane: OwnedSemaphorePermit,
    pub _background: OwnedSemaphorePermit,
    pub _cpu: OwnedSemaphorePermit,
}
impl AppHandle {
    pub(super) async fn output_permit(
        &self,
        cancel: &CancellationToken,
        deadline: Instant,
    ) -> Result<QueryLease> {
        let lane = self
            .output
            .lane
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Limit("output query busy; retry"))?;
        let background = tokio::select! {
            _ = cancel.cancelled() => return Err(Error::Cancelled),
            _ = tokio::time::sleep_until(deadline.into()) => return Err(Error::Timeout),
            p = self.engine.background.clone().acquire_owned() => p.map_err(|_|Error::Cancelled)?,
        };
        let cpu = tokio::select! {
            _ = cancel.cancelled() => return Err(Error::Cancelled),
            _ = tokio::time::sleep_until(deadline.into()) => return Err(Error::Timeout),
            p = self.engine.cpu.clone().acquire_owned() => p.map_err(|_|Error::Cancelled)?,
        };
        Ok(QueryLease {
            _lane: lane,
            _background: background,
            _cpu: cpu,
        })
    }
    pub fn grants(&self) -> Vec<GrantInfo> {
        let mut state = self.output.state.lock().unwrap();
        state.prune();
        state
            .grants
            .values()
            .filter(|g| !g.desktop)
            .map(|g| g.info.clone())
            .collect()
    }
    pub fn enroll(
        &self,
        client: String,
        sources: Vec<String>,
        consent: bool,
        runtime_dir: String,
    ) -> Result<Enrollment> {
        if !consent
            || client.is_empty()
            || client.len() > 80
            || sources.is_empty()
            || sources.len() > 8
        {
            return Err(denied());
        }
        let registry =
            bridge::read_registry(&std::path::Path::new(&runtime_dir).join("registry.json"))?;
        if registry.session != self.output.session {
            return Err(denied());
        }
        let app = self.state.lock().unwrap();
        if sources.iter().any(|s| !app.sources.contains_key(s)) {
            return Err(denied());
        }
        let mut state = self.output.state.lock().unwrap();
        state.prune();
        if state.grants.values().filter(|g| !g.desktop).count() >= 8 {
            return Err(Error::Limit("8 enrolled clients"));
        }
        let info = GrantInfo {
            id: opaque(),
            client,
            sources,
            expires_at_ms: (now_ms() + 3_600_000).to_string(),
        };
        let token = opaque();
        state.grants.insert(
            info.id.clone(),
            Grant {
                desktop: false,
                info: info.clone(),
                token: token.clone(),
                until: Instant::now() + Duration::from_secs(3600),
                cancel: CancellationToken::new(),
                calls: VecDeque::new(),
                active: 0,
            },
        );
        Ok(Enrollment {
            grant: info,
            token,
            session: self.output.session.clone(),
            runtime_dir,
        })
    }
    pub fn revoke(&self, grant: &str) {
        let mut s = self.output.state.lock().unwrap();
        if let Some(g) = s.grants.remove(grant) {
            g.cancel.cancel();
        }
        s.export = None;
        s.ai = None;
        s.epoch += 1;
        s.ai_cancel.cancel();
        s.ai_cancel = CancellationToken::new();
        s.refs.retain(|r| r.grant != grant);
        s.cursors.retain(|_, c| c.grant != grant);
    }
    pub fn clear_outputs(&self) {
        self.output.clear();
    }
    pub(super) fn scoped_hosts(&self, sources: &[String]) -> Result<HashMap<String, HostProfile>> {
        if sources.is_empty() || sources.len() > 8 || sources.iter().any(|s| s.len() > 128) {
            return Err(denied());
        }
        let state = self.state.lock().unwrap();
        sources
            .iter()
            .map(|s| {
                state
                    .contexts
                    .get(s)
                    .map(|c| (s.clone(), c.host.clone()))
                    .ok_or_else(denied)
            })
            .collect()
    }
    pub async fn prepare_export(
        &self,
        selection: Selection,
        format: ExportFormat,
    ) -> Result<PreparedPreview> {
        self.prepare_output(selection, Some(format), None).await
    }
    pub(super) async fn prepare_output(
        &self,
        selection: Selection,
        format: Option<ExportFormat>,
        provider: Option<ExplainProvider>,
    ) -> Result<PreparedPreview> {
        self.output.maintain(self.cancel.clone());
        let _prepare = self
            .output
            .prepare
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Limit("prepare busy"))?;
        if selection.event_ids.len() > 10_000 || selection.event_ids.iter().any(|s| s.len() > 128) {
            return Err(Error::Limit("selection"));
        }
        if selection.from_ms.as_ref().is_some_and(|s| s.len() > 20)
            || selection.to_ms.as_ref().is_some_and(|s| s.len() > 20)
        {
            return Err(Error::Invalid("time window".into()));
        }
        let from = selection
            .from_ms
            .as_ref()
            .map(|n| n.parse::<u64>())
            .transpose()
            .map_err(|_| Error::Invalid("time window".into()))?;
        let to = selection
            .to_ms
            .as_ref()
            .map(|n| n.parse::<u64>())
            .transpose()
            .map_err(|_| Error::Invalid("time window".into()))?;
        if from.zip(to).is_some_and(|(f, t)| f > t) {
            return Err(Error::Invalid("time window".into()));
        }
        let hosts = self.scoped_hosts(&selection.sources)?;
        let plan = QueryPlan::compile(&selection.query)
            .map_err(|_| Error::Invalid("Invalid output query".into()))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let epoch = self.output.state.lock().unwrap().epoch;
        let cancel = self.cancel.child_token();
        let permit = self.output_permit(&cancel, deadline).await?;
        let snapshot = self.engine.snapshot().await?;
        let ai = provider.is_some();
        let cap = if ai { 64 * 1024 } else { 2 * 1024 * 1024 };
        let prepared = tokio::task::spawn_blocking(move || -> Result<Prepared> {
            let _permit = permit;
            let id = opaque();
            let source_metadata: Vec<_> = hosts.iter().map(|(source, host)| {
                serde_json::json!({"source": source, "server": redaction::sanitize(&host.target).text, "environment": host.environment})
            }).collect();
            let mut metadata = serde_json::json!({
                "record_type": "metadata", "schema_version": 1, "snapshot_id": id,
                "sources": selection.sources, "source_metadata": source_metadata,
                "selection": selection.event_ids,
                "window": {"from_ms": selection.from_ms, "to_ms": selection.to_ms},
                "time_basis": "local_observed_at_ms; event_time unverified",
                "coverage": "retained authorized selection only; source history/permissions can be unknown; fragments/incomplete records withheld before predicates", "gaps": [], "complete": true,
                "redaction_profile": REDACTION_PROFILE,
                "csv_transform": "quote every cell; neutralize leading formula/control including whitespace"
            });
            let mut rows = Vec::new();
            let mut bytes = serde_json::to_vec(&metadata)?.len() + 4096;
            let mut incomplete = false;
            let mut found = 0;
            for event in snapshot.events()? {
                check(&cancel, deadline)?;
                if !hosts.contains_key(&event.source) {
                    continue;
                }
                if !selection.event_ids.is_empty() && !selection.event_ids.contains(&event.id) {
                    continue;
                }
                let time = event.observed_at_ms.parse::<u64>().unwrap_or(0);
                if from.is_some_and(|from| time < from) || to.is_some_and(|to| time > to) {
                    continue;
                }
                found += 1;
                let (projected, partial) = redaction::project(event);
                incomplete |= partial;
                let host = &hosts[&event.source];
                let server = redaction::sanitize(&host.target).text;
                if !plan.matches(&projected, &server) {
                    continue;
                }
                let row = serde_json::json!({
                    "record_type": "event", "source": projected.source, "server": server,
                    "environment": host.environment, "observed_at_ms": projected.observed_at_ms,
                    "event_time": projected.event_time, "transport_time":projected.transport_time, "provenance":projected.evidence, "level": projected.level,
                    "text": projected.raw, "redaction_incomplete": partial
                });
                bytes += serde_json::to_vec(&row)?.len() * 2 + 128;
                if rows.len() >= (if ai { 100 } else { 10_000 }) || bytes > cap {
                    metadata["complete"] = false.into();
                    metadata["gaps"] = serde_json::json!(["selection capped; narrow selection"]);
                    break;
                }
                rows.push(row);
            }
            if !selection.event_ids.is_empty() && found < selection.event_ids.len() {
                metadata["complete"] = false.into();
                metadata["gaps"] = serde_json::json!(["selected events expired"]);
            }
            if selection.sources.iter().any(|source| snapshot.source_losses.contains_key(source)) {
                metadata["complete"] = false.into();
                metadata["gaps"].as_array_mut().unwrap().push("retention or ingestion loss observed".into());
            }
            if ai && rows.is_empty() {
                return Err(Error::NeedsAction("Selected evidence expired or empty; requery".into()));
            }
            metadata["redaction_incomplete"] = incomplete.into();
            let mut payload = String::new();
            match format {
                Some(ExportFormat::Csv) => {
                    payload.push_str("metadata,source,server,environment,observed_at_ms,event_time,level,text,redaction_incomplete\r\n");
                    let metadata = serde_json::to_string(&metadata)?;
                    if rows.is_empty() {
                        payload.push_str(&csv(&metadata));
                        payload.push_str(",,,,,,,,\r\n");
                    }
                    for row in &rows {
                        check(&cancel, deadline)?;
                        let mut cells = vec![csv(&metadata)];
                        for key in ["source", "server", "environment", "observed_at_ms", "event_time", "level", "text", "redaction_incomplete"] {
                            let value = row[key].as_str().map(str::to_owned).unwrap_or_else(|| row[key].to_string());
                            cells.push(csv(&value));
                        }
                        payload.push_str(&cells.join(","));
                        payload.push_str("\r\n");
                        if payload.len() > cap {
                            return Err(Error::Limit("CSV bytes; narrow selection"));
                        }
                    }
                }
                _ => {
                    payload.push_str(&serde_json::to_string(&metadata)?);
                    payload.push('\n');
                    for row in &rows {
                        check(&cancel, deadline)?;
                        payload.push_str(&serde_json::to_string(row)?);
                        payload.push('\n');
                    }
                }
            }
            check(&cancel, deadline)?;
            if payload.len() > cap {
                return Err(Error::Limit("prepared bytes"));
            }
            let digest = format!("{:x}", Sha256::digest(payload.as_bytes()));
            Ok(Prepared {
                preview: PreparedPreview {
                    id, digest, payload, expires_at_ms: (now_ms() + 60_000).to_string(),
                    redaction_incomplete: incomplete,
                },
                until: Instant::now() + Duration::from_secs(60), epoch, provider,
                evidence: selection.event_ids,
            })
        }).await.map_err(|_| Error::Cancelled)??;
        let mut state = self.output.state.lock().unwrap();
        if epoch != state.epoch || (ai && prepared.provider != state.provider) {
            return Err(denied());
        }
        let preview = prepared.preview.clone();
        if ai {
            state.ai = Some(prepared);
        } else {
            state.export = Some(prepared);
        }
        Ok(preview)
    }
    pub fn cancel_prepared(&self) {
        let mut s = self.output.state.lock().unwrap();
        s.export = None;
        s.ai = None;
        s.epoch += 1;
        s.ai_cancel.cancel();
        s.ai_cancel = CancellationToken::new();
    }
    pub fn commit_export(
        &self,
        id: &str,
        digest: &str,
        destination: &std::path::Path,
        overwrite: bool,
    ) -> Result<()> {
        // The serialized desktop principal owns this one-shot preparation. No raw reread.
        let mut state = self.output.state.lock().unwrap();
        state.prune();
        let p = state
            .export
            .take()
            .filter(|p| p.preview.id == id && p.preview.digest == digest && p.epoch == state.epoch)
            .ok_or_else(denied)?;
        let cancel = state.ai_cancel.clone();
        drop(state);
        super::bridge::publish_private_checked(
            destination,
            p.preview.payload.as_bytes(),
            overwrite,
            &cancel,
            || {
                let state = self.output.state.lock().unwrap();
                if state.epoch != p.epoch || Instant::now() >= p.until || cancel.is_cancelled() {
                    return Err(denied());
                }
                Ok(state)
            },
        )
    }
}
fn csv(value: &str) -> String {
    let trimmed = value.trim_start_matches(char::is_whitespace);
    let formula =
        value.starts_with(['\t', '\r', '\n']) || trimmed.starts_with(['=', '+', '-', '@']);
    format!(
        "\"{}{}\"",
        if formula { "'" } else { "" },
        value.replace('"', "\"\"")
    )
}
