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
        // First try to fix any truncated JSON by finding the last complete object
        let truncated_fix = if !json.trim_end().ends_with('}') {
            if let Some(last_complete) = json.rfind(r#","answer":"#) {
                // Find the last complete question-answer pair
                if let Some(last_question) = json[..last_complete].rfind(r#"{"question":"#) {
                    format!("{}]}}", &json[..last_question])
                } else {
                    format!("{}}]}}", &json[..last_complete])
                }
            } else if let Some(last_complete) = json.rfind("}}") {
                format!("{}}}", &json[..=last_complete])
            } else {
                json.to_string()
            }
        } else {
            json.to_string()
        };

        // Remove any trailing commas in arrays
        let re = regex::Regex::new(r",(\s*[\]}])").unwrap();
        let json = re.replace_all(&truncated_fix, "$1").to_string();
        
        // Remove newlines and extra whitespace between JSON elements
        let re = regex::Regex::new(r"\s*\n\s*").unwrap();
        let json = re.replace_all(&json, " ").to_string();

        // Fix Windows paths by replacing backslashes with forward slashes
        let re = regex::Regex::new(r"\\+").unwrap();
        let json = re.replace_all(&json, "/").to_string();
        
        json
    }
    
    pub async fn process_file(&self, file_path: &Path) -> Result<Vec<ProcessedItem>> {
        let content = std::fs::read_to_string(file_path)?;
        println!("Processing file: {:?}", file_path);
        
        // Extract version from filename
        let version = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown version");
        
        // Add version context to the content
        let content_with_version = format!("Version: {}\n\nChangelog:\n{}", version, content);
        
        let system_prompt = format!("You are a helpful assistant that generates question-answer pairs from the given text. \
            Generate 20 relevant questions and their corresponding answers based on the content. \
            This is about version {}. IMPORTANT: Include the version number in EVERY question when referring to features, changes, or updates. \
            You MUST respond with a valid JSON object in a single line. Do not include any newlines or extra whitespace. \
            IMPORTANT: Follow these rules for JSON safety:\n\
            1. Use single backslash for escaping: '\\\\' not multiple backslashes\n\
            2. For Windows paths, use forward slashes instead: 'C:/Users' not 'C:\\\\Users'\n\
            3. Escape quotes with a single backslash: '\\\"' not '\\\\\"'\n\
            4. Keep answers concise to avoid truncation\n\
            The response must be in this exact format (with your own questions and answers):\n\
            {{\"questions\":[{{\"question\":\"What was fixed in {}?\",\"answer\":\"In {} the following was fixed...\"}}]}}", 
            version, version, version);
            
        let request = OllamaRequest {
            model: "exaone35max".to_string(),
            prompt: content_with_version,
            system: system_prompt,
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
                        if !items.is_empty() {
                            println!("Successfully parsed {} question-answer pairs", items.len());
                            return Ok(items);
                        }
                    }
                }
            }
            Err(e) => println!("Failed to parse as raw JSON: {}", e),
        }
        
        println!("Raw response: {}", response.response);
        Err(anyhow!("Failed to parse Ollama response"))
    }
}
