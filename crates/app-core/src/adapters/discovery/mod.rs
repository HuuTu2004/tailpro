use crate::{
    adapters::{
        sources::RemoteOperation,
        ssh::{Session, bounded_read},
    },
    api::{Candidate, DiscoveryRequest, DiscoveryResult, Error, Result},
    application::policy,
};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio_util::sync::CancellationToken;

pub async fn discover(
    session: Arc<Session>,
    request: DiscoveryRequest,
    cancel: CancellationToken,
) -> Result<DiscoveryResult> {
    if request.host_id != session.profile.id {
        return Err(Error::Invalid("host mismatch".into()));
    }
    if request.project_roots.len() > 16
        || request.selected_pids.len() > 32
        || request.selected_units.len() > 32
        || request.config_files.len() > 32
        || request.config_roots.len() > 32
    {
        return Err(Error::Limit("discovery scope"));
    }
    for p in request
        .project_roots
        .iter()
        .chain(request.config_files.iter())
        .chain(request.config_roots.iter())
        .chain(request.pm2_root.iter())
    {
        policy::path(p)?;
    }
    for unit in &request.selected_units {
        if unit.len() > 255
            || !unit
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"@._-".contains(&b))
            || unit.starts_with('-')
        {
            return Err(Error::Invalid("unit name".into()));
        }
    }
    if !request.config_files.is_empty()
        && (!request.config_consent || request.config_roots.is_empty())
    {
        return Err(Error::NeedsAction(
            "config files require consent and approved roots".into(),
        ));
    }
    probe(
        session,
        serde_json::to_value(request)?,
        cancel,
        Duration::from_secs(15),
        1024 * 1024,
    )
    .await
}
pub async fn complete(
    session: Arc<Session>,
    parent: String,
    prefix: String,
    cancel: CancellationToken,
) -> Result<DiscoveryResult> {
    policy::path(&parent)?;
    if prefix.len() > 255 || prefix.contains('/') || prefix.chars().any(char::is_control) {
        return Err(Error::Invalid("literal basename prefix".into()));
    }
    probe(
        session,
        serde_json::json!({"complete":true,"parent":parent,"prefix":prefix}),
        cancel,
        Duration::from_secs(1),
        64 * 1024,
    )
    .await
}
async fn probe(
    session: Arc<Session>,
    args: Value,
    cancel: CancellationToken,
    deadline: Duration,
    cap: usize,
) -> Result<DiscoveryResult> {
    let _permit = tokio::select! { _ = cancel.cancelled() => return Err(Error::Cancelled), p = session.probes.clone().acquire_owned() => p.map_err(|_| Error::Cancelled)? };
    let mut channel = session.spawn(RemoteOperation::Discovery(args)).await?;
    let stdout = channel.child.stdout.take().unwrap();
    let stderr = channel.child.stderr.take().unwrap();
    let diagnostic = tokio::spawn(bounded_read(stderr, 4096));
    let mut output = BufReader::new(stdout);
    let mut bytes = Vec::new();
    let mut partial = false;
    let until = tokio::time::Instant::now() + deadline;
    loop {
        let remaining = cap.saturating_sub(bytes.len());
        if remaining == 0 {
            partial = true;
            break;
        }
        let mut line = Vec::new();
        let mut limited = (&mut output).take(remaining.min(16 * 1024) as u64);
        let next = tokio::select! {
            _=cancel.cancelled()=>{ channel.close().await;diagnostic.abort();return Err(Error::Cancelled); },
            _=session.cancel.cancelled()=>{ channel.close().await;diagnostic.abort();return Err(Error::Cancelled); },
            _=tokio::time::sleep_until(until)=>{partial=true;break;},
            line=limited.read_until(b'\n',&mut line)=>line
        };
        match next {
            Ok(0) => break,
            Ok(_) => {
                if line.last() != Some(&b'\n') {
                    partial = true;
                    break;
                }
                bytes.extend(line);
            }
            Err(_) => {
                partial = true;
                break;
            }
        }
    }
    channel.close().await;
    let diagnostic = diagnostic
        .await
        .map_err(|_| Error::Cancelled)?
        .unwrap_or_default();
    let mut candidates = vec![];
    let mut reasons = vec![];
    let mut done = false;
    if partial {
        reasons.push("discovery deadline/output cap; partial metadata retained".into());
    }
    for line in bytes.split(|&b| b == b'\n').filter(|b| !b.is_empty()) {
        let v: Value = serde_json::from_slice(line)?;
        if let Some(path) = v["growth"].as_str() {
            if let Some(c) = candidates
                .iter_mut()
                .find(|c: &&mut Candidate| c.path == path)
            {
                c.activity = format!(
                    "Active (observed growth over {} ms)",
                    v["window_ms"].as_str().unwrap_or("unknown")
                );
            }
            continue;
        }
        if let Some(unit) = v["unit_hint"].as_str() {
            if candidates.len() < 500 {
                candidates.push(Candidate {
                    file_type: "systemd_unit".into(),
                    confidence: "metadata".into(),
                    process_hint: Some(unit.into()),
                    identity: None,
                    id: format!("candidate-{:x}", rand::random::<u64>()),
                    host_id: session.profile.id.clone(),
                    path: unit.into(),
                    provider: "systemd_unit".into(),
                    provenance: "running-unit metadata; Start explicitly reads visible Journal entries; access is rechecked"
                        .into(),
                    access: crate::api::Access::Ready,
                    size: None,
                    mtime: None,
                    observed_at_ms: crate::application::now_ms().to_string(),
                    tags: vec![],
                    container_id: None,
                    activity: "Unknown".into(),
                });
            }
            continue;
        }
        if v["done"].as_bool() == Some(true) {
            done = true;
            if let Some(r) = v["reasons"].as_array() {
                reasons.extend(r.iter().filter_map(Value::as_str).map(String::from));
            }
            continue;
        }
        if candidates.len() >= 500 {
            reasons.push("candidate cap".into());
            break;
        }
        let candidate: Candidate = serde_json::from_value(serde_json::json!({
            "file_type":v["file_type"],"confidence":v["confidence"],"process_hint":v.get("process_hint"),"identity":v.get("identity"),
            "id": format!("candidate-{:x}",rand::random::<u64>()), "host_id":session.profile.id,
            "path":v["path"],"provider":v["provider"],"provenance":v["provenance"],"access":v["access"],
            "size":v.get("size"),"mtime":v.get("mtime"),"observed_at_ms":crate::application::now_ms().to_string(),
            "tags":v.get("tags").unwrap_or(&serde_json::json!([])), "container_id":v.get("container_id"),"activity":v["activity"]
        }))?;
        candidates.push(candidate);
    }
    if !diagnostic.is_empty() {
        reasons.push("remote provider unavailable; Python 3 is required".into());
    }
    candidates.sort_by_key(|c| (c.provider == "systemd_unit", !c.path.ends_with(".log")));
    Ok(DiscoveryResult {
        complete: done && reasons.is_empty(),
        candidates,
        reasons,
    })
}
