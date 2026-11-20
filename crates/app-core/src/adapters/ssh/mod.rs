pub mod config;
use crate::{
    api::{Error, HostProfile, Result},
    application::policy,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::PathBuf,
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
    sync::Semaphore,
};
use tokio_util::sync::CancellationToken;

pub struct Session {
    pub profile: HostProfile,
    fingerprint: String,
    root: PathBuf,
    config: PathBuf,
    socket: PathBuf,
    master: tokio::sync::Mutex<Child>,
    diagnostic: tokio::task::JoinHandle<()>,
    failure: Arc<std::sync::atomic::AtomicU8>,
    pub cancel: CancellationToken,
    channels: Arc<Semaphore>,
    pub probes: Arc<Semaphore>,
}
impl Session {
    pub async fn connect(
        profile: HostProfile,
        cancel: CancellationToken,
        consent: bool,
    ) -> Result<Self> {
        if !config::valid_target(&profile.target)
            || profile.id.len() > 128
            || profile
                .user
                .as_ref()
                .is_some_and(|u| !config::valid_target(u))
        {
            return Err(Error::Invalid("invalid host profile".into()));
        }
        if let Some(agent) = &profile.identity_agent {
            policy::path(agent)?;
        }
        let manifest = tokio::task::spawn_blocking(config::load)
            .await
            .map_err(|_| Error::Cancelled)??;
        if manifest.needs_consent(&profile.target)? && !consent {
            return Err(Error::NeedsAction(manifest.disclosures.join("; ")));
        }
        let nonce: u64 = rand::random();
        let root = std::env::temp_dir().join(format!("tailpro-{}-{nonce:x}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&root)?;
        let config = root.join("config");
        let enforced = "Host *\n StrictHostKeyChecking yes\n UpdateHostKeys no\n ForwardAgent no\n Tunnel no\n GSSAPIDelegateCredentials no\n ForwardX11 no\n PermitLocalCommand no\n ClearAllForwardings yes\n RequestTTY no\n BatchMode yes\n ControlMaster no\n ControlPath none\n ConnectTimeout 10\n ServerAliveInterval 15\n ServerAliveCountMax 3\n";
        fs::write(&config, format!("{enforced}{}", manifest.text))?;
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600))?;
        let socket = root.join("mux");
        let fingerprint = fingerprint(&profile, &manifest.fingerprint)?;
        let mut command = Self::base(&profile, &config, &socket);
        command.args([
            "-M",
            "-N",
            "-o",
            "ControlMaster=yes",
            "-o",
            "ControlPersist=no",
        ]);
        command
            .arg(&profile.target)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut master = command.spawn()?;
        let mut stderr = master.stderr.take().unwrap();
        let failure = Arc::new(std::sync::atomic::AtomicU8::new(0));
        let flags = failure.clone();
        let diagnostic = tokio::spawn(async move {
            let mut chunk = [0_u8; 1024];
            let mut tail = Vec::new();
            while let Ok(n) = stderr.read(&mut chunk).await {
                if n == 0 {
                    break;
                }
                tail.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&tail).to_ascii_lowercase();
                let value = if [
                    "permission denied",
                    "host key verification failed",
                    "identification has changed",
                    "bad configuration",
                    "bad owner",
                    "no such identity",
                ]
                .iter()
                .any(|word| text.contains(word))
                {
                    1
                } else if [
                    "connection refused",
                    "connection timed out",
                    "no route to host",
                    "network is unreachable",
                    "could not resolve hostname",
                    "connection reset",
                ]
                .iter()
                .any(|word| text.contains(word))
                {
                    2
                } else {
                    0
                };
                if value == 1
                    || (value == 2 && flags.load(std::sync::atomic::Ordering::Relaxed) != 1)
                {
                    flags.store(value, std::sync::atomic::Ordering::Relaxed);
                }
                if tail.len() > 256 {
                    tail.drain(..tail.len() - 256);
                }
            }
        });
        let mut session = Self {
            profile,
            fingerprint,
            root,
            config,
            socket,
            master: tokio::sync::Mutex::new(master),
            diagnostic,
            failure,
            cancel,
            channels: Arc::new(Semaphore::new(6)),
            probes: Arc::new(Semaphore::new(2)),
        };
        let result = session.wait_ready().await;
        if let Err(error) = result {
            session.close().await;
            return Err(error);
        }
        Ok(session)
    }
    fn base(profile: &HostProfile, config: &PathBuf, socket: &PathBuf) -> Command {
        let mut c = Command::new("/usr/bin/ssh");
        c.arg("-F").arg(config).arg("-S").arg(socket);
        for option in [
            "BatchMode=yes",
            "StrictHostKeyChecking=yes",
            "UpdateHostKeys=no",
            "ForwardAgent=no",
            "Tunnel=no",
            "GSSAPIDelegateCredentials=no",
            "ForwardX11=no",
            "ClearAllForwardings=yes",
            "PermitLocalCommand=no",
            "RequestTTY=no",
            "ConnectTimeout=10",
            "ServerAliveInterval=15",
            "ServerAliveCountMax=3",
            "ControlMaster=no",
        ] {
            c.arg("-o").arg(option);
        }
        if let Some(port) = profile.port {
            c.arg("-p").arg(port.to_string());
        }
        if let Some(user) = &profile.user {
            c.arg("-l").arg(user);
        }
        if let Some(agent) = &profile.identity_agent {
            c.arg("-o").arg(format!("IdentityAgent={agent}"));
        }
        c.stdin(Stdio::null());
        c
    }
    async fn wait_ready(&mut self) -> Result<()> {
        let until = Instant::now() + Duration::from_secs(11);
        loop {
            if self.cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if self.master.lock().await.try_wait()?.is_some() {
                let _ =
                    tokio::time::timeout(Duration::from_millis(100), &mut self.diagnostic).await;
                if self.failure.load(std::sync::atomic::Ordering::Relaxed) == 2 {
                    return Err(std::io::Error::from(std::io::ErrorKind::ConnectionAborted).into());
                }
                return Err(Error::NeedsAction("SSH authentication/host-key/config failed. Verify the host with your SSH client; TailPro never accepts host keys automatically.".into()));
            }
            if self.socket.exists() {
                return Ok(());
            }
            if Instant::now() >= until {
                return Err(Error::Timeout);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    pub fn remote_identity(&self) -> &str {
        &self.fingerprint
    }
    pub async fn validate(&self) -> Result<()> {
        if self.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let manifest = tokio::task::spawn_blocking(config::load)
            .await
            .map_err(|_| Error::Cancelled)??;
        if fingerprint(&self.profile, &manifest.fingerprint)? != self.fingerprint {
            return Err(Error::NeedsAction(
                "SSH config/trust changed; disconnect and reconnect".into(),
            ));
        }
        let meta = fs::symlink_metadata(&self.root)?;
        if !meta.is_dir()
            || meta.permissions().mode() & 0o077 != 0
            || meta.uid() != fs::metadata("/proc/self")?.uid()
        {
            return Err(Error::NeedsAction("unsafe SSH runtime directory".into()));
        }
        Ok(())
    }
    // Kept crate-private: only application allowlisted templates may reach this boundary.
    pub(crate) async fn spawn(
        &self,
        operation: super::sources::RemoteOperation,
    ) -> Result<OwnedChannel> {
        self.validate().await?;
        let permit = tokio::select! { _ = self.cancel.cancelled() => return Err(Error::Cancelled), p = self.channels.clone().acquire_owned() => p.map_err(|_| Error::Cancelled)? };
        let remote = operation.render()?;
        let mut c = Self::base(&self.profile, &self.config, &self.socket);
        // ProxyCommand=false prevents a lost mux socket silently opening a fresh connection.
        c.args(["-o", "ProxyCommand=false"])
            .arg(&self.profile.target)
            .arg(remote);
        let child = c
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        Ok(OwnedChannel {
            child,
            _permit: permit,
        })
    }
    pub async fn alive(&self) -> bool {
        !self.cancel.is_cancelled() && self.master.lock().await.try_wait().ok().flatten().is_none()
    }
    pub async fn close(&self) {
        self.cancel.cancel();
        let mut master = self.master.lock().await;
        let _ = master.kill().await;
        let _ = master.wait().await;
        let _ = fs::remove_file(&self.socket);
        let _ = fs::remove_file(&self.config);
        let _ = fs::remove_dir(&self.root);
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.diagnostic.abort();
        let _ = self.master.get_mut().start_kill();
        let _ = fs::remove_file(&self.socket);
        let _ = fs::remove_file(&self.config);
        let _ = fs::remove_dir(&self.root);
    }
}
fn fingerprint(profile: &HostProfile, manifest: &str) -> Result<String> {
    let mut h = Sha256::new();
    h.update(manifest);
    h.update(serde_json::to_vec(profile)?);
    Ok(format!("{:x}", h.finalize()))
}
pub(crate) struct OwnedChannel {
    pub child: Child,
    _permit: tokio::sync::OwnedSemaphorePermit,
}
impl OwnedChannel {
    pub async fn close(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}
pub(crate) async fn bounded_read(
    mut reader: impl AsyncRead + Unpin,
    cap: usize,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    (&mut reader)
        .take((cap + 1) as u64)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > cap {
        return Err(Error::Limit("remote output"));
    }
    Ok(bytes)
}
