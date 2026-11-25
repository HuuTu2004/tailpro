use super::*;
use crate::adapters::{settings::HostPreferences, sources::RemoteOperation};
use sha2::{Digest, Sha256};

const DIRECTORY_PAGE: usize = 64;
const METADATA_TTL: Duration = Duration::from_secs(30);

#[derive(Clone)]
struct DirectorySnapshot {
    host: String,
    generation: String,
    expires: Instant,
    revision: String,
    page: DirectoryPage,
    entries: Arc<Vec<DirectoryEntry>>,
}

type CapabilityCache = HashMap<String, (String, Instant, HostCapabilities)>;

#[derive(Clone, Default)]
pub(super) struct SourceSelection {
    directories: Arc<Mutex<HashMap<String, DirectorySnapshot>>>,
    capabilities: Arc<Mutex<CapabilityCache>>,
    opening: Arc<tokio::sync::Mutex<()>>,
    open_cancel: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

pub(super) fn source_key(host: &str, spec: &SourceSpec) -> String {
    let value = match spec {
        SourceSpec::File { path } => format!("file:{path}"),
        SourceSpec::Docker { container_id, .. } => format!("docker:{container_id}"),
        SourceSpec::Journal { unit } => format!("journal:{unit}"),
        SourceSpec::Kubernetes {
            pod_uid,
            container_id,
            ..
        } => format!("kubernetes:{pod_uid}:{container_id}"),
    };
    format!("{:x}", Sha256::digest(format!("{host}\0{value}")))
}

impl AppHandle {
    fn live_session(&self, host: &str) -> Result<Arc<Session>> {
        self.state
            .lock()
            .unwrap()
            .hosts
            .get(host)
            .and_then(|h| h.sessions.borrow().clone())
            .ok_or_else(|| Error::NeedsAction("Connect this host first".into()))
    }

    pub async fn inspect_host(&self, host: &str, refresh: bool) -> Result<HostCapabilities> {
        let session = self.live_session(host)?;
        session.validate().await?;
        let generation = session.remote_identity().to_owned();
        if !refresh
            && let Some((identity, expires, value)) =
                self.source_selection.capabilities.lock().unwrap().get(host)
            && identity == &generation
            && *expires > Instant::now()
        {
            return Ok(value.clone());
        }
        let (session, cancel) = self.session(host, false)?;
        let result = self.inspect_session(host, &session, &cancel).await?;
        self.cache_capabilities(result.clone());
        Ok(result)
    }

    pub(super) fn cache_capabilities(&self, result: HostCapabilities) {
        let mut cache = self.source_selection.capabilities.lock().unwrap();
        cache.retain(|_, (_, expires, _)| *expires > Instant::now());
        if cache.len() < 8 {
            cache.insert(
                result.host_id.clone(),
                (
                    result.generation.clone(),
                    Instant::now() + METADATA_TTL,
                    result,
                ),
            );
        }
    }

    pub(super) async fn inspect_session(
        &self,
        host: &str,
        session: &Session,
        cancel: &CancellationToken,
    ) -> Result<HostCapabilities> {
        let value = sources::inspect(
            session,
            RemoteOperation::HostMetadata(serde_json::json!({"action":"inspect"})),
            cancel,
        )
        .await;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let mut result = HostCapabilities {
            host_id: host.into(),
            generation: session.remote_identity().into(),
            observed_at_ms: now_ms().to_string(),
            os: "Unknown".into(),
            user: None,
            home: None,
            helper_ready: false,
            providers: vec![],
            locations: vec![],
            max_sources: log_core::model::MAX_SOURCES as u32,
            issues: vec![],
        };
        match value {
            Ok(value) if value["helper_ready"] == true => {
                result.os = value["os"].as_str().unwrap_or("Unknown").into();
                result.user = value["user"].as_str().map(str::to_owned);
                result.home = value["home"].as_str().map(str::to_owned);
                result.helper_ready = true;
                result.providers = serde_json::from_value(value["providers"].clone())?;
                result.locations = serde_json::from_value(value["locations"].clone())?;
            }
            _ => result.issues.push("SSH is connected, but the Python 3 POSIX metadata helper is unavailable. Check the remote SSH environment and reader prerequisites.".into()),
        }
        Ok(result)
    }

    pub async fn browse_directory(
        &self,
        host: &str,
        path: String,
        cursor: Option<String>,
    ) -> Result<DirectoryPage> {
        policy::path(&path)?;
        let (session, cancel) = self.session(host, true)?;
        let (key, offset, snapshot) = if let Some(cursor) = cursor {
            let (key, index) = cursor
                .split_once('.')
                .ok_or_else(|| Error::Invalid("Invalid directory cursor".into()))?;
            let offset: usize = index
                .parse()
                .map_err(|_| Error::Invalid("Invalid directory cursor".into()))?;
            let snapshot = self
                .source_selection
                .directories
                .lock()
                .unwrap()
                .get(key)
                .cloned()
                .filter(|s| {
                    s.host == host
                        && s.generation == session.remote_identity()
                        && s.page.path == path
                        && s.expires > Instant::now()
                        && offset <= s.entries.len()
                })
                .ok_or_else(|| {
                    Error::NeedsAction("Directory cursor expired; refresh this location".into())
                })?;
            let current = sources::inspect(
                &session,
                RemoteOperation::HostMetadata(serde_json::json!({"action":"revision","path":path})),
                &cancel,
            )
            .await?;
            if current["revision"].as_str() != Some(snapshot.revision.as_str()) {
                self.source_selection
                    .directories
                    .lock()
                    .unwrap()
                    .remove(key);
                return Err(Error::NeedsAction(
                    "Directory changed; refresh this location before paging".into(),
                ));
            }
            (key.to_owned(), offset, snapshot)
        } else {
            let value = sources::inspect(
                &session,
                RemoteOperation::HostMetadata(serde_json::json!({"action":"browse","path":path})),
                &cancel,
            )
            .await?;
            if let Some(error) = value["error"].as_str() {
                return Err(Error::NeedsAction(error.into()));
            }
            let entries: Vec<DirectoryEntry> = serde_json::from_value(value["entries"].clone())?;
            if entries.len() > 1000 {
                return Err(Error::Limit("directory metadata"));
            }
            let key = format!("directory-{:x}", rand::random::<u128>());
            let page = DirectoryPage {
                path: value["path"].as_str().unwrap_or(&path).into(),
                parent: value["parent"].as_str().map(str::to_owned),
                resolved_path: value["resolved_path"].as_str().map(str::to_owned),
                entries: vec![],
                next_cursor: None,
                complete: value["complete"] == true,
                reasons: serde_json::from_value(value["reasons"].clone())?,
            };
            let snapshot = DirectorySnapshot {
                host: host.into(),
                generation: session.remote_identity().into(),
                expires: Instant::now() + METADATA_TTL,
                revision: value["revision"].as_str().unwrap_or("").into(),
                page,
                entries: Arc::new(entries),
            };
            let mut cache = self.source_selection.directories.lock().unwrap();
            cache.retain(|_, s| s.expires > Instant::now() && s.host != host);
            if cache.len() >= 8 {
                return Err(Error::Limit("directory snapshots"));
            }
            cache.insert(key.clone(), snapshot.clone());
            (key, 0, snapshot)
        };
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let end = (offset + DIRECTORY_PAGE).min(snapshot.entries.len());
        let mut page = snapshot.page;
        page.entries = snapshot.entries[offset..end].to_vec();
        page.next_cursor = (end < snapshot.entries.len()).then(|| format!("{key}.{end}"));
        Ok(page)
    }

    pub fn discovery_preferences(&self, host: &str) -> Result<DiscoveryPreferences> {
        let session = self.live_session(host)?;
        Ok(self
            .settings
            .lock()
            .unwrap()
            .load()?
            .discovery
            .into_iter()
            .find(|p| p.host_id == host && p.remote_identity == session.remote_identity())
            .map(|p| p.preferences)
            .unwrap_or_default())
    }

    pub fn save_discovery_preferences(
        &self,
        host: &str,
        mut preferences: DiscoveryPreferences,
    ) -> Result<()> {
        let session = self.live_session(host)?;
        if preferences.directories.len() > 16 {
            return Err(Error::Limit("16 discovery locations"));
        }
        for path in &preferences.directories {
            policy::path(path)?;
        }
        preferences.directories.sort();
        preferences.directories.dedup();
        preferences.initialized = true;
        let store = self.settings.lock().unwrap();
        let mut saved = store.load()?;
        saved.discovery.retain(|p| p.host_id != host);
        saved.discovery.push(HostPreferences {
            host_id: host.into(),
            remote_identity: session.remote_identity().into(),
            preferences,
        });
        store.save(&saved)
    }

    pub fn remembered_sources(&self, host: &str) -> Result<Vec<SourceSpec>> {
        let session = self.live_session(host)?;
        let saved = self.settings.lock().unwrap().load()?;
        let mut specs: Vec<_> = saved
            .remembered_sources
            .into_iter()
            .filter(|s| s.host_id == host && s.remote_identity == session.remote_identity())
            .map(|s| s.spec)
            .collect();
        for pin in saved
            .pins
            .into_iter()
            .filter(|p| p.host_id == host && p.remote_identity == session.remote_identity())
        {
            let spec = SourceSpec::File { path: pin.path };
            if !specs
                .iter()
                .any(|s| source_key(host, s) == source_key(host, &spec))
            {
                specs.push(spec);
            }
        }
        Ok(specs)
    }

    pub fn cancel_source_open(&self, host: &str) {
        if let Some(cancel) = self.source_selection.open_cancel.lock().unwrap().get(host) {
            cancel.cancel();
        }
    }

    pub async fn open_sources(
        &self,
        host: &str,
        specs: Vec<SourceSpec>,
    ) -> Result<Vec<OpenSourceResult>> {
        if specs.is_empty() || specs.len() > log_core::model::MAX_SOURCES {
            return Err(Error::Limit("source selection"));
        }
        let cancel = self.cancel.child_token();
        {
            let mut active = self.source_selection.open_cancel.lock().unwrap();
            if let Some(old) = active.insert(host.into(), cancel.clone()) {
                old.cancel();
            }
        }
        let _opening = tokio::select! { _ = cancel.cancelled() => return Err(Error::Cancelled), permit = self.source_selection.opening.lock() => permit };
        let session = self.live_session(host)?;
        let deadline = Instant::now() + Duration::from_secs(25);
        let mut results = Vec::new();
        for spec in specs {
            if cancel.is_cancelled() || session.cancel.is_cancelled() {
                break;
            }
            if let SourceSpec::File { path } = &spec {
                policy::path(path)?;
                let checked = sources::inspect(
                    &session,
                    RemoteOperation::HostMetadata(serde_json::json!({"action":"file","path":path})),
                    &cancel,
                )
                .await;
                let problem = match checked {
                    Ok(value) if value["kind"] == "file" && ![".gz", ".bz2", ".xz", ".zip", ".zst", ".lz4"].iter().any(|ext| path.to_ascii_lowercase().ends_with(ext)) => None,
                    Ok(value) => Some(value["error"].as_str().unwrap_or("Choose a regular, uncompressed log file; resolve symbolic links explicitly").to_owned()),
                    Err(error) => Some(error.to_string()),
                };
                if let Some(error) = problem {
                    results.push(OpenSourceResult {
                        spec,
                        source_id: None,
                        state: "failed".into(),
                        error: Some(error),
                    });
                    continue;
                }
            }
            let key = source_key(host, &spec);
            if results
                .iter()
                .any(|r: &OpenSourceResult| source_key(host, &r.spec) == key)
            {
                continue;
            }
            let existing = self
                .status()
                .sources
                .into_iter()
                .find(|s| s.host_id == host && source_key(host, &s.spec) == key);
            let existing = if let Some(info) = existing {
                if info.state == "needs_action" {
                    self.stop_source(&info.id).await?;
                    None
                } else {
                    Some(info)
                }
            } else {
                None
            };
            let result = if let Some(existing) = existing {
                Ok(existing.id)
            } else {
                self.start_source(host, spec.clone()).await
            };
            let id = match result {
                Ok(id) => id,
                Err(error) => {
                    results.push(OpenSourceResult {
                        spec,
                        source_id: None,
                        state: "failed".into(),
                        error: Some(error.to_string()),
                    });
                    continue;
                }
            };
            loop {
                let info = self.status().sources.into_iter().find(|s| s.id == id);
                let Some(info) = info else {
                    results.push(OpenSourceResult {
                        spec,
                        source_id: Some(id),
                        state: "failed".into(),
                        error: Some("Reader was stopped".into()),
                    });
                    break;
                };
                if info.state == "streaming" {
                    let error = self
                        .remember_source(host, session.remote_identity(), &spec)
                        .err()
                        .map(|error| {
                            format!("Reader opened, but its history could not be saved: {error}")
                        });
                    results.push(OpenSourceResult {
                        spec,
                        source_id: Some(id),
                        state: info.state,
                        error,
                    });
                    break;
                }
                if info.state == "needs_action"
                    || Instant::now() >= deadline
                    || session.cancel.is_cancelled()
                    || cancel.is_cancelled()
                {
                    results.push(OpenSourceResult { spec, source_id: Some(id), state: info.state, error: info.detail.or_else(|| Some("Reader did not reach streaming before the deadline; inspect its status".into())) });
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        Ok(results)
    }

    fn remember_source(&self, host: &str, identity: &str, spec: &SourceSpec) -> Result<()> {
        let key = source_key(host, spec);
        let store = self.settings.lock().unwrap();
        let mut saved = store.load()?;
        saved.remembered_sources.retain(|s| s.key != key);
        if saved
            .remembered_sources
            .iter()
            .filter(|s| s.host_id == host)
            .count()
            >= 100
            && let Some(i) = saved
                .remembered_sources
                .iter()
                .position(|s| s.host_id == host)
        {
            saved.remembered_sources.remove(i);
        }
        if saved.remembered_sources.len() >= 800 {
            saved.remembered_sources.remove(0);
        }
        saved.remembered_sources.push(WorkspaceSource {
            key,
            host_id: host.into(),
            remote_identity: identity.into(),
            spec: spec.clone(),
        });
        store.save(&saved)
    }

    pub fn saved_workspace(&self) -> Result<SavedWorkspace> {
        Ok(self.settings.lock().unwrap().load()?.workspace)
    }

    pub fn set_restore_readers(&self, enabled: bool) -> Result<()> {
        let store = self.settings.lock().unwrap();
        let mut saved = store.load()?;
        saved.workspace.restore_previous_readers = enabled;
        store.save(&saved)
    }

    pub fn save_workspace(&self, selection: WorkspaceSelection) -> Result<()> {
        if selection.source_ids.len() > log_core::model::MAX_SOURCES
            || selection.selected_sources.len() > 8
            || selection.combined_sources.len() > 8
        {
            return Err(Error::Limit("workspace sources"));
        }
        let status = self.status();
        let mut sources = Vec::new();
        let mut ids = HashMap::new();
        for id in selection.source_ids {
            let Some(source) = status.sources.iter().find(|s| s.id == id) else {
                continue;
            };
            let session = self.live_session(&source.host_id)?;
            let key = source_key(&source.host_id, &source.spec);
            ids.insert(id, key.clone());
            sources.push(WorkspaceSource {
                key,
                host_id: source.host_id.clone(),
                remote_identity: session.remote_identity().into(),
                spec: source.spec.clone(),
            });
        }
        let workspace = SavedWorkspace {
            sources,
            selected_sources: selection
                .selected_sources
                .iter()
                .filter_map(|id| ids.get(id).cloned())
                .collect(),
            combined_sources: selection
                .combined_sources
                .iter()
                .filter_map(|id| ids.get(id).cloned())
                .collect(),
            selected_host: selection.selected_host,
            restore_previous_readers: selection.restore_previous_readers,
        };
        let store = self.settings.lock().unwrap();
        let mut saved = store.load()?;
        saved.workspace = workspace;
        store.save(&saved)
    }

    pub async fn restore_host_sources(&self, host: &str) -> Result<Vec<OpenSourceResult>> {
        let session = self.live_session(host)?;
        let saved = self.saved_workspace()?;
        if !saved.restore_previous_readers {
            return Ok(vec![]);
        }
        let specs: Vec<_> = saved
            .sources
            .into_iter()
            .filter(|s| s.host_id == host && s.remote_identity == session.remote_identity())
            .map(|s| s.spec)
            .collect();
        if specs.is_empty() {
            Ok(vec![])
        } else {
            self.open_sources(host, specs).await
        }
    }
}
