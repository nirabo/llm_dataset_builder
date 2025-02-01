use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

use crate::external::{EmbeddingConfig, LLMConfig, VectorDBConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    pub batch_size: usize,
    pub max_concurrent_requests: usize,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub output_dir: String,
    pub vector_db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub embedding: EmbeddingConfig,
    pub llm: LLMConfig,
    pub vector_db: VectorDBConfig,
    pub processing: ProcessingConfig,
    pub output: OutputConfig,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        // Load embedding config
        let embedding = EmbeddingConfig {
            model: env::var("OLLAMA_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "nomic-embed-text".to_string()),
            host: env::var("OLLAMA_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: env::var("OLLAMA_PORT")
                .unwrap_or_else(|_| "11434".to_string())
                .parse()
                .unwrap_or(11434),
        };

        // Load LLM config
        let llm = LLMConfig {
            model: env::var("OLLAMA_LLM_MODEL").unwrap_or_else(|_| "mistral".to_string()),
            host: env::var("OLLAMA_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: env::var("OLLAMA_PORT")
                .unwrap_or_else(|_| "11434".to_string())
                .parse()
                .unwrap_or(11434),
            temperature: env::var("OLLAMA_TEMPERATURE")
                .unwrap_or_else(|_| "0.7".to_string())
                .parse()
                .unwrap_or(0.7),
            top_p: env::var("OLLAMA_TOP_P")
                .unwrap_or_else(|_| "0.9".to_string())
                .parse()
                .unwrap_or(0.9),
        };

        // Load vector DB config
        let vector_db = VectorDBConfig {
            collection_name: env::var("QDRANT_COLLECTION")
                .unwrap_or_else(|_| "documents".to_string()),
            host: env::var("QDRANT_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: env::var("QDRANT_PORT")
                .unwrap_or_else(|_| "6334".to_string())
                .parse()
                .unwrap_or(6334),
            vector_size: env::var("QDRANT_VECTOR_SIZE")
                .unwrap_or_else(|_| "384".to_string())
                .parse()
                .unwrap_or(384),
        };

        // Load processing config
        let processing = ProcessingConfig {
            batch_size: env::var("BATCH_SIZE")
                .unwrap_or_else(|_| "32".to_string())
                .parse()
                .unwrap_or(32),
            max_concurrent_requests: env::var("MAX_CONCURRENT_REQUESTS")
                .unwrap_or_else(|_| "4".to_string())
                .parse()
                .unwrap_or(4),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
        };

        // Load output config
        let output = OutputConfig {
            output_dir: env::var("OUTPUT_DIR").unwrap_or_else(|_| "./output".to_string()),
            vector_db_path: env::var("VECTOR_DB_PATH")
                .unwrap_or_else(|_| "./vector_db".to_string()),
        };

        Ok(Self {
            embedding,
            llm,
            vector_db,
            processing,
            output,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scopeguard::guard;
    use std::env;

    fn clean_env() {
        env::remove_var("OLLAMA_EMBEDDING_MODEL");
        env::remove_var("OLLAMA_LLM_MODEL");
        env::remove_var("OLLAMA_HOST");
        env::remove_var("OLLAMA_PORT");
        env::remove_var("OLLAMA_TEMPERATURE");
        env::remove_var("OLLAMA_TOP_P");
        env::remove_var("QDRANT_COLLECTION");
        env::remove_var("QDRANT_HOST");
        env::remove_var("QDRANT_PORT");
        env::remove_var("QDRANT_VECTOR_SIZE");
        env::remove_var("BATCH_SIZE");
        env::remove_var("MAX_CONCURRENT_REQUESTS");
        env::remove_var("LOG_LEVEL");
        env::remove_var("OUTPUT_DIR");
        env::remove_var("VECTOR_DB_PATH");
    }

    #[test]
    #[serial_test::serial]
    fn test_default_config() {
        clean_env();
        let _guard = guard((), |_| clean_env());

        let config = Config::from_env().unwrap();

        // Check default values
        assert_eq!(
            config.embedding.model, "nomic-embed-text",
            "wrong default embedding model"
        );
        assert_eq!(config.llm.model, "mistral", "wrong default llm model");
        assert_eq!(
            config.vector_db.collection_name, "documents",
            "wrong default collection name"
        );
        assert_eq!(config.processing.batch_size, 32, "wrong default batch size");
        assert_eq!(
            config.output.output_dir, "./output",
            "wrong default output dir"
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_custom_config() {
        clean_env();
        let _guard = guard((), |_| clean_env());

        // Set custom environment variables
        env::set_var("OLLAMA_EMBEDDING_MODEL", "custom-embed");
        env::set_var("OLLAMA_LLM_MODEL", "custom-llm");
        env::set_var("QDRANT_COLLECTION", "custom-collection");
        env::set_var("BATCH_SIZE", "64");
        env::set_var("OUTPUT_DIR", "/custom/output");

        // Create config after setting environment variables
        let config = Config::from_env().unwrap();

        // Check custom values
        assert_eq!(
            config.embedding.model, "custom-embed",
            "embedding model mismatch"
        );
        assert_eq!(config.llm.model, "custom-llm", "llm model mismatch");
        assert_eq!(
            config.vector_db.collection_name, "custom-collection",
            "collection name mismatch"
        );
        assert_eq!(config.processing.batch_size, 64, "batch size mismatch");
        assert_eq!(
            config.output.output_dir, "/custom/output",
            "output dir mismatch"
        );
    }
}
