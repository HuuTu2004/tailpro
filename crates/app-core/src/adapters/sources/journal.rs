use super::{Checkpoints, Position, RemoteOperation, reader::Reader};
use crate::{
    adapters::ssh::Session,
    api::{Error, Result},
};
use log_core::{
    model::{Level, Stream},
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
    let deadline = Instant::now() + Duration::from_secs(4);
    let result = async {
        loop {
            let mut limited = (&mut output).take((256 * 1024 + 1 - line.len()) as u64);
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
            if line.len() > 256 * 1024 || line.last() != Some(&b'\n') { return Err(Error::Limit("Journal framing")); }
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
                "record" => {
                    if cp.pending_len() >= 192 { reader.admit(assembler.flush(Instant::now()), cp).await?; }
                    let cursor = v["cursor"].as_str().filter(|s| !s.is_empty() && s.len() <= 2048).ok_or_else(|| Error::Invalid("Journal cursor".into()))?;
                    let message = v["message"].as_str().ok_or_else(|| Error::Invalid("Journal message".into()))?;
                    let physical = cp.allocate(Position::Journal(cursor.into()))?;
                    let mut e = reader.ingest.normalize(message.as_bytes().to_vec(), Stream::Journal, physical, &reader.cancel).await?;
                    if e.level == Level::Unknown {
                        e.level = match v["priority"].as_str() { Some("0" | "1" | "2" | "3") => Level::Error, Some("4") => Level::Warn, Some("5" | "6") => Level::Info, Some("7") => Level::Debug, _ => Level::Unknown };
                    }
                    e.quality.uncertain |= v["uncertain"].as_bool().unwrap_or(true);
                    if let Some(us) = v["time_us"].as_str().and_then(|s| s.parse::<i128>().ok())
                        && let Some(ns) = us.checked_mul(1000)
                        && let Ok(t) = time::OffsetDateTime::from_unix_timestamp_nanos(ns) {
                        e.event_time = t.format(&time::format_description::well_known::Rfc3339).ok();
                    }
                    reader.admit(assembler.push(e, Instant::now()), cp).await?;
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
