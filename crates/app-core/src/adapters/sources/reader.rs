use super::{Checkpoints, FileCheckpoint, FilePartial, Position, RemoteOperation};
use crate::{
    adapters::ssh::Session,
    api::{Error, Result, SourceInfo, SourceSpec},
    application::now_ms,
    workers::Ingest,
};
use log_core::{
    model::{Event, RecordEnding, Stream},
    parse::{Assembler, Frame, Framer, normalize},
};
use serde_json::Value;
use std::{
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    sync::watch,
};
use tokio_util::sync::CancellationToken;

pub(crate) struct Reader {
    pub source: String,
    pub generation: u64,
    pub spec: SourceSpec,
    pub ingest: Ingest,
    pub cancel: CancellationToken,
    pub info: Arc<Mutex<SourceInfo>>,
    pub sessions: watch::Receiver<Option<Arc<Session>>>,
}
pub(crate) async fn run(mut reader: Reader) {
    let mut checkpoint = Checkpoints::new(reader.generation);
    let mut interrupted = None::<Instant>;
    let mut attempts = 0_u32;
    loop {
        if reader.cancel.is_cancelled() {
            break;
        }
        let session = reader.sessions.borrow_and_update().clone();
        let Some(session) = session else {
            reader.state("waiting_for_connection", None);
            tokio::select! { _ = reader.cancel.cancelled() => break, result = reader.sessions.changed() => if result.is_err() { break; } }
            continue;
        };
        reader.state("opening", None);
        let opened = Instant::now();
        let result = match &reader.spec {
            SourceSpec::Docker { container_id, .. } => {
                docker(&reader, &session, container_id, &mut checkpoint).await
            }
            SourceSpec::Kubernetes { .. } => kubernetes(&reader, &session, &mut checkpoint).await,
            SourceSpec::File { path } => file(&reader, &session, path, &mut checkpoint).await,
            SourceSpec::Journal { unit } => {
                super::journal::read(&reader, &session, unit, &mut checkpoint).await
            }
        };
        if reader.cancel.is_cancelled() {
            break;
        }
        if matches!(
            result,
            Err(Error::NeedsAction(_)) | Err(Error::Invalid(_)) | Err(Error::Limit(_))
        ) {
            let detail = match &result {
                Err(Error::NeedsAction(reason)) => reason.clone(),
                _ => "Source protocol/capability or budget failure; continuity unknown".into(),
            };
            reader.state("needs_action", Some(detail));
            return;
        }
        if opened.elapsed() >= Duration::from_secs(1) {
            interrupted = None;
            attempts = 0;
        }
        let outage = interrupted.get_or_insert_with(Instant::now);
        if outage.elapsed() > Duration::from_secs(300) {
            reader.state(
                "needs_action",
                Some("Source reconnect deadline reached".into()),
            );
            return;
        }
        checkpoint.reconnect();
        reader.state(
            "reconnecting",
            Some("Continuity uncertain; replayed records are retained".into()),
        );
        // Preserve committed cursor only; pending records replay after reconnect.
        let _ = reader
            .marker(
                "Connection interrupted; continuity uncertain",
                &mut checkpoint,
            )
            .await;
        tokio::select! { _ = reader.cancel.cancelled() => break, _ = tokio::time::sleep(Duration::from_millis((1_u64 << attempts.min(5)).min(30)*1000+rand::random::<u64>()%500)) => {} }
        attempts += 1;
    }
    reader.state("stopped", None);
}
impl Reader {
    pub(super) fn state(&self, state: &str, detail: Option<String>) {
        let mut info = self.info.lock().unwrap();
        info.state = state.into();
        info.detail = detail;
    }
    pub(super) async fn admit(
        &self,
        events: Vec<Event>,
        checkpoint: &mut Checkpoints,
    ) -> Result<()> {
        for event in events {
            // Docker has no mid-record cursor. Its one pending parent fences
            // later stream ACKs until the full record has terminal dispositions.
            let hold = !event.checkpoint_complete
                || (matches!(
                    self.spec,
                    SourceSpec::Docker { .. } | SourceSpec::Kubernetes { .. }
                ) && event.fragment.is_some_and(|f| !f.final_part));
            let ack = self.ingest.append(event).await?;
            if !hold {
                checkpoint.ack(ack.generation, &ack.physical);
            }
        }
        Ok(())
    }
    pub(super) async fn marker(&self, message: &str, checkpoint: &mut Checkpoints) -> Result<()> {
        let sequence = checkpoint.allocate(Position::Marker)?;
        let mut event = normalize(
            message.as_bytes(),
            &self.source,
            self.generation,
            Stream::System,
            sequence,
            now_ms(),
        );
        event.quality.uncertain = true;
        self.admit(vec![event], checkpoint).await
    }
}
struct Decode {
    framer: Framer,
    assembler: Assembler,
    stream: Stream,
    docker_partial: Option<(String, u64, u64)>,
}
impl Decode {
    fn new(stream: Stream) -> Self {
        Self {
            framer: if stream == Stream::File {
                Framer::default()
            } else {
                Framer::bounded(64 * 1024)
            },
            assembler: if stream == Stream::File {
                Assembler::default()
            } else {
                Assembler::bounded(128 * 1024)
            },
            stream,
            docker_partial: None,
        }
    }
}
async fn docker(reader: &Reader, session: &Session, id: &str, cp: &mut Checkpoints) -> Result<()> {
    let capability = super::inspect(
        session,
        RemoteOperation::DockerCapability(serde_json::json!({"id":id})),
        &reader.cancel,
    )
    .await?;
    if capability["kind"] != "capability" {
        return Err(Error::NeedsAction(
            "Docker inspect unavailable/denied; check Docker CLI and permissions".into(),
        ));
    }
    if capability["readable"] == false {
        return Err(Error::NeedsAction(
            "Unsupported Docker logging driver: none".into(),
        ));
    }
    reader
        .marker(
            if capability["tty"] == true {
                "Docker TTY merges streams; history/cache coverage unknown"
            } else {
                "Docker stdout/stderr selected; driver cache/history coverage unknown"
            },
            cp,
        )
        .await?;
    let since = match &cp.position {
        Some(Position::Docker(t)) => Some(conservative_since(t)?),
        _ => None,
    };
    let mut channel = session
        .spawn(RemoteOperation::Docker {
            id: id.into(),
            since,
        })
        .await?;
    let mut stdout = channel.child.stdout.take().unwrap();
    let mut stderr = channel.child.stderr.take().unwrap();
    let mut a = Decode::new(Stream::Stdout);
    let mut b = Decode::new(Stream::Stderr);
    let mut first = [0; 16 * 1024];
    let mut second = [0; 16 * 1024];
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    let mut out_open = true;
    let mut err_open = true;
    let result = async {
        loop {
            tokio::select! {
                _ = reader.cancel.cancelled() => break,
                _ = session.cancel.cancelled() => return Err(Error::Cancelled),
                _ = tick.tick() => {
                    reader.admit(a.assembler.flush_due(Instant::now()), cp).await?;
                    reader.admit(b.assembler.flush_due(Instant::now()), cp).await?;
                }
                n = stdout.read(&mut first), if out_open => {
                    let n = n?;
                    if cp.pending_len()>=192 { reader.admit(a.assembler.flush(Instant::now()),cp).await?;reader.admit(b.assembler.flush(Instant::now()),cp).await?; }
                    if n == 0 { out_open = false; } else { docker_chunk(reader, &mut a, &first[..n], cp).await?; }
                }
                n = stderr.read(&mut second), if err_open => {
                    let n = n?;
                    if cp.pending_len()>=192 { reader.admit(a.assembler.flush(Instant::now()),cp).await?;reader.admit(b.assembler.flush(Instant::now()),cp).await?; }
                    if n == 0 { err_open = false; } else { docker_chunk(reader, &mut b, &second[..n], cp).await?; }
                }
            }
            if !out_open && !err_open { break; }
        }
        Ok(())
    }.await;
    // Retain the observed suffix, but never advance Docker's complete-record cursor.
    if let Some(frame) = a.framer.interrupt() {
        let _ = docker_frame(reader, &mut a, frame, cp).await;
    }
    if let Some(frame) = b.framer.interrupt() {
        let _ = docker_frame(reader, &mut b, frame, cp).await;
    }
    reader.admit(a.assembler.flush(Instant::now()), cp).await?;
    reader.admit(b.assembler.flush(Instant::now()), cp).await?;
    let exited = channel.child.try_wait()?;
    channel.close().await;
    result?;
    if exited.is_some_and(|status| !status.success()) && !reader.cancel.is_cancelled() {
        Err(Error::NeedsAction(
            "Docker logs ended; verify container/permissions".into(),
        ))
    } else {
        Ok(())
    }
}
async fn docker_chunk(
    reader: &Reader,
    decoder: &mut Decode,
    bytes: &[u8],
    cp: &mut Checkpoints,
) -> Result<()> {
    for chunk in bytes.chunks(512) {
        let mut frames = Vec::new();
        decoder
            .framer
            .push(chunk, |f| frames.push(f))
            .map_err(|e| Error::NeedsAction(e.into()))?;
        for frame in frames {
            docker_frame(reader, decoder, frame, cp).await?;
        }
    }
    Ok(())
}
async fn docker_frame(
    reader: &Reader,
    decoder: &mut Decode,
    frame: Frame,
    cp: &mut Checkpoints,
) -> Result<()> {
    let continuation = frame.fragment.is_some_and(|f| f.start > 0);
    let (timestamp, physical, payload) = if continuation {
        let (timestamp, physical, _) = decoder
            .docker_partial
            .as_ref()
            .ok_or_else(|| Error::Invalid("orphan Docker fragment".into()))?;
        (timestamp.clone(), *physical, frame.bytes.as_slice())
    } else {
        let split = frame
            .bytes
            .iter()
            .position(|b| *b == b' ')
            .ok_or_else(|| Error::NeedsAction("Docker timestamps unavailable".into()))?;
        let timestamp = std::str::from_utf8(&frame.bytes[..split])
            .map_err(|_| Error::Invalid("Docker timestamp".into()))?;
        if timestamp.len() > 40
            || time::OffsetDateTime::parse(
                timestamp,
                &time::format_description::well_known::Rfc3339,
            )
            .is_err()
        {
            return Err(Error::NeedsAction(
                "Docker permission/runtime error or invalid timestamp".into(),
            ));
        }
        if !matches!(reader.spec, SourceSpec::Kubernetes { .. }) {
            reader.state(
                "streaming",
                Some("Docker driver/cache/history coverage unknown; timestamp replay only".into()),
            );
        }
        if cp.pending_len() >= 192 {
            reader
                .admit(decoder.assembler.flush(Instant::now()), cp)
                .await?;
        }
        let physical = cp.allocate(Position::Docker(timestamp.into()))?;
        if frame.fragment.is_some() {
            decoder.docker_partial = Some((timestamp.into(), physical, (split + 1) as u64));
        }
        (timestamp.to_owned(), physical, &frame.bytes[split + 1..])
    };
    let mut event = if frame.fragment.is_some() || frame.ending == RecordEnding::Interrupted {
        reader
            .ingest
            .raw(payload.to_vec(), decoder.stream, physical)
    } else {
        reader
            .ingest
            .normalize(payload.to_vec(), decoder.stream, physical, &reader.cancel)
            .await?
    };
    event.ending = frame.ending;
    event.fragment = frame.fragment.map(|mut f| {
        let header = decoder.docker_partial.as_ref().map_or(0, |p| p.2);
        f.record = physical;
        f.start = f.start.saturating_sub(header);
        f.end = f.end.saturating_sub(header);
        f
    });
    event.quality.fragmented = event.fragment.is_some();
    event.quality.incomplete = frame.ending == RecordEnding::Interrupted;
    event.checkpoint_complete = !event.quality.incomplete;
    event.transport_time = Some(timestamp.clone());
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    frame.bytes.hash(&mut hash);
    decoder.stream.hash(&mut hash);
    event.quality.replay_suspected = cp.replay_suspected(&timestamp, hash.finish());
    if frame.fragment.is_some_and(|f| f.final_part) {
        decoder.docker_partial = None;
    }
    let ready = decoder.assembler.push(event, Instant::now());
    reader.admit(ready, cp).await?;
    Ok(())
}

async fn file(reader: &Reader, session: &Session, path: &str, cp: &mut Checkpoints) -> Result<()> {
    let checkpoint = match &cp.position {
        Some(Position::File(p)) => Some(p),
        _ => None,
    };
    let mut channel = session
        .spawn(RemoteOperation::File(
            serde_json::json!({"path":path,"checkpoint":checkpoint}),
        ))
        .await?;
    let stdout = channel.child.stdout.take().unwrap();
    let stderr = channel.child.stderr.take().unwrap();
    let diagnostic = tokio::spawn(crate::adapters::ssh::bounded_read(stderr, 4096));
    let mut output = BufReader::with_capacity(64 * 1024, stdout);
    let mut decoder = Decode::new(Stream::File);
    let mut device = String::new();
    let mut inode = String::new();
    let mut epoch = 0_u64;
    let opened_deadline = Instant::now() + Duration::from_secs(1);
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    let result = async {
        loop {
            let mut header = Vec::new();
            // Enforce header limit before JSON decoding/allocation.
            loop {
                let mut limited = tokio::io::AsyncReadExt::take(&mut output, 4097-header.len() as u64);
                tokio::select! {
                    _ = reader.cancel.cancelled() => break,
                    _ = session.cancel.cancelled() => return Err(Error::Cancelled),
                    _ = tick.tick()=>{if device.is_empty() && Instant::now()>opened_deadline {return Err(Error::Timeout);}reader.admit(decoder.assembler.flush_due(Instant::now()),cp).await?;},
                    result = limited.read_until(b'\n', &mut header) => {
                        let n=result?;
                        if n==0 {return Err(Error::NeedsAction("file helper ended; Python 3, regular-file access and /proc are required".into()));}
                        break;
                    }
                }
            }
            if reader.cancel.is_cancelled() { break; }
            if header.len() > 4096 || header.last() != Some(&b'\n') { return Err(Error::Limit("file helper header")); }
            let meta: Value = serde_json::from_slice(&header)?;
            let length = meta["length"].as_u64().ok_or_else(|| Error::Invalid("file frame length".into()))? as usize;
            if length > 64*1024 { return Err(Error::Limit("file chunk")); }
            match meta["kind"].as_str().unwrap_or("") {
                "opened" => {
                    reader.state("streaming",None);
                    epoch=string(&meta,"generation")?.parse().map_err(|_|Error::Invalid("file generation".into()))?;
                    device = string(&meta,"device")?; inode = string(&meta,"inode")?;
                    reader.info.lock().unwrap().generation = format!("{}:{epoch}:{device}:{inode}",reader.generation);
                    let offset = string(&meta,"offset")?.parse::<u64>().map_err(|_| Error::Invalid("file offset".into()))?;
                    decoder.framer = if let Some(partial) = meta.get("partial").filter(|v| !v.is_null()) {
                        Framer::resume(offset, string(partial,"record")?.parse().map_err(|_|Error::Invalid("partial record".into()))?, string(partial,"next_payload")?.parse().map_err(|_|Error::Invalid("partial offset".into()))?)
                    } else { Framer::at(offset) };
                    let guard = unhex(&string(&meta,"guard")?)?; decoder.framer.seed_guard(&guard);
                }
                "data" => {
                    if device.is_empty() { return Err(Error::Invalid("data before descriptor handshake".into())); }
                    let mut chunk = vec![0; length];
                    tokio::select! { _ = reader.cancel.cancelled() => break, _ = session.cancel.cancelled() => return Err(Error::Cancelled), r = output.read_exact(&mut chunk) => { r?; } }
                    for part in chunk.chunks(512) {
                    let mut frames = vec![]; decoder.framer.push(part, |f| frames.push(f)).map_err(|e| Error::NeedsAction(e.into()))?;
                    for frame in frames { file_frame(reader, &mut decoder, frame, cp, &device, &inode, epoch).await?; }
                    }
                }
                "rotated" | "truncated" => {
                    // Finalize the old descriptor's partial bytes before crossing the generation fence.
                    if let Some(frame) = decoder.framer.finish() { file_frame(reader, &mut decoder, frame, cp, &device, &inode, epoch).await?; }
                    reader.admit(decoder.assembler.flush(Instant::now()),cp).await?;
                    decoder.assembler = Assembler::default(); epoch = epoch.checked_add(1).ok_or(Error::Limit("file generation overflow"))?;
                    reader.info.lock().unwrap().generation = format!("{}:{epoch}:{device}:{inode}",reader.generation);
                    reader.marker(if meta["kind"] == "rotated" {"Log rotated"} else {"Log truncated"},cp).await?;
                }
                "recent_window" => reader.marker("Initial recent window; earlier bytes not loaded",cp).await?,
                "resume" => {},
                "idle" => reader.admit(decoder.assembler.flush_due(Instant::now()),cp).await?,
                _ => return Err(Error::Invalid("unknown helper frame".into()))
            }
        }
        Ok(())
    }.await;
    if !device.is_empty()
        && let Some(frame) = decoder.framer.interrupt()
    {
        let _ = file_frame(reader, &mut decoder, frame, cp, &device, &inode, epoch).await;
    }
    let flushed = reader
        .admit(decoder.assembler.flush(Instant::now()), cp)
        .await;
    channel.close().await;
    let _ = diagnostic.await;
    result.and(flushed)
}
#[allow(clippy::too_many_arguments)]
async fn file_frame(
    reader: &Reader,
    decoder: &mut Decode,
    frame: Frame,
    cp: &mut Checkpoints,
    device: &str,
    inode: &str,
    epoch: u64,
) -> Result<()> {
    let physical = cp.allocate(Position::File(FileCheckpoint {
        generation: epoch.to_string(),
        device: device.into(),
        inode: inode.into(),
        next_offset: frame.end_offset.to_string(),
        guard: hex(&frame.guard),
        partial: frame
            .fragment
            .filter(|_| {
                frame.ending == RecordEnding::Fragment || frame.ending == RecordEnding::Interrupted
            })
            .map(|f| FilePartial {
                record: f.record.to_string(),
                next_payload: f.end.to_string(),
            })
            .or_else(|| {
                (frame.ending == RecordEnding::Interrupted).then(|| FilePartial {
                    record: (frame.end_offset - frame.bytes.len() as u64).to_string(),
                    next_payload: frame.bytes.len().to_string(),
                })
            }),
    }))?;
    let mut event = if frame.fragment.is_some() || frame.ending == RecordEnding::Interrupted {
        reader.ingest.raw(frame.bytes, Stream::File, physical)
    } else {
        reader
            .ingest
            .normalize(frame.bytes, Stream::File, physical, &reader.cancel)
            .await?
    };
    event.ending = frame.ending;
    event.fragment = frame.fragment;
    event.quality.fragmented = event.fragment.is_some();
    event.quality.incomplete = frame.ending == RecordEnding::Interrupted;
    event.checkpoint_complete = !event.quality.incomplete;
    event.generation = format!("{}:{epoch}:{device}:{inode}", reader.generation);
    event.quality.truncated = frame.truncated;
    reader
        .admit(decoder.assembler.push(event, Instant::now()), cp)
        .await
}
fn string(value: &Value, key: &str) -> Result<String> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::Invalid(format!("file metadata {key}")))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Result<Vec<u8>> {
    if s.len() > 128 || !s.len().is_multiple_of(2) {
        return Err(Error::Invalid("file guard".into()));
    }
    s.as_bytes()
        .chunks_exact(2)
        .map(|b| {
            std::str::from_utf8(b)
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
                .ok_or_else(|| Error::Invalid("file guard".into()))
        })
        .collect()
}

fn conservative_since(timestamp: &str) -> Result<String> {
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
    let timestamp = OffsetDateTime::parse(timestamp, &Rfc3339)
        .map_err(|_| Error::Invalid("Docker RFC3339 timestamp".into()))?;
    (timestamp - Duration::nanoseconds(1))
        .format(&Rfc3339)
        .map_err(|_| Error::Invalid("Docker timestamp boundary".into()))
}

async fn kubernetes(reader: &Reader, session: &Session, cp: &mut Checkpoints) -> Result<()> {
    let mut args = serde_json::to_value(&reader.spec)?;
    args["action"] = serde_json::json!("follow");
    if let Some(Position::Docker(t)) = &cp.position {
        args["since"] = serde_json::json!(conservative_since(t)?);
    }
    let mut channel = session.spawn(RemoteOperation::Kubernetes(args)).await?;
    let diagnostic = tokio::spawn(crate::adapters::ssh::bounded_read(
        channel.child.stderr.take().unwrap(),
        4096,
    ));
    let mut output = BufReader::with_capacity(16 * 1024, channel.child.stdout.take().unwrap());
    let mut decoder = Decode::new(Stream::Stdout);
    let mut line = Vec::new();
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut opened = false;
    let result=async {
        loop {
            let mut limited=(&mut output).take((4097-line.len()) as u64);
            tokio::select! {
                _=reader.cancel.cancelled()=>break,
                _=session.cancel.cancelled()=>return Err(Error::Cancelled),
                _=tick.tick()=>{if !opened&&Instant::now()>deadline{return Err(Error::Timeout);}reader.admit(decoder.assembler.flush_due(Instant::now()),cp).await?;continue;},
                n=limited.read_until(b'\n',&mut line)=>{if n?==0{return Err(Error::NeedsAction("Kubernetes helper ended; continuity unknown".into()));}}
            }
            if line.len()>4096||line.last()!=Some(&b'\n'){return Err(Error::Limit("Kubernetes header"));}
            let meta:Value=serde_json::from_slice(&line)?;line.clear();
            match meta["kind"].as_str().unwrap_or("") {
                "opened"=>{opened=true;reader.state("streaming_kubernetes_partial",Some("Selected Pod/container instance; merged API stream; latest-file history/permissions/rotation and API identity race remain uncertain".into()));},
                "data"=>{
                    let n=meta["length"].as_u64().filter(|n|*n<=16*1024).ok_or(Error::Limit("Kubernetes chunk"))? as usize;
                    let mut bytes=vec![0;n];
                    tokio::select!{_=reader.cancel.cancelled()=>break,_=session.cancel.cancelled()=>return Err(Error::Cancelled),r=output.read_exact(&mut bytes)=>{r?;}}
                    docker_chunk(reader,&mut decoder,&bytes,cp).await?;
                },
                "ended"=>{if meta["previous"]==true{return Err(Error::NeedsAction("Previous Kubernetes instance read completed; bounded retained window only".into()));}return Err(Error::Io(std::io::Error::new(std::io::ErrorKind::UnexpectedEof,"Kubernetes stream ended")));},
                "unsupported"=>return Err(Error::NeedsAction("Kubernetes unsupported: remote kubectl, explicit kubeconfig and Linux memfd required".into())),
                "needs_action"=>{reader.marker("Kubernetes identity/configuration/access changed; gap or unknown coverage; inspect selected instance again",cp).await?;return Err(Error::NeedsAction(format!("Kubernetes: {}",meta["reason"].as_str().unwrap_or("unknown"))));},
                _=>return Err(Error::Invalid("Kubernetes helper protocol".into()))
            }
        }
        Ok(())
    }.await;
    if let Some(frame) = decoder.framer.interrupt() {
        let _ = docker_frame(reader, &mut decoder, frame, cp).await;
    }
    let flushed = reader
        .admit(decoder.assembler.flush(Instant::now()), cp)
        .await;
    channel.close().await;
    diagnostic.abort();
    result.and(flushed)
}
