use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExternalError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Ollama error: {0}")]
    OllamaError(String),

    #[error("Vector DB error: {0}")]
    VectorDBError(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
