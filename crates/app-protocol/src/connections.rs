use crate::SourceSpec;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Authentication {
    #[default]
    ConfigAgent,
    PrivateKey,
    Interactive,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ProviderCapability {
    pub kind: String,
    pub state: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct DiscoveryLocation {
    pub label: String,
    pub path: String,
    pub resolved_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct HostCapabilities {
    pub host_id: String,
    pub generation: String,
    pub observed_at_ms: String,
    pub os: String,
    pub user: Option<String>,
    pub home: Option<String>,
    pub helper_ready: bool,
    pub providers: Vec<ProviderCapability>,
    pub locations: Vec<DiscoveryLocation>,
    pub max_sources: u32,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct DiscoveryCoverage {
    pub provider: String,
    pub location: String,
    pub state: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub access: String,
    pub size: Option<String>,
    pub modified_ns: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct DirectoryPage {
    pub path: String,
    pub parent: Option<String>,
    pub resolved_path: Option<String>,
    pub entries: Vec<DirectoryEntry>,
    pub next_cursor: Option<String>,
    pub complete: bool,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
pub struct DiscoveryPreferences {
    #[serde(default)]
    pub initialized: bool,
    pub directories: Vec<String>,
    pub docker: bool,
    pub journal: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct OpenSourceResult {
    pub spec: SourceSpec,
    pub source_id: Option<String>,
    pub state: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
pub struct WorkspaceSelection {
    pub source_ids: Vec<String>,
    pub selected_sources: Vec<String>,
    pub combined_sources: Vec<String>,
    pub selected_host: Option<String>,
    pub restore_previous_readers: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct WorkspaceSource {
    pub key: String,
    pub host_id: String,
    pub remote_identity: String,
    pub spec: SourceSpec,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
pub struct SavedWorkspace {
    pub sources: Vec<WorkspaceSource>,
    pub selected_sources: Vec<String>,
    pub combined_sources: Vec<String>,
    pub selected_host: Option<String>,
    pub restore_previous_readers: bool,
}

#[derive(Clone, Serialize, TS)]
pub struct AuthenticationChallenge {
    pub id: String,
    pub kind: String,
    pub prompt: String,
    pub fingerprint: Option<String>,
}

#[derive(Clone, Serialize, TS)]
pub struct ConnectionAttempt {
    pub id: String,
    pub profile_id: String,
    pub state: String,
    pub challenge: Option<AuthenticationChallenge>,
    pub error: Option<String>,
    pub test_only: bool,
    pub capabilities: Option<HostCapabilities>,
}

#[derive(Clone, Serialize, Deserialize, TS)]
pub struct SourcesOpened {
    pub host_id: String,
    pub source_ids: Vec<String>,
}
