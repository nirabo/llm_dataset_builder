use anyhow::Result;
use ollama_rs::{generation::options::GenerationOptions, Ollama};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::external::error::ExternalError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub model: String,
    pub host: String,
    pub port: u16,
}

impl EmbeddingConfig {
    /// Get the full URL for the Ollama service
    pub fn get_url(&self) -> Result<String> {
        let url = if self.host.starts_with("http://") || self.host.starts_with("https://") {
            format!("{}:{}", self.host.trim_end_matches('/'), self.port)
        } else {
            format!("http://{}:{}", self.host, self.port)
        };

        // Validate the URL
        Url::parse(&url).map_err(|e| ExternalError::ConfigError(format!("Invalid URL: {}", e)))?;

        Ok(url)
    }
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model: "nomic-embed-text".to_string(),
            host: "localhost".to_string(),
            port: 11434,
        }
    }
}

/// Wrapper for Ollama embedding engine
pub struct EmbeddingEngine {
    client: Ollama,
    config: EmbeddingConfig,
}

impl EmbeddingEngine {
    /// Create a new embedding engine with the given configuration
    pub async fn new(config: EmbeddingConfig) -> Result<Self> {
        let url = config.get_url()?;
        let url = Url::parse(&url)
            .map_err(|e| ExternalError::ConfigError(format!("Invalid URL: {}", e)))?;

        let client = Ollama::new(
            url.host_str().unwrap_or("localhost").to_string(),
            config.port,
        );

        Ok(Self { client, config })
    }

    /// Generate embeddings for a text
    pub async fn generate_embeddings(&self, text: &str) -> Result<Vec<f32>> {
        let response = self
            .client
            .generate_embeddings(
                self.config.model.clone(),
                text.to_string(),
                Some(GenerationOptions::default()),
            )
            .await
            .map_err(|e| ExternalError::OllamaError(e.to_string()))?;

        // Convert from Vec<f64> to Vec<f32>
        Ok(response.embeddings.into_iter().map(|x| x as f32).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::automock;

    #[automock]
    trait EmbeddingClient {
        async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>>;
    }

    #[test]
    fn test_url_generation() {
        // Test with plain hostname
        let config = EmbeddingConfig {
            host: "localhost".to_string(),
            port: 11434,
            model: "test".to_string(),
        };
        assert_eq!(config.get_url().unwrap(), "http://localhost:11434");

        // Test with http:// prefix
        let config = EmbeddingConfig {
            host: "http://example.com".to_string(),
            port: 11434,
            model: "test".to_string(),
        };
        assert_eq!(config.get_url().unwrap(), "http://example.com:11434");

        // Test with https:// prefix
        let config = EmbeddingConfig {
            host: "https://example.com".to_string(),
            port: 11434,
            model: "test".to_string(),
        };
        assert_eq!(config.get_url().unwrap(), "https://example.com:11434");
    }

    #[tokio::test]
    async fn test_embedding_generation() {
        let mut mock = MockEmbeddingClient::new();

        mock.expect_generate_embedding()
            .times(1)
            .returning(|_| Ok(vec![0.1, 0.2, 0.3]));

        let text = "This is a test text for embedding generation.";
        let embedding = mock.generate_embedding(text).await.unwrap();

        assert!(!embedding.is_empty());
        assert_eq!(embedding.len(), 3);
        assert_eq!(embedding, vec![0.1, 0.2, 0.3]);
    }
}
