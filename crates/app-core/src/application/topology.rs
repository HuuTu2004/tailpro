use super::{output::*, *};
use crate::adapters::{sources::RemoteOperation, ssh::bounded_read};
use tokio::io::AsyncReadExt;
impl AppHandle {
    pub async fn topology(&self, request: TopologyRequest) -> Result<TopologyGraph> {
        self.topology_job(request, false).await
    }
    pub async fn topology_revision(&self, request: TopologyRequest) -> Result<String> {
        Ok(self.topology_job(request, true).await?.config_revision)
    }
    async fn topology_job(
        &self,
        request: TopologyRequest,
        metadata_only: bool,
    ) -> Result<TopologyGraph> {
        if !request.consent
            || request.files.is_empty()
            || request.files.len() > 32
            || request.roots.is_empty()
            || request.roots.len() > 32
            || request.endpoint_names.len() > 16
        {
            return Err(Error::NeedsAction(
                "Separate topology consent, exact Compose files and approved roots required".into(),
            ));
        }
        for path in request.files.iter().chain(&request.roots) {
            policy::path(path)?;
        }
        for name in &request.endpoint_names {
            if name.is_empty()
                || name.len() > 128
                || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                return Err(Error::Invalid("endpoint name allowlist".into()));
            }
        }
        self.scoped_hosts(&request.sources)?;
        let epoch = self.output.state.lock().unwrap().epoch;
        let session = {
            let state = self.state.lock().unwrap();
            state
                .hosts
                .get(&request.host_id)
                .and_then(|h| h.sessions.borrow().clone())
                .ok_or_else(|| Error::NeedsAction("connect host first".into()))?
        };
        let cancel = self
            .views
            .begin(&request.view_id, request.generation, false)?;
        let until = Instant::now() + Duration::from_secs(2);
        let permit = self.output_permit(&cancel, until).await?;
        let _probe = tokio::select! {
            _=cancel.cancelled()=>return Err(Error::Cancelled),
            _=tokio::time::sleep_until(until.into())=>return Err(Error::Timeout),
            p=session.probes.clone().acquire_owned()=>p.map_err(|_|Error::Cancelled)?,
        };
        let mut args = serde_json::to_value(&request)?;
        args["metadata_only"] = metadata_only.into();
        let mut channel = session.spawn(RemoteOperation::Topology(args)).await?;
        let mut output = channel.child.stdout.take().unwrap().take(256 * 1024 + 1);
        let diagnostic = tokio::spawn(bounded_read(channel.child.stderr.take().unwrap(), 4096));
        let mut bytes = Vec::new();
        let read = tokio::select! {
            _=cancel.cancelled()=>Err(Error::Cancelled),
            _=session.cancel.cancelled()=>Err(Error::Cancelled),
            r=tokio::time::timeout_at(until.into(),output.read_to_end(&mut bytes))=>match r {Ok(Ok(_))=>Ok(()),Ok(Err(e))=>Err(e.into()),Err(_)=>Err(Error::Timeout)},
        };
        channel.close().await;
        diagnostic.abort();
        read?;
        if bytes.len() > 256 * 1024 {
            return Err(Error::Limit("topology payload"));
        }
        if metadata_only {
            let work_cancel = cancel.clone();
            let graph = tokio::task::spawn_blocking(move || -> Result<TopologyGraph> {
                let _permit = permit;
                check(&work_cancel, until)?;
                let graph: TopologyGraph = serde_json::from_slice(&bytes)?;
                check(&work_cancel, until)?;
                Ok(graph)
            })
            .await
            .map_err(|_| Error::Cancelled)??;
            if epoch != self.output.state.lock().unwrap().epoch || !graph.complete {
                return Err(denied());
            }
            return Ok(graph);
        }
        let settings = self.settings.clone();
        let hosts = self.scoped_hosts(&request.sources)?;
        let snapshot = self.engine.snapshot().await?;
        let work_cancel = cancel.clone();
        let mut graph = tokio::task::spawn_blocking(move || -> Result<TopologyGraph> {
            let _permit = permit;
            check(&work_cancel, until)?;
            let mappings = settings.lock().unwrap().load()?.service_mappings;
            let mut graph: TopologyGraph = serde_json::from_slice(&bytes)?;
            if graph.nodes.len() > 256 || graph.edges.len() > 512 {
                return Err(Error::Limit("topology graph"));
            }
            let mut counts = HashMap::<String, u32>::new();
            let mut observed = 0;
            for e in snapshot.events()? {
                check(&work_cancel, until)?;
                if !request.sources.contains(&e.source) || hosts[&e.source].id != request.host_id {
                    continue;
                }
                let (p, _) = redaction::project(e);
                if p.level == log_core::model::Level::Error {
                    if observed == 500 {
                        graph.complete = false;
                        graph.reasons.push("500 observed error sample cap".into());
                        break;
                    }
                    observed += 1;
                    *counts.entry(e.source.clone()).or_default() += 1;
                }
            }
            for node in &mut graph.nodes {
                node.sources = mappings
                    .iter()
                    .filter(|m| {
                        m.service == node.id
                            && request.sources.contains(&m.source)
                            && hosts[&m.source].id == request.host_id
                    })
                    .map(|m| m.source.clone())
                    .collect();
                if !node.sources.is_empty() {
                    node.observed_errors = Some(
                        node.sources
                            .iter()
                            .map(|s| counts.get(s).copied().unwrap_or(0))
                            .sum(),
                    );
                    if node
                        .sources
                        .iter()
                        .any(|s| snapshot.source_losses.contains_key(s))
                    {
                        graph.complete = false;
                        graph.reasons.push(
                            "authorized retained coverage has ingestion/retention gaps".into(),
                        );
                    }
                }
            }
            graph.reasons.sort();
            graph.reasons.dedup();
            check(&work_cancel, until)?;
            Ok(graph)
        })
        .await
        .map_err(|_| Error::Cancelled)??;
        check(&cancel, until)?;
        if epoch != self.output.state.lock().unwrap().epoch {
            return Err(denied());
        }
        if serde_json::to_vec(&graph)?.len() > 256 * 1024 {
            return Err(Error::Limit("topology result"));
        }
        graph.schema_version = WIRE_VERSION;
        Ok(graph)
    }
}
