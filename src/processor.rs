use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::ProcessedItem;

pub struct OllamaProcessor {
    client: Client,
    endpoint: String,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    system: String,
    format: String,
    stream: bool,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

#[derive(Debug, Deserialize)]
struct QuestionWrapper {
    questions: Vec<ProcessedItem>,
}

#[derive(Debug, Deserialize)]
struct OllamaError {
    error: OllamaErrorDetails,
}

#[derive(Debug, Deserialize)]
struct OllamaErrorDetails {
    code: u32,
    message: String,
    documentation: Option<String>,
}

impl OllamaProcessor {
    pub fn new(endpoint: String) -> Self {
        Self {
            client: Client::new(),
            endpoint,
        }
    }

    fn sanitize_json(json: &str) -> String {
        // Remove any trailing commas in arrays
        let re = regex::Regex::new(r",(\s*[\]}])").unwrap();
        let json = re.replace_all(json, "$1").to_string();
        
        // Remove newlines and extra whitespace between JSON elements
        let re = regex::Regex::new(r"\s*\n\s*").unwrap();
        let json = re.replace_all(&json, " ").to_string();
        
        // Ensure all JSON special characters in strings are properly escaped
        let re = regex::Regex::new(r#"([^\\])"#).unwrap();
        let json = re.replace_all(&json, r#"\$1"#).to_string();
        
        // Remove any null bytes or invalid UTF-8
        json.chars()
            .filter(|c| !c.is_control() && *c != '\0')
            .collect()
    }
    
    pub async fn process_file(&self, file_path: &Path) -> Result<Vec<ProcessedItem>> {
        let content = std::fs::read_to_string(file_path)?;
        println!("Processing file: {:?}", file_path);
        
        let system_prompt = "You are a helpful assistant that generates question-answer pairs from the given text. \
            Generate 20 relevant questions and their corresponding answers based on the content. \
            You MUST respond with a valid JSON object in a single line. Do not include any newlines or extra whitespace. \
            IMPORTANT: Do not use raw JSON special characters in the text. Escape all quotes, braces, and special characters. \
            The response must be in this exact format (with your own questions and answers):\n\
            {\"questions\":[{\"question\":\"What is X?\",\"answer\":\"X is...\"},{\"question\":\"How does Y work?\",\"answer\":\"Y works by...\"}]}";
            
        let request = OllamaRequest {
            model: "exaone35max".to_string(),
            prompt: content,
            system: system_prompt.to_string(),
            format: "json".to_string(),
            stream: false,
            temperature: 0.0,
        };
        
        println!("Sending request to Ollama...");
        let response = self.client
            .post(format!("{}/api/generate", self.endpoint))
            .json(&request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let error_text = response.text().await?;
            if let Ok(error) = serde_json::from_str::<OllamaError>(&error_text) {
                return Err(anyhow!(
                    "Ollama API error: {} (code: {}). Documentation: {}", 
                    error.error.message,
                    error.error.code,
                    error.error.documentation.unwrap_or_default()
                ));
            } else {
                return Err(anyhow!("Ollama API error: {}", error_text));
            }
        }
            
        let response: OllamaResponse = response.json().await?;
        println!("Received response from Ollama");
        
        // Sanitize the JSON response
        let sanitized_response = Self::sanitize_json(&response.response);
        println!("Sanitized response: {}", sanitized_response);
        
        // Try to parse as wrapped questions first
        match serde_json::from_str::<QuestionWrapper>(&sanitized_response) {
            Ok(wrapper) => {
                println!("Successfully parsed {} question-answer pairs", wrapper.questions.len());
                Ok(wrapper.questions)
            }
            Err(wrapper_err) => {
                println!("Failed to parse as wrapped questions: {}", wrapper_err);
                
                // Try to parse as direct array
                match serde_json::from_str::<Vec<ProcessedItem>>(&sanitized_response) {
                    Ok(items) => {
                        println!("Successfully parsed {} question-answer pairs", items.len());
                        Ok(items)
                    }
                    Err(array_err) => {
                        println!("Failed to parse as array: {}", array_err);
                        
                        // Try to parse as single item
                        match serde_json::from_str::<ProcessedItem>(&sanitized_response) {
                            Ok(item) => {
                                println!("Successfully parsed single question-answer pair");
                                Ok(vec![item])
                            }
                            Err(item_err) => {
                                println!("Failed to parse as single item: {}", item_err);
                                println!("Raw response: {}", response.response);
                                Err(anyhow!("Failed to parse Ollama response: wrapper error: {}, array error: {}, item error: {}", 
                                    wrapper_err, array_err, item_err))
                            }
                        }
                    }
                }
            }
        }
    }
}
