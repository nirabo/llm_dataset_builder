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
    
    pub async fn process_file(&self, file_path: &Path) -> Result<Vec<ProcessedItem>> {
        let content = std::fs::read_to_string(file_path)?;
        println!("Processing file: {:?}", file_path);
        
        let system_prompt = "You are a helpful assistant that generates question-answer pairs from the given text. \
            Generate 20 relevant questions and their corresponding answers based on the content. \
            You MUST respond with a valid JSON object. Do not include any additional text or formatting. \
            The response must be in this exact format (with your own questions and answers):\n\
            {\"questions\":[\
                {\"question\":\"First question here?\",\"answer\":\"First answer here.\"},\
                {\"question\":\"Second question here?\",\"answer\":\"Second answer here.\"}\
            ]}";
            
            
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
        
        // Try to parse as wrapped questions first
        match serde_json::from_str::<QuestionWrapper>(&response.response) {
            Ok(wrapper) => {
                println!("Successfully parsed {} question-answer pairs", wrapper.questions.len());
                Ok(wrapper.questions)
            }
            Err(wrapper_err) => {
                // Try to parse as direct array
                match serde_json::from_str::<Vec<ProcessedItem>>(&response.response) {
                    Ok(items) => {
                        println!("Successfully parsed {} question-answer pairs", items.len());
                        Ok(items)
                    }
                    Err(array_err) => {
                        // If array parsing fails, try parsing as single item
                        match serde_json::from_str::<ProcessedItem>(&response.response) {
                            Ok(item) => {
                                println!("Got single question-answer pair, converting to array");
                                Ok(vec![item])
                            }
                            Err(_) => {
                                println!("Error parsing response: Unable to parse as wrapped questions ({}), array ({}), or single item", wrapper_err, array_err);
                                println!("Raw response: {}", response.response);
                                Err(anyhow!("Failed to parse Ollama response"))
                            }
                        }
                    }
                }
            }
        }
    }
}
