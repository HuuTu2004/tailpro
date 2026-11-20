use super::*;

struct TestRegistration(Arc<Mutex<Option<CancellationToken>>>);
impl Drop for TestRegistration {
    fn drop(&mut self) {
        if let Some(cancel) = self.0.lock().unwrap().take() {
            cancel.cancel();
        }
    }
}

impl AppHandle {
    /// Persists a profile without connecting, replacing sessions or changing grants.
    pub fn save_host_profile(&self, profile: HostProfile) -> Result<()> {
        crate::adapters::ssh::validate_profile(&profile)?;
        let store = self.settings.lock().unwrap();
        let mut saved = store.load()?;
        saved.hosts.retain(|host| host.id != profile.id);
        saved.hosts.push(profile);
        store.save(&saved)
    }

    pub async fn test_connection(&self, profile: HostProfile, consent: bool) -> Result<()> {
        self.test_connection_with_cancel(profile, consent, CancellationToken::new())
            .await
    }

    pub fn cancel_connection_test(&self) {
        if let Some(cancel) = self.connection_test_cancel.lock().unwrap().as_ref() {
            cancel.cancel();
        }
    }

    /// A cancelled caller may drop its future; the owned task still closes/reaps its
    /// isolated session. The caller's token supports immediate explicit cancellation.
    pub async fn test_connection_with_cancel(
        &self,
        profile: HostProfile,
        consent: bool,
        cancel: CancellationToken,
    ) -> Result<()> {
        crate::adapters::ssh::validate_profile(&profile)?;
        let permit = self
            .connection_tests
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Limit("one connection test at a time"))?;
        let shutdown = self.cancel.clone();
        let owned_cancel = CancellationToken::new();
        *self.connection_test_cancel.lock().unwrap() = Some(owned_cancel.clone());
        let registration = TestRegistration(self.connection_test_cancel.clone());
        tokio::spawn(async move {
            let _permit = permit;
            let _registration = registration;
            let session_cancel = CancellationToken::new();
            let connect = Session::connect(profile, session_cancel.clone(), consent);
            tokio::pin!(connect);
            let outcome = tokio::select! {
                biased;
                _ = cancel.cancelled() => Error::Cancelled,
                _ = owned_cancel.cancelled() => Error::Cancelled,
                _ = shutdown.cancelled() => Error::Cancelled,
                _ = tokio::time::sleep(Duration::from_secs(12)) => Error::Timeout,
                result = &mut connect => {
                    let session = result?;
                    session.close().await;
                    if cancel.is_cancelled() || owned_cancel.is_cancelled() || shutdown.is_cancelled() {
                        return Err(Error::Cancelled);
                    }
                    return Ok(());
                }
            };
            session_cancel.cancel();
            // Session::connect cooperatively cleans up its own failed/cancelled setup.
            // If it won the race, close the successful temporary session here too.
            if let Ok(session) = connect.await {
                session.close().await;
            }
            Err(outcome)
        })
        .await
        .map_err(|_| Error::Cancelled)?
    }
}
