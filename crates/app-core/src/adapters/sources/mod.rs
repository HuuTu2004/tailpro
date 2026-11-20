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
    Journal(Value),
    Topology(Value),
    Discovery(Value),
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
