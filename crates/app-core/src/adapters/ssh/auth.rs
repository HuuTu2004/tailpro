//! Connection-scoped OpenSSH askpass bridge. Credentials never enter settings or argv.
use crate::api::{AuthenticationChallenge, ConnectionAttempt, Error, Result};
use std::{
    io::{BufRead, Read, Write},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::oneshot,
};
use tokio_util::sync::CancellationToken;

const FRAME_CAP: u64 = 12 * 1024;
pub(crate) const ATTEMPT_DEADLINE: Duration = Duration::from_secs(180);

pub(crate) struct Secret(Vec<u8>);
impl Drop for Secret {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

pub(crate) struct ChallengeState {
    pub view: Mutex<ConnectionAttempt>,
    reply: Mutex<Option<(String, oneshot::Sender<Secret>)>>,
    pub cancel: CancellationToken,
}
impl ChallengeState {
    pub fn new(id: String, profile_id: String, test_only: bool) -> Arc<Self> {
        Arc::new(Self {
            view: Mutex::new(ConnectionAttempt {
                id,
                profile_id,
                state: "authenticating".into(),
                challenge: None,
                error: None,
                test_only,
                capabilities: None,
            }),
            reply: Mutex::new(None),
            cancel: CancellationToken::new(),
        })
    }
    pub fn answer(&self, challenge_id: &str, mut value: String, verified: bool) -> Result<()> {
        if value.len() > 8192 || value.contains(['\n', '\r', '\0']) {
            return Err(Error::Invalid(
                "Authentication response exceeds bounds".into(),
            ));
        }
        let mut view = self.view.lock().unwrap();
        let challenge = view
            .challenge
            .as_ref()
            .filter(|c| c.id == challenge_id)
            .ok_or_else(|| Error::NeedsAction("Authentication request expired".into()))?;
        if challenge.kind == "host_key" {
            if !verified || challenge.fingerprint.as_deref() != Some(value.as_str()) {
                return Err(Error::NeedsAction(
                    "Verify and enter the exact fingerprint from a trusted source".into(),
                ));
            }
            value = "yes".into();
        } else if challenge.kind == "confirmation" {
            if !verified {
                return Err(Error::NeedsAction(
                    "Confirm this authentication request explicitly".into(),
                ));
            }
            value = "yes".into();
        }
        let (_, reply) = self
            .reply
            .lock()
            .unwrap()
            .take()
            .filter(|(id, _)| id == challenge_id)
            .ok_or_else(|| Error::NeedsAction("Authentication request expired".into()))?;
        view.challenge = None;
        reply
            .send(Secret(value.into_bytes()))
            .map_err(|_| Error::Cancelled)
    }
    pub fn finish(&self, state: &str, error: Option<String>) {
        self.reply.lock().unwrap().take();
        let mut view = self.view.lock().unwrap();
        view.state = state.into();
        view.error = error;
        view.challenge = None;
    }
}

pub(crate) struct AskpassBridge {
    pub socket: String,
    pub token: String,
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
impl AskpassBridge {
    pub fn start(state: Arc<ChallengeState>) -> Result<Self> {
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let socket = listener.local_addr()?.to_string();
        let listener = TcpListener::from_std(listener)?;
        let token = format!("{:032x}", rand::random::<u128>());
        let expected = token.clone();
        let cancel = state.cancel.child_token();
        let stop = cancel.clone();
        let task = tokio::spawn(async move {
            let mut count = 0;
            loop {
                let accepted = tokio::select! { _ = stop.cancelled() => break, result = listener.accept() => result };
                let Ok((stream, _)) = accepted else {
                    break;
                };
                let (read, mut write) = stream.into_split();
                let mut bytes = Vec::new();
                let mut reader = BufReader::new(tokio::io::AsyncReadExt::take(read, FRAME_CAP));
                let read = tokio::select! { _ = stop.cancelled() => break, value = tokio::time::timeout(Duration::from_secs(2), reader.read_until(b'\n', &mut bytes)) => value };
                if !matches!(read, Ok(Ok(_))) || bytes.last() != Some(&b'\n') {
                    continue;
                }
                let Ok(request) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                    continue;
                };
                if request["token"].as_str() != Some(expected.as_str()) {
                    continue;
                }
                count += 1;
                if count > 12 {
                    state.finish(
                        "failed",
                        Some("Authentication challenge limit reached".into()),
                    );
                    state.cancel.cancel();
                    break;
                }
                let prompt = request["prompt"].as_str().unwrap_or("");
                if prompt.is_empty() || prompt.len() > 4096 {
                    continue;
                }
                let hint = request["hint"].as_str().unwrap_or("");
                let fingerprint = if prompt.starts_with("The authenticity of host ")
                    && prompt.contains("key fingerprint")
                    && prompt.contains("continue connecting")
                {
                    prompt
                        .split_whitespace()
                        .find(|part| part.starts_with("SHA256:"))
                        .map(|value| value.trim_end_matches('.').to_owned())
                } else {
                    None
                };
                let kind = if fingerprint.is_some() {
                    "host_key"
                } else if hint == "confirm" {
                    "confirmation"
                } else {
                    "secret"
                };
                let id = format!("challenge-{:x}", rand::random::<u128>());
                let (tx, rx) = oneshot::channel();
                {
                    let mut view = state.view.lock().unwrap();
                    *state.reply.lock().unwrap() = Some((id.clone(), tx));
                    view.challenge = Some(AuthenticationChallenge {
                        id: id.clone(),
                        kind: kind.into(),
                        prompt: prompt.into(),
                        fingerprint,
                    });
                }
                let answer = tokio::select! { _ = stop.cancelled() => None, value = tokio::time::timeout(Duration::from_secs(90), rx) => value.ok().and_then(|value| value.ok()) };
                {
                    let mut view = state.view.lock().unwrap();
                    state.reply.lock().unwrap().take();
                    if view.challenge.as_ref().is_some_and(|c| c.id == id) {
                        view.challenge = None;
                    }
                }
                let Some(answer) = answer else {
                    state.cancel.cancel();
                    break;
                };
                let _ = write.write_all(&answer.0).await;
                let _ = write.write_all(b"\n").await;
                let _ = write.shutdown().await;
            }
        });
        Ok(Self {
            socket,
            token,
            cancel,
            task,
        })
    }
}
impl Drop for AskpassBridge {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.task.abort();
    }
}

/// Called before desktop bootstrap only in the app-owned askpass child process.
/// The sole stdout consumer is OpenSSH; never print diagnostics or credentials elsewhere.
pub fn askpass_main() -> i32 {
    fn run() -> std::io::Result<()> {
        let path = std::env::var("TAILPRO_ASKPASS_SOCKET")
            .ok()
            .ok_or(std::io::ErrorKind::PermissionDenied)?;
        let token = std::env::var("TAILPRO_ASKPASS_TOKEN")
            .map_err(|_| std::io::ErrorKind::PermissionDenied)?;
        let prompt = std::env::args().nth(1).unwrap_or_default();
        if prompt.len() > 4096 || token.len() != 32 {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        let address: std::net::SocketAddr =
            path.parse().map_err(|_| std::io::ErrorKind::InvalidInput)?;
        if !address.ip().is_loopback() {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        let mut stream = std::net::TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
        stream.set_read_timeout(Some(Duration::from_secs(95)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let request = serde_json::json!({"token":token,"prompt":prompt,"hint":std::env::var("SSH_ASKPASS_PROMPT").unwrap_or_default()});
        stream.write_all(&serde_json::to_vec(&request)?)?;
        stream.write_all(b"\n")?;
        let mut answer = Secret(Vec::new());
        std::io::BufReader::new(stream.take(FRAME_CAP)).read_until(b'\n', &mut answer.0)?;
        if answer.0.last() != Some(&b'\n') || answer.0.len() > 8193 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        std::io::stdout().lock().write_all(&answer.0)?;
        Ok(())
    }
    if run().is_ok() { 0 } else { 1 }
}
