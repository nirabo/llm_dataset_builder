use anyhow::Result;
use ollama_rs::{
    generation::{completion::request::GenerationRequest, options::GenerationOptions},
    Ollama,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::external::error::ExternalError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub model: String,
    pub host: String,
    pub port: u16,
    pub temperature: f32,
    pub top_p: f32,
}

impl LLMConfig {
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

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            model: "mistral".to_string(),
            host: "localhost".to_string(),
            port: 11434,
            temperature: 0.7,
            top_p: 0.9,
        }
    }
}

/// Wrapper for Ollama LLM engine
pub struct LLMEngine {
    client: Ollama,
    config: LLMConfig,
}

impl LLMEngine {
    /// Create a new LLM engine with the given configuration
    pub async fn new(config: LLMConfig) -> Result<Self> {
        let url = config.get_url()?;
        let url = Url::parse(&url)
            .map_err(|e| ExternalError::ConfigError(format!("Invalid URL: {}", e)))?;

        let client = Ollama::new(
            url.host_str().unwrap_or("localhost").to_string(),
            config.port,
        );

        Ok(Self { client, config })
    }

    /// Generate text completion
    pub async fn generate(&self, prompt: &str) -> Result<String> {
        let mut request = GenerationRequest::new(self.config.model.clone(), prompt.to_string());

        let options = GenerationOptions::default()
            .temperature(self.config.temperature)
            .top_p(self.config.top_p);

        request.options = Some(options);

        let response = self
            .client
            .generate(request)
            .await
            .map_err(|e| ExternalError::OllamaError(e.to_string()))?;

        Ok(response.response)
    }

    /// Generate question-answer pair from context
    pub async fn generate_qa_pair(&self, context: &str) -> Result<(String, String)> {
        let prompt = format!(
            "Based on the following context, generate a question and answer pair. \
            Format your response exactly as follows (including the labels):\n\
            Question: <question>\n\
            Answer: <answer>\n\n\
            Context:\n{}",
            context
        );

        let response = self.generate(&prompt).await?;

        // Parse response into question and answer
        let mut question = String::new();
        let mut answer = String::new();

        for line in response.lines() {
            if let Some(stripped) = line.strip_prefix("Question: ") {
                question = stripped.to_string();
            } else if let Some(stripped) = line.strip_prefix("Answer: ") {
                answer = stripped.to_string();
            }
        }

        if question.is_empty() || answer.is_empty() {
            return Err(ExternalError::OllamaError("Failed to parse QA pair".to_string()).into());
        }

        Ok((question, answer))
    }

    /// Generate multiple QA pairs from the same context
    pub async fn generate_qa_pairs(
        &self,
        context: &str,
        count: usize,
    ) -> Result<Vec<(String, String)>> {
        let prompt = format!(
            "Based on the following context, generate {} different question and answer pairs. \
            Format each pair exactly as follows (including the labels):\n\
            Question: <question>\n\
            Answer: <answer>\n\n\
            Generate each pair on new lines. Make the questions diverse and non-overlapping.\n\n\
            Context:\n{}",
            count, context
        );

        let response = self.generate(&prompt).await?;
        let mut pairs = Vec::new();

        let mut current_question = String::new();
        let mut current_answer = String::new();

        for line in response.lines() {
            if let Some(stripped) = line.strip_prefix("Question: ") {
                if !current_question.is_empty() && !current_answer.is_empty() {
                    pairs.push((current_question.clone(), current_answer.clone()));
                }
                current_question = stripped.to_string();
                current_answer.clear();
            } else if let Some(stripped) = line.strip_prefix("Answer: ") {
                current_answer = stripped.to_string();
            }
        }

        // Add the last pair if it exists
        if !current_question.is_empty() && !current_answer.is_empty() {
            pairs.push((current_question, current_answer));
        }

        Ok(pairs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::automock;

    #[automock]
    trait LLMClient {
        async fn generate(&self, prompt: &str) -> Result<String>;
        async fn generate_qa_pair(&self, context: &str) -> Result<(String, String)>;
        async fn generate_qa_pairs(
            &self,
            context: &str,
            count: usize,
        ) -> Result<Vec<(String, String)>>;
    }

    #[test]
    fn test_url_generation() {
        // Test with plain hostname
        let config = LLMConfig {
            host: "localhost".to_string(),
            port: 11434,
            model: "test".to_string(),
            temperature: 0.7,
            top_p: 0.9,
        };
        assert_eq!(config.get_url().unwrap(), "http://localhost:11434");

        // Test with http:// prefix
        let config = LLMConfig {
            host: "http://example.com".to_string(),
            port: 11434,
            model: "test".to_string(),
            temperature: 0.7,
            top_p: 0.9,
        };
        assert_eq!(config.get_url().unwrap(), "http://example.com:11434");

        // Test with https:// prefix
        let config = LLMConfig {
            host: "https://example.com".to_string(),
            port: 11434,
            model: "test".to_string(),
            temperature: 0.7,
            top_p: 0.9,
        };
        assert_eq!(config.get_url().unwrap(), "https://example.com:11434");
    }

    #[tokio::test]
    async fn test_text_generation() {
        let mut mock = MockLLMClient::new();

        mock.expect_generate()
            .times(1)
            .returning(|_| Ok("Rust is a safe and fast programming language.".to_string()));

        let response = mock
            .generate("Write a short sentence about Rust programming.")
            .await
            .unwrap();
        assert!(!response.is_empty());
    }

    #[tokio::test]
    async fn test_qa_pair_generation() {
        let mut mock = MockLLMClient::new();

        mock.expect_generate_qa_pair().times(1).returning(|_| {
            Ok((
                "What is Rust's main focus?".to_string(),
                "Rust focuses on safety, concurrency, and performance.".to_string(),
            ))
        });

        let context = "Rust is a systems programming language focused on safety, concurrency, and performance.";
        let (question, answer) = mock.generate_qa_pair(context).await.unwrap();

        assert!(!question.is_empty());
        assert!(!answer.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_qa_pairs() {
        let mut mock = MockLLMClient::new();

        mock.expect_generate_qa_pairs()
            .times(1)
            .returning(|_, _count| {
                Ok(vec![
                    (
                        "What is Rust?".to_string(),
                        "Rust is a systems programming language.".to_string(),
                    ),
                    (
                        "What are Rust's key features?".to_string(),
                        "Safety, concurrency, and performance.".to_string(),
                    ),
                ])
            });

        let context = "Rust is a systems programming language focused on safety, concurrency, and performance.";
        let pairs = mock.generate_qa_pairs(context, 2).await.unwrap();

        assert_eq!(pairs.len(), 2);
        assert!(!pairs[0].0.is_empty());
        assert!(!pairs[0].1.is_empty());
        assert!(!pairs[1].0.is_empty());
        assert!(!pairs[1].1.is_empty());
    }
}
