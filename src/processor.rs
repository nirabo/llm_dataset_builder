use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use serde_json::Value;

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
        
        json
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
            return Err(anyhow!("Ollama API error: {}", error_text));
        }
            
        let response: OllamaResponse = response.json().await?;
        println!("Received response from Ollama");
        
        // Sanitize the JSON response
        let sanitized_response = Self::sanitize_json(&response.response);
        println!("Sanitized response: {}", sanitized_response);
        
        // Try parsing as raw JSON first
        match serde_json::from_str::<Value>(&sanitized_response) {
            Ok(value) => {
                if let Some(questions) = value.get("questions") {
                    if let Ok(items) = serde_json::from_value::<Vec<ProcessedItem>>(questions.clone()) {
                        println!("Successfully parsed {} question-answer pairs", items.len());
                        return Ok(items);
                    }
                }
            }
            Err(e) => println!("Failed to parse as raw JSON: {}", e),
        }
        
        println!("Raw response: {}", response.response);
        Err(anyhow!("Failed to parse Ollama response"))
    }
}
