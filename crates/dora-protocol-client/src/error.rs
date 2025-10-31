use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolClientError {
    #[error("invalid base url: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("failed to decode response: {0}")]
    Deserialize(#[from] serde_json::Error),
    #[error("stream error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Protocol(String),
}
