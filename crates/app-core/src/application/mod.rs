mod agent_context;
pub mod bridge;
mod connection_profiles;
mod explainer;
mod output;
mod parsing;
pub mod policy;
mod production;
pub mod redaction;
mod topology;
mod views;
use crate::{
    adapters::{
        discovery,
        settings::{Pin, SettingsStore},
        sources::{self, Reader},
        ssh::{Session, config},
    },
    api::*,
    workers::EngineWorker,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
struct Host {
    info: Arc<Mutex<ConnectionInfo>>,
    cancel: CancellationToken,
    sessions: watch::Sender<Option<Arc<Session>>>,
    task: tokio::task::JoinHandle<()>,
    discovery: Option<CancellationToken>,
    completion: Option<CancellationToken>,
}
struct Source {
    parser: Arc<std::sync::RwLock<Arc<crate::workers::parser::Parser>>>,
    info: Arc<Mutex<SourceInfo>>,
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
struct Context {
    host: HostProfile,
    expires: Option<Instant>,
}
struct State {
    contexts: HashMap<String, Context>,
    hosts: HashMap<String, Host>,
    sources: HashMap<String, Source>,
}
#[derive(Clone)]
pub struct AppHandle {
    state: Arc<Mutex<State>>,
    engine: EngineWorker,
    cancel: CancellationToken,
    settings: Arc<Mutex<SettingsStore>>,
    views: views::Views,
    output: output::Outputs,
    connection_tests: Arc<tokio::sync::Semaphore>,
    connection_test_cancel: Arc<Mutex<Option<CancellationToken>>>,
}
impl AppHandle {
    pub fn new(settings_path: PathBuf) -> Self {
        let cancel = CancellationToken::new();
        Self {
            state: Arc::new(Mutex::new(State {
                contexts: HashMap::new(),
                hosts: HashMap::new(),
                sources: HashMap::new(),
            })),
            engine: EngineWorker::start(cancel.clone()),
            views: views::Views::new(),
            output: output::Outputs::new(),
            connection_tests: Arc::new(tokio::sync::Semaphore::new(1)),
            connection_test_cancel: Arc::new(Mutex::new(None)),
            cancel,
            settings: Arc::new(Mutex::new(SettingsStore::new(settings_path))),
        }
    }
    pub fn health(&self) -> Health {
        Health {
            policy_revision: self.output.state.lock().unwrap().epoch.to_string(),
            schema_version: 1,
            status: "ok".into(),
            read_only: true,
        }
    }
    pub fn import_config(&self) -> Result<ConfigImport> {
        Ok(config::load()?.dto())
    }
    pub fn saved_hosts(&self) -> Result<Vec<HostProfile>> {
        Ok(self.settings.lock().unwrap().load()?.hosts)
    }
    pub fn status(&self) -> AppStatus {
        let state = self.state.lock().unwrap();
        AppStatus {
            health: self.health(),
            connections: state
                .hosts
                .values()
                .map(|h| h.info.lock().unwrap().clone())
                .collect(),
            sources: state
                .sources
                .values()
                .map(|s| s.info.lock().unwrap().clone())
                .collect(),
        }
    }
    pub async fn connect(&self, profile: HostProfile) -> Result<()> {
        self.connect_with_consent(profile, false).await
    }
    pub async fn connect_with_consent(&self, profile: HostProfile, consent: bool) -> Result<()> {
        crate::adapters::ssh::validate_profile(&profile)?;
        self.disconnect(&profile.id).await?;
        let mut state = self.state.lock().unwrap();
        if state.hosts.len() >= 3 {
            return Err(Error::Limit("3 hosts"));
        }
        {
            let store = self.settings.lock().unwrap();
            let mut saved = store.load()?;
            saved.hosts.retain(|h| h.id != profile.id);
            saved.hosts.push(profile.clone());
            store.save(&saved)?;
        }
        let info = Arc::new(Mutex::new(ConnectionInfo {
            host: profile.clone(),
            state: ConnectionState::Connecting,
            detail: None,
        }));
        let cancel = self.cancel.child_token();
        let (tx, _) = watch::channel(None);
        let task = tokio::spawn(connection_loop(
            profile,
            info.clone(),
            tx.clone(),
            cancel.clone(),
            consent,
        ));
        state.hosts.insert(
            info.lock().unwrap().host.id.clone(),
            Host {
                info: info.clone(),
                cancel,
                sessions: tx,
                task,
                discovery: None,
                completion: None,
            },
        );
        Ok(())
    }
    pub async fn disconnect(&self, host_id: &str) -> Result<()> {
        self.clear_outputs();
        let (host, tasks) = {
            let mut state = self.state.lock().unwrap();
            let host = state.hosts.remove(host_id);
            let keys: Vec<_> = state
                .sources
                .iter()
                .filter(|(_, s)| s.info.lock().unwrap().host_id == host_id)
                .map(|(k, _)| k.clone())
                .collect();
            let tasks = keys
                .into_iter()
                .filter_map(|k| state.sources.remove(&k))
                .collect::<Vec<_>>();
            for source in &tasks {
                if let Some(context) = state.contexts.get_mut(&source.info.lock().unwrap().id) {
                    context.expires = Some(Instant::now() + Duration::from_secs(65));
                }
            }
            (host, tasks)
        };
        if let Some(host) = &host {
            host.cancel.cancel();
        }
        for source in &tasks {
            source.cancel.cancel();
        }
        let cleanup = async {
            for source in tasks {
                let id = source.info.lock().unwrap().id.clone();
                let _ = source.task.await;
                let _ = self.engine.retire(&id);
            }
            if let Some(host) = host {
                let _ = host.task.await;
            }
        };
        tokio::time::timeout(Duration::from_secs(3), cleanup)
            .await
            .map_err(|_| Error::Timeout)?;
        Ok(())
    }
    fn session(
        &self,
        host_id: &str,
        completion: bool,
    ) -> Result<(Arc<Session>, CancellationToken)> {
        let mut state = self.state.lock().unwrap();
        let host = state
            .hosts
            .get_mut(host_id)
            .ok_or_else(|| Error::Invalid("unknown host".into()))?;
        let session = host
            .sessions
            .borrow()
            .clone()
            .ok_or_else(|| Error::NeedsAction("connect host first".into()))?;
        let slot = if completion {
            &mut host.completion
        } else {
            &mut host.discovery
        };
        if let Some(old) = slot.take() {
            old.cancel();
        }
        let token = host.cancel.child_token();
        *slot = Some(token.clone());
        Ok((session, token))
    }
    pub async fn discover(&self, request: DiscoveryRequest) -> Result<DiscoveryResult> {
        let (session, cancel) = self.session(&request.host_id, false)?;
        discovery::discover(session, request, cancel).await
    }
    pub async fn complete(
        &self,
        host_id: &str,
        parent: String,
        prefix: String,
    ) -> Result<DiscoveryResult> {
        let (session, cancel) = self.session(host_id, true)?;
        discovery::complete(session, parent, prefix, cancel).await
    }
    pub async fn start_source(&self, host_id: &str, spec: SourceSpec) -> Result<String> {
        match &spec {
            SourceSpec::Docker {
                container_id,
                aliases,
            } => {
                policy::container_id(container_id)?;
                if aliases.len() > 16
                    || aliases
                        .iter()
                        .any(|s| s.len() > 128 || s.chars().any(char::is_control))
                {
                    return Err(Error::Invalid("source metadata aliases".into()));
                }
            }
            SourceSpec::File { path } => policy::path(path)?,
            SourceSpec::Journal { unit } => policy::unit(unit)?,
            SourceSpec::Kubernetes {
                kubeconfig,
                context,
                namespace,
                pod,
                pod_uid,
                container,
                container_id,
                config_digest,
                trusted,
                ..
            } => {
                policy::path(kubeconfig)?;
                for value in [context, namespace, pod, pod_uid, container, container_id] {
                    if value.is_empty()
                        || value.len() > 253
                        || value.starts_with('-')
                        || value.chars().any(|c| c.is_whitespace() || c.is_control())
                    {
                        return Err(Error::Invalid(
                            "Invalid Kubernetes selected identity".into(),
                        ));
                    }
                }
                if !trusted
                    || config_digest.len() != 64
                    || !config_digest.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(Error::NeedsAction(
                        "Inspect and trust the selected Kubernetes context first".into(),
                    ));
                }
            }
        }
        let (sessions, cancel, profile) = {
            let mut state = self.state.lock().unwrap();
            state
                .contexts
                .retain(|_, c| c.expires.is_none_or(|at| Instant::now() < at));
            if state.contexts.len() >= 32 {
                return Err(Error::Limit("source context metadata"));
            }
            if state.sources.len() >= 8 {
                return Err(Error::Limit("8 sources"));
            }
            let host = state
                .hosts
                .get(host_id)
                .ok_or_else(|| Error::Invalid("unknown host".into()))?;
            if host.sessions.borrow().is_none() {
                return Err(Error::NeedsAction("connect host first".into()));
            }
            (
                host.sessions.subscribe(),
                host.cancel.child_token(),
                host.info.lock().unwrap().host.clone(),
            )
        };
        let id = format!("source-{:x}", rand::random::<u64>());
        let generation = rand::random::<u64>();
        let ingest = self.engine.register(&id, generation).await?;
        let info = Arc::new(Mutex::new(SourceInfo {
            id: id.clone(),
            host_id: host_id.into(),
            spec: spec.clone(),
            state: "starting".into(),
            detail: None,
            generation: generation.to_string(),
        }));
        let parser = ingest.parser.clone();
        let reader = Reader {
            source: id.clone(),
            generation,
            spec,
            ingest,
            cancel: cancel.clone(),
            info: info.clone(),
            sessions,
        };
        if cancel.is_cancelled() {
            let _ = self.engine.retire(&id);
            return Err(Error::Cancelled);
        }
        self.state.lock().unwrap().contexts.insert(
            id.clone(),
            Context {
                host: profile,
                expires: None,
            },
        );
        let task = tokio::spawn(sources::run(reader));
        self.state.lock().unwrap().sources.insert(
            id.clone(),
            Source {
                info,
                cancel,
                task,
                parser,
            },
        );
        Ok(id)
    }
    pub async fn stop_source(&self, source_id: &str) -> Result<()> {
        self.clear_outputs();
        let source = {
            let mut state = self.state.lock().unwrap();
            if let Some(context) = state.contexts.get_mut(source_id) {
                context.expires = Some(Instant::now() + Duration::from_secs(65));
            }
            state.sources.remove(source_id)
        };
        if let Some(source) = source {
            let _ = self.engine.retire(&source.info.lock().unwrap().id);
            source.cancel.cancel();
            let _ = tokio::time::timeout(Duration::from_secs(3), source.task).await;
        }
        Ok(())
    }
    pub fn pin(&self, host_id: &str, path: &str) -> Result<()> {
        policy::path(path)?;
        let state = self.state.lock().unwrap();
        let host = state
            .hosts
            .get(host_id)
            .ok_or_else(|| Error::Invalid("unknown host".into()))?;
        // Pin only a path that successfully reached a running file source on this profile.
        if !state.sources.values().any(|s| {
            let i = s.info.lock().unwrap();
            i.host_id == host_id
                && i.state == "streaming"
                && matches!(&i.spec, SourceSpec::File {path:p} if p == path)
        }) {
            return Err(Error::NeedsAction(
                "start this file successfully before pinning".into(),
            ));
        }
        let store = self.settings.lock().unwrap();
        let mut saved = store.load()?;
        saved
            .pins
            .retain(|p| !(p.host_id == host_id && p.path == path));
        saved.pins.push(Pin {
            host_id: host_id.into(),
            remote_identity: host
                .sessions
                .borrow()
                .as_ref()
                .ok_or_else(|| Error::NeedsAction("host disconnected".into()))?
                .remote_identity()
                .to_owned(),
            path: path.into(),
        });
        store.save(&saved)
    }
    pub fn cancel_discovery(&self, host_id: &str) {
        if let Some(host) = self.state.lock().unwrap().hosts.get_mut(host_id) {
            if let Some(token) = host.discovery.take() {
                token.cancel();
            }
            if let Some(token) = host.completion.take() {
                token.cancel();
            }
        }
    }
    pub fn pins(&self, host_id: &str) -> Result<Vec<String>> {
        let state = self.state.lock().unwrap();
        let host = state
            .hosts
            .get(host_id)
            .ok_or_else(|| Error::Invalid("unknown host".into()))?;
        let target = host
            .sessions
            .borrow()
            .as_ref()
            .ok_or_else(|| Error::NeedsAction("host disconnected".into()))?
            .remote_identity()
            .to_owned();
        Ok(self
            .settings
            .lock()
            .unwrap()
            .load()?
            .pins
            .into_iter()
            .filter(|p| p.host_id == host_id && p.remote_identity == target)
            .map(|p| p.path)
            .collect())
    }
    pub fn set_environment(&self, host_id: &str, environment: Environment) -> Result<()> {
        self.clear_outputs();
        let mut state = self.state.lock().unwrap();
        let host = state
            .hosts
            .get_mut(host_id)
            .ok_or_else(|| Error::Invalid("unknown host".into()))?;
        host.info.lock().unwrap().host.environment = environment.clone();
        for context in state.contexts.values_mut().filter(|c| c.host.id == host_id) {
            context.host.environment = environment.clone();
        }
        let store = self.settings.lock().unwrap();
        let mut saved = store.load()?;
        for host in saved.hosts.iter_mut().filter(|h| h.id == host_id) {
            host.environment = environment.clone();
        }
        store.save(&saved)
    }
    pub async fn shutdown(&self) {
        self.views.cancel_all();
        self.output.clear();
        let hosts: Vec<_> = self.state.lock().unwrap().hosts.keys().cloned().collect();
        for host in hosts {
            let _ = self.disconnect(&host).await;
        }
        self.cancel.cancel();
    }
}
async fn connection_loop(
    profile: HostProfile,
    info: Arc<Mutex<ConnectionInfo>>,
    sessions: watch::Sender<Option<Arc<Session>>>,
    cancel: CancellationToken,
    consent: bool,
) {
    let mut started = Instant::now();
    let mut attempt = 0;
    loop {
        if cancel.is_cancelled() {
            break;
        }
        match Session::connect(profile.clone(), cancel.child_token(), consent).await {
            Ok(session) => {
                let session = Arc::new(session);
                sessions.send_replace(Some(session.clone()));
                {
                    let mut i = info.lock().unwrap();
                    i.state = ConnectionState::Connected;
                    i.detail = None;
                }
                loop {
                    tokio::select! { _ = cancel.cancelled() => break, _ = tokio::time::sleep(Duration::from_secs(1)) => {} }
                    if !session.alive().await || session.validate().await.is_err() {
                        break;
                    }
                }
                sessions.send_replace(None);
                session.close().await;
                started = Instant::now();
                attempt = 0;
                if cancel.is_cancelled() {
                    break;
                }
                if consent {
                    let mut i = info.lock().unwrap();
                    i.state = ConnectionState::NeedsAction;
                    i.detail = Some(
                        "Dynamic config consent expires on disconnect; reconnect explicitly".into(),
                    );
                    return;
                }
            }
            Err(Error::NeedsAction(_)) => {
                let mut i = info.lock().unwrap();
                i.state = ConnectionState::NeedsAction;
                i.detail = Some("Verify known_hosts, static SSH config, agent socket and noninteractive authentication with your SSH client".into());
                return;
            }
            Err(_) if cancel.is_cancelled() => break,
            Err(_) => {}
        }
        if started.elapsed() > Duration::from_secs(300) {
            let mut i = info.lock().unwrap();
            i.state = ConnectionState::NeedsAction;
            i.detail = Some("Reconnect deadline reached".into());
            return;
        }
        info.lock().unwrap().state = ConnectionState::Reconnecting;
        let delay = (1_u64 << attempt.min(5)).min(30) * 1000 + rand::random::<u64>() % 500;
        attempt += 1;
        tokio::select! { _ = cancel.cancelled() => break, _ = tokio::time::sleep(Duration::from_millis(delay)) => {} }
    }
    info.lock().unwrap().state = ConnectionState::Disconnected;
}
