use super::{output::*, *};
use log_core::model::Event;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

pub(super) const TOOLS: &[&str] = &[
    "list_sources",
    "get_recent_logs",
    "search_logs",
    "get_log_context",
    "locate_service",
    "get_error_clusters",
    "trace_id",
    "inspect_context",
];
pub(super) fn advanced(tool: &str) -> bool {
    matches!(
        tool,
        "locate_service" | "get_error_clusters" | "trace_id" | "inspect_context"
    )
}
pub(super) fn window(request: &ReadRequest) -> Result<(Option<u64>, Option<u64>)> {
    let parse = |s: &Option<String>| {
        s.as_ref()
            .map(|s| {
                s.parse::<u64>()
                    .map_err(|_| Error::Invalid("observed time window".into()))
            })
            .transpose()
    };
    let from = parse(&request.from_ms)?;
    let to = parse(&request.to_ms)?;
    if from.zip(to).is_some_and(|(a, b)| a > b) {
        return Err(Error::Invalid("observed time window".into()));
    }
    Ok((from, to))
}
pub(super) fn signature(p: &Event) -> Option<(String, String)> {
    if p.raw.len() > 4096 || p.message.contains(redaction::MASK) {
        return None;
    }
    let mut value = serde_json::from_str::<serde_json::Value>(&p.raw).ok();
    if let Some(object) = value.as_mut().and_then(|v| v.as_object_mut()) {
        if ["stack", "exception", "traceback", "error"]
            .iter()
            .any(|key| {
                object
                    .get(*key)
                    .is_some_and(|v| v.to_string().contains(redaction::MASK))
            })
        {
            return None;
        }
        for key in [
            "timestamp",
            "time",
            "observed_at_ms",
            "trace_id",
            "order_id",
            "request_id",
            "duration_ms",
        ] {
            object.remove(key);
        }
    }
    // Keep the entire remaining representation, including every stack frame.
    // A digest is only a bucket key: callers compare representations on collisions.
    let representation = value.map_or_else(|| p.raw.clone(), |v| v.to_string());
    let representation = format!("{:?}:{}", p.level, representation);
    Some((
        format!("{:x}", Sha256::digest(representation.as_bytes())),
        representation,
    ))
}
static STACK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:^|[\s("])([A-Za-z0-9_./\\:$%~+-]+?):(\d{1,9})(?::\d+)?"#).unwrap()
});
pub(super) fn hints(text: &str, evidence: &str, mapping: Option<&ServiceMapping>) -> Vec<CodeHint> {
    let Some(mapping) = mapping else {
        return vec![];
    };
    static PYTHON: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"File\s+["']([^"']+)["'],\s+line\s+(\d{1,9})"#).unwrap());
    let decoded = serde_json::from_str::<serde_json::Value>(text).ok();
    let mut strings = vec![];
    let mut pending = decoded.as_ref().into_iter().collect::<Vec<_>>();
    let mut visited = 0;
    while let Some(v) = pending.pop() {
        visited += 1;
        if visited > 4096 {
            break;
        }
        match v {
            serde_json::Value::String(s) => strings.push(s.as_str()),
            serde_json::Value::Object(o) => pending.extend(o.values()),
            serde_json::Value::Array(a) => pending.extend(a),
            _ => {}
        }
    }
    if decoded.is_none() {
        strings.push(text);
    }
    let mut result: Vec<CodeHint> = vec![];
    let mut candidates = 0;
    for text in strings {
        for c in STACK.captures_iter(text).chain(PYTHON.captures_iter(text)) {
            candidates += 1;
            if candidates > 64 || result.len() == 32 {
                return result;
            }
            let path = &c[1];
            if path.len() > 1024
                || path.starts_with(['/', '~', '-'])
                || path.contains(['\\', ':', '$', '%'])
                || path
                    .split('/')
                    .any(|p| p.is_empty() || p == ".." || p == ".")
                || !path
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_./+-".contains(&b))
                || !path.contains('.')
            {
                continue;
            }
            let Ok(line) = c[2].parse::<u32>() else {
                continue;
            };
            if line == 0
                || result
                    .iter()
                    .any(|h| h.relative_path == path && h.line == line)
            {
                continue;
            }
            result.push(CodeHint {repo_label:redaction::sanitize(&mapping.repo_label).text, relative_path:path.into(),line,evidence_ref:evidence.into(),confidence:"developer mapping; checkout/revision unverified".into(),declared_revision:mapping.revision.as_ref().map(|r|redaction::sanitize(r).text),provenance:"sanitized stack evidence + developer-declared mapping; no repository read; deployed code unverified (including revision mismatches)".into()});
        }
    }
    result
}
impl AppHandle {
    pub fn service_mappings(&self) -> Result<Vec<ServiceMapping>> {
        Ok(self.settings.lock().unwrap().load()?.service_mappings)
    }
    pub fn save_service_mapping(&self, mapping: ServiceMapping) -> Result<()> {
        self.scoped_hosts(std::slice::from_ref(&mapping.source))?;
        if mapping.aliases.len() > 16
            || mapping.repo_label.is_empty()
            || mapping.service.is_empty()
            || [
                &mapping.source,
                &mapping.service,
                &mapping.project,
                &mapping.repo_label,
            ]
            .into_iter()
            .chain(mapping.aliases.iter())
            .any(|s| s.len() > 128 || s.chars().any(char::is_control))
            || mapping
                .revision
                .as_ref()
                .is_some_and(|s| s.len() > 128 || s.chars().any(char::is_control))
        {
            return Err(Error::Invalid("service mapping".into()));
        }
        policy::path(&mapping.repo_root)?; // Stored declaration only. Never opened.
        let store = self.settings.lock().unwrap();
        let mut saved = store.load()?;
        saved
            .service_mappings
            .retain(|m| m.source != mapping.source);
        saved.service_mappings.push(mapping);
        store.save(&saved)?;
        drop(store);
        // Cancel all outstanding contexts when their mapping provenance changes.
        self.clear_outputs();
        Ok(())
    }
    pub fn delete_service_mapping(&self, source: &str) -> Result<()> {
        let store = self.settings.lock().unwrap();
        let mut saved = store.load()?;
        saved.service_mappings.retain(|m| m.source != source);
        store.save(&saved)?;
        drop(store);
        self.clear_outputs();
        Ok(())
    }
    // Trusted bundled UI uses the identical policy/query path with an ephemeral,
    // unexposed principal. Its owned result is capped; no extra cache or scheduler.
    pub async fn debug_read(
        &self,
        view_id: &str,
        generation: u32,
        tool: &str,
        request: ReadRequest,
    ) -> Result<ReadResponse> {
        if !advanced(tool) || request.sources.is_empty() {
            return Err(denied());
        }
        self.scoped_hosts(&request.sources)?;
        let cancel = self.views.begin(view_id, generation, false)?;
        let grant = opaque();
        let token = opaque();
        self.output.state.lock().unwrap().grants.insert(
            grant.clone(),
            Grant {
                desktop: true,
                info: GrantInfo {
                    id: grant.clone(),
                    client: "bundled desktop".into(),
                    sources: request.sources.clone(),
                    expires_at_ms: (now_ms() + 60_000).to_string(),
                },
                token: token.clone(),
                until: Instant::now() + Duration::from_secs(60),
                cancel,
                calls: Default::default(),
                active: 0,
            },
        );
        struct Cleanup(AppHandle, String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let mut s = self.0.output.state.lock().unwrap();
                if let Some(g) = s.grants.remove(&self.1) {
                    g.cancel.cancel();
                }
                s.refs.retain(|r| r.grant != self.1);
                s.cursors.retain(|_, c| c.grant != self.1);
            }
        }
        let _cleanup = Cleanup(self.clone(), grant.clone());
        self.agent_read(&grant, &token, tool, request).await
    }
}
