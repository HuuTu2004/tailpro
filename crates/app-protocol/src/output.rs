use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS, schemars::JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ReadRequest {
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub query: String,
    pub limit: Option<u32>,
    pub max_bytes: Option<u32>,
    pub cursor: Option<String>,
    pub evidence_ref: Option<String>,
    pub service: Option<String>,
    pub correlation: Option<Correlation>,
    pub from_ms: Option<String>,
    pub to_ms: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Evidence {
    pub evidence_ref: String,
    pub source: String,
    pub server: String,
    pub environment: EnvironmentLabel,
    pub level: String,
    pub text: String,
    pub observed_at_ms: String,
    pub event_time: Option<String>,
    #[serde(default)]
    pub transport_time: Option<String>,
    #[serde(default)]
    pub provenance: String,
    pub redaction_incomplete: bool,
    pub quality: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentLabel {
    Development,
    Staging,
    Production,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct GrantedSource {
    pub id: String,
    pub server: String,
    pub environment: EnvironmentLabel,
    pub health: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ReadResponse {
    #[serde(default)]
    pub query_scanned: String,
    #[serde(default)]
    pub query_matched_lower_bound: String,
    #[serde(default)]
    pub query_returned: u32,
    pub schema_version: u32,
    pub sources: Vec<GrantedSource>,
    pub events: Vec<Evidence>,
    pub time_basis: String,
    pub coverage: String,
    pub complete: bool,
    pub gaps: Vec<String>,
    pub truncated: bool,
    pub redaction_incomplete: bool,
    pub next_cursor: Option<String>,
    pub observation_cursor: Option<String>,
    pub trace_ref: Option<String>,
    pub capabilities: Vec<String>,
    pub candidates: Vec<ServiceCandidate>,
    pub ambiguous: bool,
    pub clusters: Vec<ErrorCluster>,
    pub timeline: Vec<TracePoint>,
    pub code_hints: Vec<CodeHint>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct GrantInfo {
    pub id: String,
    pub client: String,
    pub sources: Vec<String>,
    pub expires_at_ms: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Enrollment {
    pub grant: GrantInfo,
    pub token: String,
    pub session: String,
    pub runtime_dir: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub sources: Vec<String>,
    pub event_ids: Vec<String>,
    pub from_ms: Option<String>,
    pub to_ms: Option<String>,
    pub query: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Ndjson,
    Csv,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct PreparedPreview {
    pub id: String,
    pub digest: String,
    pub payload: String,
    pub expires_at_ms: String,
    pub redaction_incomplete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExplainProvider {
    Ollama { endpoint: String, model: String },
    Byok { endpoint: String, model: String },
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Explanation {
    pub evidence: Vec<String>,
    pub hypothesis: String,
    pub untrusted: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeHello {
    pub schema_version: u32,
    pub session: String,
    pub grant: String,
    pub token: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeRequest {
    pub tool: String,
    pub request: ReadRequest,
}
#[derive(Serialize, Deserialize)]
pub struct BridgeReply {
    pub result: Option<ReadResponse>,
    pub error: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct BridgeRegistry {
    pub schema_version: u32,
    pub session: String,
    pub socket: String,
}

pub fn read_request_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(ReadRequest)).expect("static read schema")
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, schemars::JsonSchema)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Correlation {
    TraceId(String),
    OrderId(String),
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ServiceMapping {
    pub source: String,
    pub service: String,
    pub project: String,
    pub aliases: Vec<String>,
    pub repo_label: String,
    pub repo_root: String,
    pub revision: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ServiceCandidate {
    pub source: String,
    pub aliases: Vec<String>,
    pub provenance: String,
    pub confidence: String,
    pub next_tools: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ErrorCluster {
    pub group_ref: String,
    pub signature_version: String,
    pub signature: String,
    pub observed_count: u32,
    pub sources: Vec<String>,
    pub first_observed_ms: String,
    pub last_observed_ms: String,
    pub representative_refs: Vec<String>,
    pub incomplete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct TracePoint {
    pub source: String,
    pub evidence_ref: String,
    pub observed_at_ms: String,
    pub event_time: Option<String>,
    pub duration_ms: Option<f64>,
    pub semantics: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct CodeHint {
    pub repo_label: String,
    pub relative_path: String,
    pub line: u32,
    pub evidence_ref: String,
    pub confidence: String,
    pub declared_revision: Option<String>,
    pub provenance: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct TopologyRequest {
    pub view_id: String,
    pub generation: u32,
    pub host_id: String,
    pub files: Vec<String>,
    pub roots: Vec<String>,
    pub endpoint_names: Vec<String>,
    pub consent: bool,
    pub sources: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct TopologyNode {
    pub id: String,
    pub sources: Vec<String>,
    pub observed_errors: Option<u32>,
    pub provenance: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct TopologyEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub provenance: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct TopologyGraph {
    pub schema_version: u32,
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
    pub complete: bool,
    pub reasons: Vec<String>,
    pub coverage: String,
    pub config_digest: String,
    pub config_revision: String,
}

/// Each tool advertises only the arguments it uses; the runtime also validates.
pub fn tool_request_schema(tool: &str) -> serde_json::Value {
    let mut schema = read_request_schema();
    let common = ["sources", "limit", "max_bytes"];
    let specific: &[&str] = match tool {
        "locate_service" => &["service"],
        "get_error_clusters" => &["query", "from_ms", "to_ms"],
        "trace_id" => &["query", "correlation", "from_ms", "to_ms"],
        "inspect_context" => &["evidence_ref"],
        "get_log_context" => &["query", "evidence_ref"],
        _ => &["query", "cursor"],
    };
    if let Some(properties) = schema["properties"].as_object_mut() {
        properties
            .retain(|key, _| common.contains(&key.as_str()) || specific.contains(&key.as_str()));
    }
    schema["required"] = match tool {
        "locate_service" => serde_json::json!(["service"]),
        "trace_id" => serde_json::json!(["correlation"]),
        "inspect_context" | "get_log_context" => serde_json::json!(["evidence_ref"]),
        _ => serde_json::json!([]),
    };
    schema
}
