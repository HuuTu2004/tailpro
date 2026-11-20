//! Versioned wire contracts. No application runtime or MCP SDK dependency.
mod desktop;
mod output;
pub use desktop::*;
pub use output::*;
pub const WIRE_VERSION: u32 = 2;
pub const REDACTION_PROFILE: &str = "tailpro-redaction-v1";
