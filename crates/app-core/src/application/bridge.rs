#[cfg(unix)]
use std::os::{
    fd::AsRawFd,
    unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
};
#[cfg(windows)]
use tokio::net::{TcpListener as UnixListener, TcpStream as UnixStream};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
// Private local bridge lifecycle. The adapter never owns cleanup or starts an engine.
use super::{output::*, *};
use app_protocol::*;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Semaphore,
};

#[cfg(unix)]
fn uid() -> u32 {
    unsafe { libc::geteuid() }
}
fn private(path: &Path, mode: u32) -> Result<()> {
    tailpro_platform::check_private(path, mode == 0o700)?;
    Ok(())
}
#[cfg(unix)]
pub fn runtime_dir() -> Result<PathBuf> {
    let base = if let Some(base) = std::env::var_os("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(base);
        private(&p, 0o700)?;
        p
    } else {
        let home = PathBuf::from(std::env::var_os("HOME").ok_or_else(denied)?);
        home
    };
    let path = base.join(".tailpro-bridge");
    create_private(&path)?;
    if path.as_os_str().len() + 48 > 104 {
        return Err(Error::Limit("Unix runtime path too long"));
    }
    Ok(path)
}
#[cfg(windows)]
pub fn runtime_dir() -> Result<PathBuf> {
    let base = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or_else(denied)?);
    let path = base.join("tailpro-bridge");
    create_private(&path)?;
    Ok(path)
}
fn create_private(path: &Path) -> Result<()> {
    Ok(tailpro_platform::create_private_dir(path)?)
}
fn open_private(path: &Path, create: bool) -> Result<File> {
    if path.exists() {
        private(path, 0o600)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(create).create(create);
    #[cfg(unix)]
    options
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    let file = options.open(path)?;
    #[cfg(windows)]
    if create {
        tailpro_platform::protect(path, false)?;
    }
    if !file.metadata()?.is_file() {
        return Err(denied());
    }
    private(path, 0o600)?;
    Ok(file)
}
pub struct Bridge {
    pub directory: PathBuf,
    #[cfg_attr(windows, allow(dead_code))]
    socket: PathBuf,
    #[cfg(unix)]
    inode: u64,
    _lock: File,
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
impl Bridge {
    #[cfg(unix)]
    pub async fn start(app: AppHandle) -> Result<Self> {
        app.output.maintain(app.cancel.clone());
        let directory = runtime_dir()?;
        let lock = open_private(&directory.join("instance.lock"), true)?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(Error::NeedsAction(
                "Another TailPro instance owns the local bridge".into(),
            ));
        }
        // Refused connect is the only stale proof; preserve timeout, EACCES and backlog-full sockets.
        for entry in fs::read_dir(&directory)?.take(64) {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("session-") || !name.ends_with(".sock") {
                continue;
            }
            let path = entry.path();
            let before = fs::symlink_metadata(&path)?;
            if !before.file_type().is_socket() || before.uid() != uid() {
                continue;
            }
            if let Ok(Err(e)) =
                tokio::time::timeout(Duration::from_millis(30), UnixStream::connect(&path)).await
                && e.kind() == std::io::ErrorKind::ConnectionRefused
            {
                let after = fs::symlink_metadata(&path)?;
                if after.ino() == before.ino()
                    && after.uid() == uid()
                    && after.file_type().is_socket()
                {
                    fs::remove_file(&path)?;
                }
            }
        }
        let socket = directory.join(format!("session-{}.sock", &app.output.session[..32]));
        let listener = UnixListener::bind(&socket)?;
        if unsafe { libc::listen(listener.as_raw_fd(), 8) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
        let inode = fs::symlink_metadata(&socket)?.ino();
        let registry = BridgeRegistry {
            schema_version: WIRE_VERSION,
            session: app.output.session.clone(),
            socket: socket.file_name().unwrap().to_string_lossy().into_owned(),
        };
        publish_private(
            &directory.join("registry.json"),
            &serde_json::to_vec(&registry)?,
            true,
            &app.cancel,
        )?;
        let cancel = app.cancel.child_token();
        let stop = cancel.clone();
        let task = tokio::spawn(async move {
            let sockets = Arc::new(Semaphore::new(8));
            let preauth = Arc::new(Semaphore::new(2));
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _=stop.cancelled()=>break,
                    Some(_)=tasks.join_next(), if !tasks.is_empty()=>{},
                    accepted=listener.accept()=>{
                        while tasks.try_join_next().is_some() {}
                        let Ok((stream,_))=accepted else {break;};
                        let Ok(socket_permit)=sockets.clone().try_acquire_owned() else {continue;};
                        let Ok(auth_permit)=preauth.clone().try_acquire_owned() else {continue;};
                        let app=app.clone();
                        tasks.spawn(async move {
                            let _socket=socket_permit;
                            let _=serve(stream,app,auth_permit).await;
                        });
                    }
                }
            }
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
        });
        Ok(Self {
            directory,
            socket,
            inode,
            _lock: lock,
            cancel,
            task,
        })
    }
    pub async fn close(self) {
        self.cancel.cancel();
        let _ = self.task.await;
        #[cfg(unix)]
        if fs::symlink_metadata(&self.socket)
            .is_ok_and(|m| m.ino() == self.inode && m.uid() == uid() && m.file_type().is_socket())
        {
            let _ = fs::remove_file(&self.socket);
        }
        // The exclusive lock still belongs to us until this method returns.
        let _ = fs::remove_file(self.directory.join("registry.json"));
    }
}
/// Length prefix is checked before allocation. Buffers are at most 64 KiB/request.
pub async fn read_frame(stream: &mut UnixStream, cap: usize) -> Result<Vec<u8>> {
    let len = stream.read_u32().await? as usize;
    if len == 0 || len > cap {
        return Err(Error::Limit("bridge frame"));
    }
    let mut data = vec![0; len];
    stream.read_exact(&mut data).await?;
    Ok(data)
}
pub async fn write_frame(stream: &mut UnixStream, bytes: &[u8]) -> Result<()> {
    if bytes.len() > 1024 * 1024 {
        return Err(Error::Limit("bridge response"));
    }
    stream.write_u32(bytes.len() as u32).await?;
    stream.write_all(bytes).await?;
    Ok(())
}
async fn peer_closed(stream: &UnixStream) {
    loop {
        if stream.readable().await.is_err() {
            return;
        }
        match stream.try_read(&mut [0u8; 1]) {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            // Each connection is request/response. Pipelining bytes also closes it.
            _ => return,
        }
    }
}
async fn serve(
    mut stream: UnixStream,
    app: AppHandle,
    preauth: tokio::sync::OwnedSemaphorePermit,
) -> Result<()> {
    #[cfg(unix)]
    if stream.peer_cred()?.uid() != uid() {
        return Err(denied());
    }
    let bytes = tokio::time::timeout(
        Duration::from_millis(200),
        read_frame(&mut stream, 64 * 1024),
    )
    .await
    .map_err(|_| Error::Timeout)??;
    let hello: BridgeHello = serde_json::from_slice(&bytes)?;
    drop(bytes);
    if hello.schema_version != WIRE_VERSION || hello.session != app.output.session {
        return Err(denied());
    }
    app.output
        .state
        .lock()
        .unwrap()
        .authorized(&hello.grant, &hello.token)?;
    write_frame(&mut stream, b"{\"ok\":true}").await?;
    drop(preauth);
    loop {
        let bytes = tokio::select! {
            _=app.cancel.cancelled()=>return Ok(()),
            r=tokio::time::timeout(Duration::from_secs(60),read_frame(&mut stream,64*1024))=>r.map_err(|_|Error::Timeout)??,
        };
        let req: BridgeRequest = serde_json::from_slice(&bytes)?;
        drop(bytes);
        // Four equal socket shares bound decoded+response+wire copies to the 8 MiB bridge budget.
        if req.request.max_bytes.is_some_and(|n| n > 256 * 1024) {
            return Err(Error::Limit("bridge response share 256 KiB"));
        }
        let result = tokio::select! {
            result=app.agent_read(&hello.grant,&hello.token,&req.tool,req.request)=>result,
            _=peer_closed(&stream)=>return Err(Error::Cancelled),
        };
        let reply = match result {
            Ok(result) => BridgeReply {
                result: Some(result),
                error: None,
            },
            Err(Error::Timeout) => BridgeReply {
                result: None,
                error: Some("Incomplete: deadline reached; cursor unchanged; narrow query".into()),
            },
            Err(Error::Cancelled) => BridgeReply {
                result: None,
                error: Some("Read cancelled; cursor unchanged".into()),
            },
            Err(Error::Limit(reason)) => BridgeReply {
                result: None,
                error: Some(format!("Resource limit: {reason}")),
            },
            Err(_) => BridgeReply {
                result: None,
                error: Some(
                    "Read unavailable, expired or unauthorized; requery after granting scope"
                        .into(),
                ),
            },
        };
        let payload = serde_json::to_vec(&reply)?;
        drop(reply);
        if payload.len() > 256 * 1024 + 1024 {
            return Err(Error::Limit("bridge response"));
        }
        let prefix = (payload.len() as u32).to_be_bytes();
        for chunk in [&prefix[..], &payload[..]] {
            let mut offset = 0;
            while offset < chunk.len() {
                tokio::time::timeout(Duration::from_secs(2), stream.writable())
                    .await
                    .map_err(|_| Error::Timeout)??;
                let state = app.output.state.lock().unwrap();
                state.authorized(&hello.grant, &hello.token)?;
                match stream.try_write(&chunk[offset..]) {
                    Ok(0) => return Err(denied()),
                    Ok(n) => offset += n,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
}
/// Private temp + atomic publication. Never follow the destination or overwrite implicitly.
pub fn publish_private(
    destination: &Path,
    bytes: &[u8],
    overwrite: bool,
    cancel: &CancellationToken,
) -> Result<()> {
    publish_private_checked(destination, bytes, overwrite, cancel, || Ok(()))
}
pub fn publish_private_checked<G>(
    destination: &Path,
    bytes: &[u8],
    overwrite: bool,
    cancel: &CancellationToken,
    before_publish: impl FnOnce() -> Result<G>,
) -> Result<()> {
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| Error::Invalid("Absolute destination required".into()))?;
    if !destination.is_absolute() || destination.file_name().is_none() {
        return Err(Error::Invalid("Absolute destination required".into()));
    }
    let temp = parent.join(format!(".tailpro-{}.tmp", opaque()));
    let outcome = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        let mut f = options.open(&temp)?;
        #[cfg(windows)]
        tailpro_platform::protect(&temp, false)?;
        for chunk in bytes.chunks(64 * 1024) {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            f.write_all(chunk)?;
        }
        f.sync_all()?;
        drop(f);
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let _authorization = before_publish()?;
        if overwrite {
            fs::rename(&temp, destination)?;
        } else {
            fs::hard_link(&temp, destination)?;
            fs::remove_file(&temp)?;
        }
        drop(_authorization);
        tailpro_platform::sync_parent(parent)?;
        Ok(())
    })();
    let _ = fs::remove_file(temp);
    outcome
}
// Read-only helper shared with the adapter via its own implementation, never token diagnostics.
pub fn read_registry(path: &Path) -> Result<BridgeRegistry> {
    private(path.parent().ok_or_else(denied)?, 0o700)?;
    let f = open_private(path, false)?;
    if f.metadata()?.len() > 4096 {
        return Err(Error::Limit("registry"));
    }
    let mut bytes = Vec::new();
    f.take(4097).read_to_end(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(windows)]
impl Bridge {
    pub async fn start(app: AppHandle) -> Result<Self> {
        app.output.maintain(app.cancel.clone());
        let directory = runtime_dir()?;
        let lock = open_private(&directory.join("instance.lock"), true)?;
        lock.try_lock().map_err(|_| {
            Error::NeedsAction("Another TailPro instance owns the local bridge".into())
        })?;
        let listener = UnixListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let port = listener.local_addr()?.port();
        let socket = directory.join(format!("session-{}.{port}.sock", &app.output.session[..32]));
        let registry = BridgeRegistry {
            schema_version: WIRE_VERSION,
            session: app.output.session.clone(),
            socket: socket.file_name().unwrap().to_string_lossy().into_owned(),
        };
        publish_private(
            &directory.join("registry.json"),
            &serde_json::to_vec(&registry)?,
            true,
            &app.cancel,
        )?;
        let cancel = app.cancel.child_token();
        let stop = cancel.clone();
        let task = tokio::spawn(async move {
            let sockets = Arc::new(Semaphore::new(8));
            let preauth = Arc::new(Semaphore::new(2));
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    Some(_) = tasks.join_next(), if !tasks.is_empty() => {},
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break; };
                        let Ok(socket_permit) = sockets.clone().try_acquire_owned() else { continue; };
                        let Ok(auth_permit) = preauth.clone().try_acquire_owned() else { continue; };
                        let app = app.clone(); tasks.spawn(async move { let _socket = socket_permit; let _ = serve(stream, app, auth_permit).await; });
                    }
                }
            }
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
        });
        Ok(Self {
            directory,
            socket,
            _lock: lock,
            cancel,
            task,
        })
    }
}
