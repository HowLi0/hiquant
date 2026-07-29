//! 全局错误类型

use thiserror::Error;

pub type Result<T> = std::result::Result<T, HiquantError>;

#[derive(Debug, Error)]
pub enum HiquantError {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("account error: {0}")]
    Account(String),

    #[error("position error: {0}")]
    Position(String),

    #[error("order error: {0}")]
    Order(String),

    #[error("market error: {0}")]
    Market(String),

    #[error("broker error: {0}")]
    Broker(String),

    #[error("data source error: {0}")]
    DataSource(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("risk check failed: {0}")]
    RiskCheck(String),

    #[error("io error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("serde json error: {source}")]
    SerdeJson {
        #[from]
        source: serde_json::Error,
    },

    #[error("other: {0}")]
    Other(String),
}

impl HiquantError {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
