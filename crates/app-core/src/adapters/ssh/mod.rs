#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
pub mod auth;
pub mod config;
use crate::api::{Error, HostProfile, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
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

// Cancelled local config reads may finish in the blocking pool; keep their
// permits until the read ends so repeated tests cannot queue unbounded work.
static CONNECT_CONFIG_READS: Semaphore = Semaphore::const_new(4);

pub struct Session {
    pub profile: HostProfile,
    fingerprint: String,
    trust_file: Option<PathBuf>,
    known_hosts: Option<String>,
    root: PathBuf,
    config: PathBuf,
    socket: PathBuf,
    master: tokio::sync::Mutex<Child>,
    diagnostic: tokio::task::JoinHandle<()>,
    failure: Arc<std::sync::atomic::AtomicU8>,
    ready: Arc<std::sync::atomic::AtomicBool>,
    pub cancel: CancellationToken,
    channels: Arc<Semaphore>,
    pub probes: Arc<Semaphore>,
}
pub(crate) fn validate_profile(profile: &HostProfile) -> Result<()> {
    if profile.id.is_empty()
        || profile.id.len() > 128
        || profile.id.chars().any(char::is_control)
        || !config::valid_target(&profile.target)
        || profile.port == Some(0)
        || profile
            .user
            .as_ref()
            .is_some_and(|user| !config::valid_target(user))
    {
        return Err(Error::Invalid("invalid host profile".into()));
    }
    if let Some(agent) = &profile.identity_agent {
        local_path(agent)?;
    }
    if let Some(path) = &profile.identity_file {
        local_path(path)?;
    }
    if profile.authentication == crate::api::Authentication::PrivateKey
        && profile.identity_file.is_none()
    {
        return Err(Error::Invalid("Choose a private key file".into()));
    }
    if let Some(jump) = &profile.proxy_jump
        && (jump.len() > 2048
            || jump.split(',').count() > 8
            || jump.split(',').any(|p| !config::valid_target(p)))
    {
        return Err(Error::Invalid(
            "Use SSH aliases or literal [user@]host[:port] for jump hosts".into(),
        ));
    }
    #[cfg(windows)]
    if profile.authentication == crate::api::Authentication::Interactive {
        return Err(Error::NeedsAction("Windows readers require key/SSH-agent authentication. Load an encrypted key into the Windows OpenSSH agent, then choose SSH config / agent. Password and keyboard-interactive sessions require macOS or Linux.".into()));
    }
    Ok(())
}
fn local_path(value: &str) -> Result<()> {
    if value.len() > 4096
        || !std::path::Path::new(value).is_absolute()
        || value.chars().any(char::is_control)
    {
        return Err(Error::Invalid("Choose an absolute local path".into()));
    }
    Ok(())
}

struct PendingRuntime {
    root: PathBuf,
    armed: bool,
}
impl Drop for PendingRuntime {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(self.root.join("mux"));
            let _ = fs::remove_file(self.root.join("config"));
            let _ = fs::remove_dir(&self.root);
        }
    }
}
impl Session {
    pub async fn connect(
        profile: HostProfile,
        cancel: CancellationToken,
        consent: bool,
    ) -> Result<Self> {
        Self::connect_managed(profile, cancel, consent, None, None).await
    }
    pub(crate) async fn connect_managed(
        profile: HostProfile,
        cancel: CancellationToken,
        consent: bool,
        trust_file: Option<PathBuf>,
        challenges: Option<Arc<auth::ChallengeState>>,
    ) -> Result<Self> {
        validate_profile(&profile)?;
        let config_permit = tokio::select! {
            _ = cancel.cancelled() => return Err(Error::Cancelled),
            permit = CONNECT_CONFIG_READS.acquire() => permit.map_err(|_| Error::Cancelled)?,
        };
        let manifest = tokio::select! {
            _ = cancel.cancelled() => return Err(Error::Cancelled),
            loaded = tokio::task::spawn_blocking(move || {
                let _permit = config_permit;
                config::load()
            }) => loaded.map_err(|_| Error::Cancelled)??,
        };
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if manifest.needs_consent(&profile.target)? && !consent {
            return Err(Error::NeedsAction(manifest.disclosures.join("; ")));
        }
        let mut trust_paths = Vec::new();
        if let Some(path) = &trust_file {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
                tailpro_platform::protect(parent, true)?;
            }
            if path.exists() {
                tailpro_platform::check_private(path, false)?;
            }
            let mut options = fs::OpenOptions::new();
            options.create(true).append(true);
            #[cfg(unix)]
            options.mode(0o600);
            options.open(path)?;
            tailpro_platform::protect(path, false)?;
            trust_paths.push(path.clone());
            trust_paths.extend(manifest.user_trust_paths(&profile.target)?);
        }
        let known_hosts = (!trust_paths.is_empty()).then(|| {
            trust_paths
                .iter()
                .map(|p| {
                    format!(
                        "\"{}\"",
                        p.to_string_lossy()
                            .replace('\\', "\\\\")
                            .replace('\"', "\\\"")
                    )
                })
                .collect::<Vec<_>>()
                .join(" ")
        });
        let askpass = challenges.map(auth::AskpassBridge::start).transpose()?;
        let nonce: u64 = rand::random();
        let root = std::env::temp_dir().join(format!("tailpro-{}-{nonce:x}", std::process::id()));
        tailpro_platform::create_private_dir(&root)?;
        let mut pending_runtime = PendingRuntime {
            root: root.clone(),
            armed: true,
        };
        let config = root.join("config");
        let enforced = "Host *\n StrictHostKeyChecking yes\n UpdateHostKeys no\n ForwardAgent no\n Tunnel no\n GSSAPIDelegateCredentials no\n ForwardX11 no\n PermitLocalCommand no\n ClearAllForwardings yes\n RequestTTY no\n BatchMode yes\n ControlMaster no\n ControlPath none\n ConnectTimeout 10\n ServerAliveInterval 15\n ServerAliveCountMax 3\n";
        fs::write(&config, format!("{enforced}{}", manifest.text))?;
        tailpro_platform::protect(&config, false)?;
        let socket = root.join("mux");
        let session_fingerprint =
            fingerprint(&profile, &manifest.fingerprint, trust_file.as_deref())?;
        let mut command = Self::base(
            &profile,
            &config,
            &socket,
            askpass.is_some(),
            known_hosts.as_deref(),
        );
        if let Some(bridge) = &askpass {
            command
                .env("SSH_ASKPASS", std::env::current_exe()?)
                .env("SSH_ASKPASS_REQUIRE", "force")
                .env("TAILPRO_ASKPASS_SOCKET", &bridge.socket)
                .env("TAILPRO_ASKPASS_TOKEN", &bridge.token)
                .env("LC_ALL", "C");
        }
        #[cfg(unix)]
        command.args([
            "-M",
            "-N",
            "-o",
            "ControlMaster=yes",
            "-o",
            "ControlPersist=no",
        ]);
        #[cfg(windows)]
        command
            .arg(&profile.target)
            .arg("printf '__TAILPRO_READY__\n'; exec sleep 2147483647");
        #[cfg(unix)]
        command.arg(&profile.target);
        command
            .stdout(if cfg!(windows) {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut master = command.spawn()?;
        let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
        #[cfg(windows)]
        {
            let mut stdout = master.stdout.take().unwrap();
            let ready = ready.clone();
            tokio::spawn(async move {
                let mut marker = [0_u8; 18];
                if stdout.read_exact(&mut marker).await.is_ok() && &marker == b"__TAILPRO_READY__\n"
                {
                    ready.store(true, std::sync::atomic::Ordering::Release);
                }
            });
        }
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
            fingerprint: session_fingerprint,
            trust_file,
            known_hosts,
            root,
            config,
            socket,
            master: tokio::sync::Mutex::new(master),
            diagnostic,
            failure,
            ready,
            cancel,
            channels: Arc::new(Semaphore::new(6)),
            probes: Arc::new(Semaphore::new(2)),
        };
        pending_runtime.armed = false;
        let result = session
            .wait_ready(if askpass.is_some() {
                auth::ATTEMPT_DEADLINE
            } else {
                Duration::from_secs(11)
            })
            .await;
        if let Err(error) = result {
            session.close().await;
            return Err(error);
        }
        // Explicit fingerprint confirmation may have appended a new host key.
        let refreshed = config::load().and_then(|manifest| {
            fingerprint(
                &session.profile,
                &manifest.fingerprint,
                session.trust_file.as_deref(),
            )
        });
        match refreshed {
            Ok(identity) => session.fingerprint = identity,
            Err(error) => {
                session.close().await;
                return Err(error);
            }
        }
        Ok(session)
    }
    fn base(
        profile: &HostProfile,
        config: &PathBuf,
        socket: &PathBuf,
        interactive: bool,
        known_hosts: Option<&str>,
    ) -> Command {
        let mut c = Command::new(config::ssh_binary());
        #[cfg(windows)]
        c.creation_flags(0x08000000);
        c.arg("-F").arg(config);
        #[cfg(unix)]
        c.arg("-S").arg(socket);
        #[cfg(windows)]
        let _ = socket;
        c.arg("-o").arg(if interactive {
            "BatchMode=no"
        } else {
            "BatchMode=yes"
        });
        c.arg("-o").arg(if interactive {
            "StrictHostKeyChecking=ask"
        } else {
            "StrictHostKeyChecking=yes"
        });
        c.args(["-o", "FingerprintHash=sha256", "-o", "VerifyHostKeyDNS=no"]);
        if let Some(paths) = known_hosts {
            c.arg("-o").arg(format!("UserKnownHostsFile={paths}"));
        }
        for option in [
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
        if let Some(jump) = &profile.proxy_jump {
            c.arg("-J").arg(jump);
        }
        if profile.authentication == crate::api::Authentication::PrivateKey
            && let Some(path) = &profile.identity_file
        {
            c.arg("-i").arg(path).args(["-o", "IdentitiesOnly=yes"]);
        }
        if profile.authentication != crate::api::Authentication::Interactive {
            c.args([
                "-o",
                "PasswordAuthentication=no",
                "-o",
                "KbdInteractiveAuthentication=no",
            ]);
        } else {
            c.args([
                "-o",
                "PreferredAuthentications=keyboard-interactive,password,publickey",
            ]);
        }
        c.stdin(Stdio::null());
        c
    }
    async fn wait_ready(&mut self, deadline: Duration) -> Result<()> {
        let until = Instant::now() + deadline;
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
            if self.socket.exists() || self.ready.load(std::sync::atomic::Ordering::Acquire) {
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
        if fingerprint(
            &self.profile,
            &manifest.fingerprint,
            self.trust_file.as_deref(),
        )? != self.fingerprint
        {
            return Err(Error::NeedsAction(
                "SSH config/trust changed; disconnect and reconnect".into(),
            ));
        }
        tailpro_platform::check_private(&self.root, true)?;
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
        let mut c = Self::base(
            &self.profile,
            &self.config,
            &self.socket,
            false,
            self.known_hosts.as_deref(),
        );
        // ProxyCommand=false prevents a lost mux socket silently opening a fresh connection.
        #[cfg(unix)]
        c.args(["-o", "ProxyCommand=false"]);
        c.arg(&self.profile.target).arg(remote);
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
fn fingerprint(
    profile: &HostProfile,
    manifest: &str,
    trust: Option<&std::path::Path>,
) -> Result<String> {
    // Keep legacy settings/pins valid when all newly added auth options are default.
    #[derive(serde::Serialize)]
    struct Identity<'a> {
        id: &'a str,
        target: &'a str,
        port: Option<u16>,
        user: &'a Option<String>,
        identity_agent: &'a Option<String>,
        environment: &'a crate::api::Environment,
    }
    let identity = Identity {
        id: &profile.id,
        target: &profile.target,
        port: profile.port,
        user: &profile.user,
        identity_agent: &profile.identity_agent,
        environment: &profile.environment,
    };
    let mut h = Sha256::new();
    h.update(manifest);
    h.update(serde_json::to_vec(&identity)?);
    if profile.authentication != crate::api::Authentication::ConfigAgent
        || profile.identity_file.is_some()
        || profile.proxy_jump.is_some()
    {
        h.update(serde_json::to_vec(&(
            &profile.authentication,
            &profile.identity_file,
            &profile.proxy_jump,
        ))?);
    }
    if let Some(path) = trust
        && path.exists()
    {
        let meta = fs::symlink_metadata(path)?;
        if !meta.is_file() || meta.len() > 1024 * 1024 {
            return Err(Error::Limit("app known_hosts file"));
        }
        let bytes = fs::read(path)?;
        if !bytes.is_empty() {
            h.update(bytes);
        }
    }
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
