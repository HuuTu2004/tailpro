mod checkpoint;
mod journal;
mod reader;
use crate::{
    api::{Error, Result},
    application::policy,
};
pub use checkpoint::*;
pub(crate) use reader::{Reader, run};
use serde_json::Value;

/// No arbitrary-command variant. Only bundled, read-only remote templates.
pub(crate) enum RemoteOperation {
    Docker { id: String, since: Option<String> },
    File(Value),
    DockerCapability(Value),
    Journal(Value),
    Topology(Value),
    Kubernetes(Value),
    Discovery(Value),
    HostMetadata(Value),
}
impl RemoteOperation {
    pub fn render(self) -> Result<String> {
        match self {
            Self::Docker { id, since } => {
                policy::container_id(&id)?;
                let mut command = String::from("docker logs --timestamps --follow");
                if let Some(since) = since {
                    if since.len() > 40
                        || !since
                            .bytes()
                            .all(|b| b.is_ascii_digit() || b"-:TZ.+".contains(&b))
                    {
                        return Err(Error::Invalid("invalid timestamp".into()));
                    }
                    command.push_str(" --since ");
                    command.push_str(&policy::quote(&since));
                }
                command.push(' ');
                command.push_str(&policy::quote(&id));
                Ok(command)
            }
            Self::DockerCapability(args) => Ok(python(include_str!("docker_capability.py"), &args)),
            Self::Kubernetes(args) => Ok(python(include_str!("kubernetes_helper.py"), &args)),
            Self::Journal(args) => {
                policy::unit(args["unit"].as_str().unwrap_or(""))?;
                Ok(python(include_str!("journal_helper.py"), &args))
            }
            Self::Topology(args) => Ok(python(include_str!("../discovery/topology.py"), &args)),
            Self::File(args) => {
                policy::path(
                    args["path"]
                        .as_str()
                        .ok_or_else(|| Error::Invalid("file path".into()))?,
                )?;
                Ok(python(include_str!("file_helper.py"), &args))
            }
            Self::Discovery(args) => Ok(python(include_str!("../discovery/probe.py"), &args)),
            Self::HostMetadata(args) => Ok(python(include_str!("../discovery/inspect.py"), &args)),
        }
    }
}
fn python(script: &str, args: &Value) -> String {
    format!(
        "python3 -u -c {} {}",
        policy::quote(script),
        policy::quote(&args.to_string())
    )
}

pub(crate) async fn inspect(
    session: &crate::adapters::ssh::Session,
    operation: RemoteOperation,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<Value> {
    let output_cap = if matches!(&operation, RemoteOperation::HostMetadata(_)) {
        256 * 1024
    } else {
        32 * 1024
    };
    let _permit = tokio::select! {_=cancel.cancelled()=>return Err(Error::Cancelled),p=session.probes.clone().acquire_owned()=>p.map_err(|_|Error::Cancelled)?};
    let mut channel = session.spawn(operation).await?;
    let stdout = channel.child.stdout.take().unwrap();
    let stderr = channel.child.stderr.take().unwrap();
    let diagnostic = tokio::spawn(crate::adapters::ssh::bounded_read(stderr, 4096));
    let result = tokio::select! {
        _=cancel.cancelled()=>Err(Error::Cancelled),_=session.cancel.cancelled()=>Err(Error::Cancelled),
        r=tokio::time::timeout(std::time::Duration::from_secs(15),crate::adapters::ssh::bounded_read(stdout,output_cap))=>r.map_err(|_|Error::Timeout).and_then(|r|r).and_then(|bytes|serde_json::from_slice(&bytes).map_err(Error::from))
    };
    channel.close().await;
    diagnostic.abort();
    result
}
