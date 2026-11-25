//! Explicit opt-in synthetic driver only; absent from desktop/normal builds and all wire APIs.
use super::*;
impl AppHandle {
    pub async fn manual_source(&self, id: &str) -> Result<crate::workers::Ingest> {
        let host = HostProfile {
            id: "manual".into(),
            target: "synthetic.invalid".into(),
            port: None,
            user: None,
            identity_agent: None,
            environment: Environment::Production,
        };
        let cancel = self.cancel.child_token();
        let ingest=self.engine.register(id,1).await?;
        self.state.lock().unwrap().contexts.insert(
            id.into(),
            Context {
                host,
                expires: None,
            },
        );
        self.state.lock().unwrap().sources.insert(
            id.into(),
            Source {
                parser: ingest.parser.clone(),
                info: Arc::new(Mutex::new(SourceInfo {
                    id: id.into(),
                    host_id: "manual".into(),
                    spec: SourceSpec::File {
                        path: "/synthetic-only".into(),
                    },
                    state: "synthetic".into(),
                    detail: None,
                    generation: "1".into(),
                })),
                cancel: cancel.clone(),
                task: tokio::spawn(async move { cancel.cancelled().await }),
            },
        );
        Ok(ingest)
    }
}
