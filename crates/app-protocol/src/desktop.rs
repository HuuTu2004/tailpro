use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Development,
    Staging,
    Production,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct HostProfile {
    pub id: String,
    pub target: String,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub identity_agent: Option<String>,
    pub environment: Environment,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Connecting,
    Connected,
    Reconnecting,
    NeedsAction,
    Disconnected,
}
#[derive(Clone, Debug, Serialize, TS)]
pub struct ConnectionInfo {
    pub host: HostProfile,
    pub state: ConnectionState,
    pub detail: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Ready,
    Denied,
    Missing,
    Unknown,
    Unsupported,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Candidate {
    pub file_type: String,
    pub confidence: String,
    pub process_hint: Option<String>,
    pub identity: Option<String>,
    pub id: String,
    pub host_id: String,
    pub path: String,
    pub provider: String,
    pub provenance: String,
    pub access: Access,
    pub size: Option<String>,
    pub mtime: Option<String>,
    pub observed_at_ms: String,
    pub tags: Vec<String>,
    pub container_id: Option<String>,
    pub activity: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct DiscoveryRequest {
    pub host_id: String,
    pub project_roots: Vec<String>,
    pub docker: bool,
    pub var_log: bool,
    pub pm2_root: Option<String>,
    pub selected_pids: Vec<u32>,
    pub selected_units: Vec<String>,
    pub config_files: Vec<String>,
    pub config_roots: Vec<String>,
    pub config_consent: bool,
}
#[derive(Clone, Debug, Serialize, TS)]
pub struct DiscoveryResult {
    pub candidates: Vec<Candidate>,
    pub complete: bool,
    pub reasons: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceSpec {
    Docker {
        container_id: String,
        #[serde(default)]
        aliases: Vec<String>,
    },
    File {
        path: String,
    },
    Journal {
        unit: String,
    },
    Kubernetes {
        kubeconfig: String,
        context: String,
        namespace: String,
        pod: String,
        pod_uid: String,
        container: String,
        container_id: String,
        restart_count: u32,
        config_digest: String,
        trusted: bool,
        previous: bool,
        #[serde(default)]
        previous_container_id: Option<String>,
    },
}
#[derive(Clone, Debug, Serialize, TS)]
pub struct SourceInfo {
    pub id: String,
    pub host_id: String,
    pub spec: SourceSpec,
    pub state: String,
    pub detail: Option<String>,
    pub generation: String,
}
#[derive(Serialize, TS)]
pub struct Health {
    pub policy_revision: String,
    pub schema_version: u32,
    pub status: String,
    pub read_only: bool,
}
#[derive(Serialize, TS)]
pub struct AppStatus {
    pub health: Health,
    pub connections: Vec<ConnectionInfo>,
    pub sources: Vec<SourceInfo>,
}
#[derive(Serialize, TS)]
pub struct ConfigImport {
    pub aliases: Vec<String>,
    pub disclosures: Vec<String>,
    pub fingerprint: String,
}

pub fn typescript() -> String {
    let config = ts_rs::Config::default();
    let declarations = [
        crate::Correlation::decl(&config),
        crate::ServiceMapping::decl(&config),
        crate::ServiceCandidate::decl(&config),
        crate::ErrorCluster::decl(&config),
        crate::TracePoint::decl(&config),
        crate::CodeHint::decl(&config),
        crate::TopologyRequest::decl(&config),
        crate::TopologyNode::decl(&config),
        crate::TopologyEdge::decl(&config),
        crate::TopologyGraph::decl(&config),
        crate::ReadRequest::decl(&config),
        crate::Evidence::decl(&config),
        crate::EnvironmentLabel::decl(&config),
        crate::GrantedSource::decl(&config),
        crate::ReadResponse::decl(&config),
        crate::GrantInfo::decl(&config),
        crate::Enrollment::decl(&config),
        crate::Selection::decl(&config),
        crate::ExportFormat::decl(&config),
        crate::PreparedPreview::decl(&config),
        crate::ExplainProvider::decl(&config),
        crate::Explanation::decl(&config),
        Environment::decl(&config),
        HostProfile::decl(&config),
        ConnectionState::decl(&config),
        ConnectionInfo::decl(&config),
        Access::decl(&config),
        Candidate::decl(&config),
        DiscoveryRequest::decl(&config),
        DiscoveryResult::decl(&config),
        SourceSpec::decl(&config),
        SourceInfo::decl(&config),
        KubernetesCapability::decl(&config),
        Health::decl(&config),
        AppStatus::decl(&config),
        ConfigImport::decl(&config),
        ViewRequest::decl(&config),
        ViewRow::decl(&config),
        ViewPage::decl(&config),
        ViewNotice::decl(&config),
        SavedView::decl(&config),
        DetailRequest::decl(&config),
        InspectorNode::decl(&config),
        EventDetail::decl(&config),
    ];
    format!(
        "// Generated by app-protocol/export-contracts. Do not edit.\n{}\n",
        declarations
            .into_iter()
            .map(|d| format!("export {d}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ViewRequest {
    pub view_id: String,
    pub generation: u32,
    pub query: String,
    pub sources: Vec<String>,
    pub before: Option<String>,
    pub anchor: Option<String>,
}
#[derive(Clone, Serialize, TS)]
pub struct ViewRow {
    pub id: String,
    pub source: String,
    pub server: String,
    pub environment: Environment,
    pub level: String,
    pub message: String,
    pub time: String,
    pub time_basis: String,
    pub trace_id: Option<String>,
    pub duration_ms: Option<f64>,
    pub quality: String,
}
#[derive(Serialize, TS)]
pub struct ViewPage {
    pub gap_sources: Vec<String>,
    pub coverage: String,
    pub schema_version: u32,
    pub generation: u32,
    pub revision: String,
    pub events: Vec<ViewRow>,
    pub older_cursor: Option<String>,
    pub complete: bool,
    pub truncated: bool,
    pub anchor_expired: bool,
    pub scanned: u32,
    pub elapsed_us: String,
}
#[derive(Clone, Serialize, TS)]
pub struct ViewNotice {
    pub schema_version: u32,
    pub revision: String,
    pub accepted: String,
    pub discarded: String,
    pub evicted: String,
    pub retained_rows: String,
    pub resident_bytes: String,
    pub complete: bool,
    pub gap_sources: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct SavedView {
    pub name: String,
    pub query: String,
    pub sources: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct DetailRequest {
    pub view_id: String,
    pub generation: u32,
    pub event_id: String,
    pub path: Vec<u32>,
    pub offset: u32,
    pub include_raw: bool,
}
#[derive(Serialize, TS)]
pub struct InspectorNode {
    pub index: u32,
    pub key: String,
    pub value: String,
    pub kind: String,
    pub truncated: bool,
}
#[derive(Serialize, TS)]
pub struct EventDetail {
    pub schema_version: u32,
    pub generation: u32,
    pub event_id: String,
    pub raw: Option<String>,
    pub raw_hex: Option<String>,
    pub raw_bytes: String,
    pub evidence_json: String,
    pub quality: String,
    pub group_id: Option<String>,
    pub group_parts: Vec<String>,
    pub group_parts_truncated: bool,
    pub nodes: Vec<InspectorNode>,
    pub more: bool,
    pub json_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct KubernetesCapability {
    pub config_digest: String,
    pub pod_uid: String,
    pub container_id: String,
    pub restart_count: u32,
    pub previous_available: bool,
    pub previous_container_id: Option<String>,
    pub coverage: String,
}
