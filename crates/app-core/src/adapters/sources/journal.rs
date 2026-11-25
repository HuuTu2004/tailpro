use super::{Checkpoints, Position, RemoteOperation, reader::Reader};
use crate::{
    adapters::ssh::Session,
    api::{Error, Result},
};
use log_core::{
    model::{Fragment, Level, Stream},
    parse::Assembler,
};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

pub(super) async fn read(
    reader: &Reader,
    session: &Session,
    unit: &str,
    cp: &mut Checkpoints,
) -> Result<()> {
    let cursor = match &cp.position {
        Some(Position::Journal(c)) => Some(c),
        _ => None,
    };
    let mut channel = session
        .spawn(RemoteOperation::Journal(
            serde_json::json!({"unit":unit,"cursor":cursor}),
        ))
        .await?;
    let stderr = channel.child.stderr.take().unwrap();
    let diagnostic = tokio::spawn(crate::adapters::ssh::bounded_read(stderr, 4096));
    let mut output = BufReader::with_capacity(16 * 1024, channel.child.stdout.take().unwrap());
    let mut assembler = Assembler::bounded(128 * 1024);
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    let mut line = Vec::new();
    let mut opened = false;
    let mut parent: Option<(String, u64)> = None;
    let deadline = Instant::now() + Duration::from_secs(4);
    let result = async {
        loop {
            let mut limited = (&mut output).take((4096 + 1 - line.len()) as u64);
            tokio::select! {
                _ = reader.cancel.cancelled() => break,
                _ = session.cancel.cancelled() => return Err(Error::Cancelled),
                _ = tick.tick() => {
                    if !opened && Instant::now() > deadline { return Err(Error::Timeout); }
                    reader.admit(assembler.flush_due(Instant::now()), cp).await?;
                    continue;
                },
                n = limited.read_until(b'\n', &mut line) => {
                    if n? == 0 { return Err(Error::NeedsAction("Journal helper ended; verify Python 3, journalctl and journal access".into())); }
                }
            }
            if line.len() > 4096 || line.last() != Some(&b'\n') { return Err(Error::Limit("Journal framing")); }
            let v: Value = serde_json::from_slice(&line)?;
            line.clear();
            match v["kind"].as_str().unwrap_or("") {
                "opened" => { opened = true; reader.state("streaming_visible_journal", Some("Visible journal entries only; permissions/history coverage unknown; cursor is RAM-only".into())); }
                "access" => {
                    reader.marker("Journal permissions restricted or unavailable; visible entries may be partial", cp).await?;
                    reader.state("journal_access_restricted", Some("Check journal permissions; empty output does not prove an empty unit".into()));
                }
                "gap" => { reader.admit(assembler.flush(Instant::now()), cp).await?; reader.marker("Journal cursor invalid/vacuumed; bounded recent fallback; gap", cp).await?; }
                "unsupported" => return Err(Error::NeedsAction("Unsupported host: journalctl unavailable".into())),
                "unavailable" => return Err(Error::NeedsAction("Journal unavailable; verify unit/journal permissions".into())),
                "message_part" => {
                    if cp.pending_len() >= 192 { reader.admit(assembler.flush(Instant::now()), cp).await?; }
                    let cursor = v["cursor"].as_str().filter(|s| !s.is_empty() && s.len() <= 2048).ok_or_else(|| Error::Invalid("Journal cursor".into()))?;
                    let start = v["start"].as_str().and_then(|s|s.parse::<u64>().ok()).ok_or_else(||Error::Invalid("Journal part offset".into()))?;
                    let length = v["length"].as_u64().filter(|n|*n<=64*1024).ok_or(Error::Limit("Journal payload part"))? as usize;
                    let index = v["index"].as_u64().filter(|n|*n<32).ok_or(Error::Limit("Journal repeated values"))?;
                    let physical = if start==0&&index==0 {
                        if parent.is_some() {return Err(Error::Invalid("incomplete Journal entry".into()));}
                        let physical=cp.allocate(Position::Journal(cursor.into()))?;parent=Some((cursor.into(),physical));physical
                    } else { parent.as_ref().filter(|(c,_)|c==cursor).ok_or_else(||Error::Invalid("orphan Journal part".into()))?.1 };
                    let mut bytes=vec![0;length];
                    tokio::select! {_=reader.cancel.cancelled()=>return Err(Error::Cancelled),_=session.cancel.cancelled()=>return Err(Error::Cancelled),r=output.read_exact(&mut bytes)=>{r?;}}
                    let fragmented=v["fragmented"].as_bool().unwrap_or(true);
                    let mut e=if fragmented {reader.ingest.raw(bytes,Stream::Journal,physical)} else {reader.ingest.normalize(bytes,Stream::Journal,physical,&reader.cancel).await?};
                    e.checkpoint_complete=v["complete"].as_bool().unwrap_or(false);
                    if fragmented {e.fragment=Some(Fragment{record:physical.checked_mul(32).and_then(|n|n.checked_add(index)).ok_or(Error::Limit("Journal identity overflow"))?,start,end:start.checked_add(length as u64).ok_or(Error::Limit("Journal part overflow"))?,final_part:v["final"].as_bool().unwrap_or(false)});e.quality.fragmented=true;}
                    if e.level == Level::Unknown {
                        e.evidence.severity_namespace=log_core::model::ingestion::SeverityNamespace::Syslog;
                        e.evidence.severity_origin=log_core::model::ingestion::FieldOrigin::Explicit;
                        e.evidence.severity_literal=v["priority"].as_str().map(str::to_owned);
                        e.level = match v["priority"].as_str() { Some("0" | "1" | "2" | "3") => Level::Error, Some("4") => Level::Warn, Some("5" | "6") => Level::Info, Some("7") => Level::Debug, _ => Level::Unknown };
                    }
                    if let Some(us) = v["time_us"].as_str().and_then(|s| s.parse::<i128>().ok())
                        && let Some(ns) = us.checked_mul(1000)
                        && let Ok(t) = time::OffsetDateTime::from_unix_timestamp_nanos(ns) {
                        e.transport_time = t.format(&time::format_description::well_known::Rfc3339).ok();
                    }
                    let complete=e.checkpoint_complete;
                    reader.admit(assembler.push(e, Instant::now()), cp).await?;
                    if complete {parent=None;}
                }
                _ => return Err(Error::Invalid("Journal protocol".into())),
            }
        }
        Ok(())
    }.await;
    let flushed = reader.admit(assembler.flush(Instant::now()), cp).await;
    channel.close().await;
    diagnostic.abort();
    result.and(flushed)
}
