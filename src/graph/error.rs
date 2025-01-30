use thiserror::Error;

#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Edge not found between {from} and {to}")]
    EdgeNotFound { from: String, to: String },

    #[error("Invalid node type: {0}")]
    InvalidNodeType(String),

    #[error("Vector store error: {0}")]
    VectorStoreError(String),

    #[error("Document parsing error: {0}")]
    ParseError(String),

    #[error("Embedding generation error: {0}")]
    EmbeddingError(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
