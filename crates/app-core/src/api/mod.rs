mod dto;
pub use dto::*;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NeedsAction(String),
    #[error("operation cancelled")]
    Cancelled,
    #[error("operation deadline exceeded")]
    Timeout,
    #[error("resource limit: {0}")]
    Limit(&'static str),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol: {0}")]
    Json(#[from] serde_json::Error),
    #[error("engine: {0}")]
    Engine(#[from] log_core::engine::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
