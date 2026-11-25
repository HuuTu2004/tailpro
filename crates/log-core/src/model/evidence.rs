use serde::Serialize;
use super::ingestion::{FieldOrigin, SeverityNamespace};

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all="snake_case")]
pub enum Decoder { Json, Logfmt, Header, Rule, #[default] Raw }
#[derive(Debug, Serialize)]
pub struct ParseEvidence {
    pub decoder: Decoder,
    pub revision: u64,
    pub conflict: bool,
    pub time_origin: FieldOrigin,
    pub epoch_unit: Option<super::ingestion::EpochUnit>,
    pub event_time_ns: Option<String>,
    pub time_literal: Option<String>,
    pub time_path: Option<&'static str>,
    pub severity_origin: FieldOrigin,
    pub severity_namespace: SeverityNamespace,
    pub severity_literal: Option<String>,
    pub http_status: Option<u16>,
    pub derived_http_error: bool,
}
impl Default for ParseEvidence {
    fn default() -> Self { Self { decoder: Decoder::Raw, revision: 1, conflict: false, time_origin: FieldOrigin::Unknown, epoch_unit: None, event_time_ns: None, time_literal: None, time_path: None, severity_origin: FieldOrigin::Unknown, severity_namespace: SeverityNamespace::Unknown, severity_literal: None, http_status: None, derived_http_error: false } }
}
impl ParseEvidence {
    pub fn charged_bytes(&self) -> usize { self.event_time_ns.as_ref().map_or(0,String::capacity) + self.time_literal.as_ref().map_or(0,String::capacity) + self.severity_literal.as_ref().map_or(0,String::capacity) }
}
