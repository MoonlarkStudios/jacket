#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("{0}")]
    Invalid(String),
    #[error("Audio tool failed: {0}")]
    Tool(String),
    #[error("Cancelled. No incomplete export was saved.")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("Audio metadata failed: {0}")]
    Metadata(String),
}

pub type Result<T> = std::result::Result<T, EngineError>;
