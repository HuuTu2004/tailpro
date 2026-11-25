use super::AppHandle;
use crate::{
    api::{Error, Result},
    workers::parser::Parser,
};
use std::sync::Arc;
impl AppHandle {
    /// Validate without affecting the live source. The sample is neither persisted nor exported.
    pub async fn preview_parser(
        &self,
        source: &str,
        config: String,
        sample: String,
    ) -> Result<String> {
        if sample.len() > 64 * 1024 || config.len() > 64 * 1024 {
            return Err(Error::Limit("parser preview"));
        }
        if !self.state.lock().unwrap().sources.contains_key(source) {
            return Err(Error::Invalid("Unknown active source".into()));
        }
        let source = source.to_owned();
        let _permit = self
            .engine
            .cpu
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Limit("CPU jobs"))?;
        tokio::task::spawn_blocking(move||{
            let _permit=_permit;
            let parser=Parser::compile(&config,&source,0)?;
            let mut e=log_core::parse::normalize_with(sample.as_bytes(),&source,0,log_core::model::Stream::File,0,0,&parser.profile);
            let spans=parser.apply(&mut e);
            Ok(serde_json::json!({"matched_spans":spans,"decoder":e.evidence.decoder,"level":e.level,"time_ns":e.evidence.event_time_ns,"message":e.message,"conflict":e.evidence.conflict,"malformed":e.quality.malformed}).to_string())
        }).await.map_err(|_|Error::Cancelled)?
    }
    pub async fn apply_parser(&self, source: &str, config: String) -> Result<String> {
        if config.len() > 64 * 1024 {
            return Err(Error::Limit("parser config"));
        }
        let source = source.to_owned();
        let permit = self
            .engine
            .cpu
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Limit("CPU jobs"))?;
        let name = source.clone();
        let mut parser = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            Parser::compile(&config, &name, 0)
        })
        .await
        .map_err(|_| Error::Cancelled)??;
        let state = self.state.lock().unwrap();
        let target = state
            .sources
            .get(&source)
            .ok_or_else(|| Error::Invalid("Unknown active source".into()))?;
        let count: usize = state
            .sources
            .iter()
            .filter(|(id, _)| **id != source)
            .map(|(_, s)| s.parser.read().unwrap().rule_count())
            .sum();
        if count + parser.rule_count() > 16 {
            return Err(Error::Limit(
                "16 rules per workspace / 4 MiB compiled ceiling",
            ));
        }
        let mut current = target.parser.write().unwrap();
        parser.revision = current
            .revision
            .checked_add(1)
            .ok_or(Error::Limit("parser revision overflow"))?;
        let revision = parser.revision.to_string();
        *current = Arc::new(parser);
        Ok(revision)
    }
    /// Local diagnostics contains counters and source IDs, never sample payloads.
    pub async fn ingestion_diagnostics(&self) -> Result<String> {
        let snapshot = self.engine.snapshot().await?;
        let mut metrics = serde_json::to_value(&snapshot.source_metrics)?;
        if let Some(sources) = metrics.as_object_mut() {
            for source in sources.values_mut() {
                if let Some(fields) = source.as_object_mut() {
                    for value in fields.values_mut() {
                        if value.is_number() {
                            *value = serde_json::Value::String(value.to_string());
                        }
                    }
                }
            }
        }
        let losses: std::collections::BTreeMap<_, _> = snapshot
            .source_losses
            .iter()
            .map(|(id, revision)| (id, revision.to_string()))
            .collect();
        Ok(serde_json::json!({"schema_version":1,"mode":"RAM; no durable ACK", "source_metrics":metrics,"source_loss_revisions":losses,"coverage":"Observed reader bytes and retained window only; upstream history/permissions may be unknown"}).to_string())
    }
    pub async fn inspect_kubernetes(
        &self,
        host: &str,
        kubeconfig: String,
        context: String,
        namespace: String,
        pod: String,
        container: String,
    ) -> Result<crate::api::KubernetesCapability> {
        super::policy::path(&kubeconfig)?;
        for v in [&context, &namespace, &pod, &container] {
            if v.is_empty()
                || v.len() > 253
                || v.starts_with('-')
                || v.chars().any(|c| c.is_whitespace() || c.is_control())
            {
                return Err(Error::Invalid("Invalid selected Kubernetes scope".into()));
            }
        }
        let (session, cancel) = self.session(host, true)?;
        let value=crate::adapters::sources::inspect(&session,crate::adapters::sources::RemoteOperation::Kubernetes(serde_json::json!({"action":"probe","kubeconfig":kubeconfig,"context":context,"namespace":namespace,"pod":pod,"container":container})),&cancel).await?;
        if value["kind"] != "capability" {
            return Err(Error::NeedsAction(format!(
                "Kubernetes: {}",
                value["reason"].as_str().unwrap_or("unsupported")
            )));
        }
        serde_json::from_value(value).map_err(Error::from)
    }
}
