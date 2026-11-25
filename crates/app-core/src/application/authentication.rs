use super::*;
use crate::adapters::ssh::auth::{ATTEMPT_DEADLINE, ChallengeState};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub(super) struct AuthenticationState {
    active: Arc<Mutex<Option<Arc<ChallengeState>>>>,
    permits: Arc<tokio::sync::Semaphore>,
}
impl Default for AuthenticationState {
    fn default() -> Self {
        Self {
            active: Arc::new(Mutex::new(None)),
            permits: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}
impl AppHandle {
    pub(super) fn trust_file(&self, profile: &HostProfile) -> Result<PathBuf> {
        Ok(self
            .settings
            .lock()
            .unwrap()
            .directory()?
            .join("ssh-trust")
            .join(format!(
                "{:x}.known_hosts",
                Sha256::digest(profile.id.as_bytes())
            )))
    }

    pub fn begin_connection(
        &self,
        profile: HostProfile,
        consent: bool,
        test_only: bool,
    ) -> Result<String> {
        crate::adapters::ssh::validate_profile(&profile)?;
        {
            let state = self.state.lock().unwrap();
            if !test_only && state.hosts.len() >= 3 && !state.hosts.contains_key(&profile.id) {
                return Err(Error::Limit("3 connected hosts"));
            }
        }
        let permit = self
            .authentication
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                Error::NeedsAction("Finish or cancel the current connection attempt first".into())
            })?;
        let id = format!("connection-{:x}", rand::random::<u128>());
        let attempt = ChallengeState::new(id.clone(), profile.id.clone(), test_only);
        *self.authentication.active.lock().unwrap() = Some(attempt.clone());
        let app = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let outcome = app
                .run_connection_attempt(profile, consent, test_only, attempt.clone())
                .await;
            match outcome {
                Ok(()) => attempt.finish(if test_only { "tested" } else { "connected" }, None),
                Err(Error::Cancelled) => {
                    attempt.finish("cancelled", Some("Connection cancelled".into()))
                }
                Err(error) => attempt.finish("failed", Some(error.to_string())),
            }
        });
        Ok(id)
    }

    pub fn connection_attempt(&self, id: &str) -> Result<ConnectionAttempt> {
        let active = self.authentication.active.lock().unwrap();
        let attempt = active
            .as_ref()
            .ok_or_else(|| Error::NeedsAction("Connection attempt expired".into()))?;
        let view = attempt.view.lock().unwrap();
        if view.id != id {
            return Err(Error::NeedsAction("Connection attempt expired".into()));
        }
        Ok(view.clone())
    }

    pub fn answer_authentication(
        &self,
        attempt_id: &str,
        challenge_id: &str,
        value: String,
        verified: bool,
    ) -> Result<()> {
        let active = self.authentication.active.lock().unwrap();
        let attempt = active.as_ref().ok_or(Error::Cancelled)?;
        if attempt.view.lock().unwrap().id != attempt_id {
            return Err(Error::Cancelled);
        }
        attempt.answer(challenge_id, value, verified)
    }

    pub fn cancel_connection_attempt(&self) {
        if let Some(attempt) = self.authentication.active.lock().unwrap().as_ref()
            && matches!(
                attempt.view.lock().unwrap().state.as_str(),
                "authenticating" | "checking_readers"
            )
        {
            attempt.cancel.cancel();
        }
    }

    async fn run_connection_attempt(
        &self,
        profile: HostProfile,
        consent: bool,
        test_only: bool,
        attempt: Arc<ChallengeState>,
    ) -> Result<()> {
        let session_cancel = self.cancel.child_token();
        let connect = Session::connect_managed(
            profile.clone(),
            session_cancel.clone(),
            consent,
            Some(self.trust_file(&profile)?),
            Some(attempt.clone()),
        );
        tokio::pin!(connect);
        let failure = tokio::select! {
            biased;
            _ = attempt.cancel.cancelled() => Error::Cancelled,
            _ = self.cancel.cancelled() => Error::Cancelled,
            _ = tokio::time::sleep(ATTEMPT_DEADLINE) => Error::Timeout,
            result = &mut connect => {
                let session = result?;
                attempt.view.lock().unwrap().state = "checking_readers".into();
                let capabilities = match self.inspect_session(&profile.id, &session, &attempt.cancel).await {
                    Ok(value) => value,
                    Err(error) => { session.close().await; return Err(error); }
                };
                if attempt.cancel.is_cancelled() { session.close().await; return Err(Error::Cancelled); }
                if test_only { session.close().await; }
                else { self.install_connection(profile.clone(), session, consent).await?; self.cache_capabilities(capabilities.clone()); }
                attempt.view.lock().unwrap().capabilities = Some(capabilities);
                return Ok(());
            }
        };
        session_cancel.cancel();
        if let Ok(session) = connect.await {
            session.close().await;
        }
        Err(failure)
    }

    async fn install_connection(
        &self,
        profile: HostProfile,
        session: Session,
        consent: bool,
    ) -> Result<()> {
        if let Err(error) = self.disconnect(&profile.id).await {
            session.close().await;
            return Err(error);
        }
        let session = Arc::new(session);
        let installed = (|| -> Result<()> {
            let mut state = self.state.lock().unwrap();
            if state.hosts.len() >= 3 {
                return Err(Error::Limit("3 connected hosts"));
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
                state: ConnectionState::Connected,
                detail: None,
            }));
            let cancel = session.cancel.clone();
            let (tx, _) = watch::channel(None);
            tx.send_replace(Some(session.clone()));
            let task = tokio::spawn(connection_loop(
                profile.clone(),
                info.clone(),
                tx.clone(),
                cancel.clone(),
                consent,
                Some(session.clone()),
                Some(self.trust_file(&profile)?),
            ));
            state.hosts.insert(
                profile.id.clone(),
                Host {
                    info,
                    cancel,
                    sessions: tx,
                    task,
                    discovery: None,
                    completion: None,
                },
            );
            Ok(())
        })();
        if installed.is_err() {
            session.close().await;
        }
        installed
    }

    pub async fn forget_host(&self, host: &str) -> Result<()> {
        self.disconnect(host).await?;
        let store = self.settings.lock().unwrap();
        let mut saved = store.load()?;
        saved.hosts.retain(|p| p.id != host);
        saved.pins.retain(|p| p.host_id != host);
        saved.discovery.retain(|p| p.host_id != host);
        saved.remembered_sources.retain(|s| s.host_id != host);
        saved.workspace.sources.retain(|s| s.host_id != host);
        saved
            .workspace
            .selected_sources
            .retain(|key| saved.workspace.sources.iter().any(|s| &s.key == key));
        saved
            .workspace
            .combined_sources
            .retain(|key| saved.workspace.sources.iter().any(|s| &s.key == key));
        if saved.workspace.selected_host.as_deref() == Some(host) {
            saved.workspace.selected_host = None;
        }
        store.save(&saved)
    }
}
