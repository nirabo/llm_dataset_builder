use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use regex::Regex;
use async_trait::async_trait;

#[derive(Debug, Deserialize, Serialize)]
pub struct ProcessedItem {
    pub question: String,
    pub answer: String,
}

#[async_trait]
pub trait OllamaClient: Send + Sync {
    async fn generate_questions(&self, content: &str, target_count: usize) -> Result<Vec<ProcessedItem>>;
}

pub struct DefaultOllamaClient {
    endpoint: String,
    model: String,
    client: Client,
}

impl DefaultOllamaClient {
    pub fn new(endpoint: String, model: String) -> Self {
        Self {
            endpoint,
            model,
            client: Client::new(),
        }
    }

    fn sanitize_json(json: &str) -> String {
        // First strip any markdown code blocks
        let json = if let Some(content) = json.strip_prefix("```json") {
            if let Some(content) = content.strip_suffix("```") {
                content.trim()
            } else {
                json
            }
        } else {
            json
        };

        // First try to fix any truncated JSON by finding the last complete object
        let truncated_fix = if !json.trim_end().ends_with('}') {
            if let Some(last_complete) = json.rfind(r#","answer":"#) {
                // Find the last complete question-answer pair
                if let Some(last_question) = json[..last_complete].rfind(r#"{"question":"#) {
                    let mut result = String::from(&json[..last_question]);
                    result.push_str("]}}}");
                    result
                } else {
                    let mut result = String::from(&json[..last_complete]);
                    result.push_str("}]}}}");
                    result
                }
            } else if let Some(last_complete) = json.rfind("}}") {
                let mut result = String::from(&json[..=last_complete]);
                result.push('}');
                result
            } else {
                json.to_string()
            }
        } else {
            json.to_string()
        };

        // Remove any trailing commas in arrays
        let re = Regex::new(r",(\s*[\]}])").unwrap();
        let json = re.replace_all(&truncated_fix, "$1").to_string();
        
        // Remove newlines and extra whitespace between JSON elements
        let re = Regex::new(r"\s*\n\s*").unwrap();
        let json = re.replace_all(&json, " ").to_string();

        // Fix Windows paths while preserving escaped quotes
        let mut result = String::with_capacity(json.len());
        let mut chars = json.chars().peekable();
        
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    if next == '"' {
                        // Keep escaped quotes as-is
                        result.push('\\');
                        result.push('"');
                        chars.next(); // consume the quote
                    } else {
                        // Convert other backslashes to forward slashes
                        result.push('/');
                    }
                } else {
                    result.push('/');
                }
            } else {
                result.push(c);
            }
        }
        
        result
    }
}

#[async_trait]
impl OllamaClient for DefaultOllamaClient {
    async fn generate_questions(&self, content: &str, target_count: usize) -> Result<Vec<ProcessedItem>> {
        const MAX_RETRIES: usize = 3;
        let mut retries = 0;

        while retries < MAX_RETRIES {
            let prompt_text = if content.contains("# Release Notes") || content.contains("# Changelog") {
                format!(
                    "Generate exactly {} unique questions and answers from these release notes. \
                     Focus on specific changes, features, and improvements. \
                     Format as JSON array with 'question' and 'answer' fields. \
                     Questions should be detailed and specific to the version mentioned in the notes.",
                    target_count
                )
            } else {
                format!(
                    "Generate exactly {} unique questions and answers from this documentation. \
                     Focus on key concepts, features, and usage. \
                     Format as JSON array with 'question' and 'answer' fields.",
                    target_count
                )
            };

            let (system_msg, user_msg) = if content.contains("# Release Notes") || content.contains("# Changelog") {
                (
                    "You are a helpful assistant that generates questions and answers about software release notes. \
                     Format your response as JSON. Keep answers concise and factual. \
                     Focus on the specific changes and improvements in this version.",
                    format!("{}\nContent: {}", prompt_text, content)
                )
            } else {
                (
                    "You are a helpful assistant that generates questions and answers about technical documentation. \
                     Format your response as JSON. Keep answers concise and factual. \
                     Focus on the technical details and functionality being described.",
                    format!("{}\nContent: {}", prompt_text, content)
                )
            };

            println!("Requesting {} questions from Ollama...", target_count);
            let response = self.client
                .post(&format!("{}/api/chat", self.endpoint))
                .json(&serde_json::json!({
                    "model": &self.model,
                    "messages": [
                        {
                            "role": "system",
                            "content": system_msg
                        },
                        {
                            "role": "user",
                            "content": user_msg
                        }
                    ],
                    "stream": false, 
                    "format": {
                        "type": "object", 
                        "required": ["questions"],
                        "properties": {
                            "questions": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "required": ["question", "answer"],
                                    "properties": {
                                        "question": {
                                            "type": "string"
                                        },
                                        "answer": {
                                            "type": "string"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }))
                .send()
                .await?;

            if !response.status().is_success() {
                let error_text = response.text().await?;
                println!("Ollama API error: {}", error_text);
                return Err(anyhow!("Ollama API error: {}", error_text));
            }

            let response_text = response.text().await?;
            println!("Received response from Ollama");
            
            #[derive(Debug, Deserialize)]
            struct ChatMessage {
                content: String,
            }
            
            #[derive(Debug, Deserialize)]
            struct ChatResponse {
                message: ChatMessage,
            }

            match serde_json::from_str::<ChatResponse>(&response_text) {
                Ok(chat_response) => {
                    let sanitized = Self::sanitize_json(&chat_response.message.content);

                    #[derive(Debug, Deserialize)]
                    struct QuestionResponse {
                        questions: Vec<ProcessedItem>,
                    }

                    match serde_json::from_str::<QuestionResponse>(&sanitized) {
                        Ok(parsed) => {
                            println!("Received {} questions (requested {})", parsed.questions.len(), target_count);
                            return Ok(parsed.questions);
                        }
                        Err(e) => {
                            println!("Failed to parse as JSON (attempt {}/{}): {}", retries + 1, MAX_RETRIES, e);
                            println!("Raw response: {}", response_text);
                            println!("Sanitized response: {}", sanitized);
                            retries += 1;
                            if retries == MAX_RETRIES {
                                return Err(anyhow!("Failed to parse Ollama response after {} attempts", MAX_RETRIES));
                            }
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to parse chat response (attempt {}/{}): {}", retries + 1, MAX_RETRIES, e);
                    println!("Raw response: {}", response_text);
                    retries += 1;
                    if retries == MAX_RETRIES {
                        return Err(anyhow!("Failed to parse chat response after {} attempts", MAX_RETRIES));
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }

        Err(anyhow!("Failed to process section after {} attempts", MAX_RETRIES))
    }
}

#[async_trait]
pub trait OllamaProcessor {
    async fn process_file(&self, file_path: &Path) -> Result<Vec<ProcessedItem>>;
}

pub struct DefaultOllamaProcessor {
    client: Box<dyn OllamaClient>,
}

impl DefaultOllamaProcessor {
    pub fn new(endpoint: String, model: String) -> Self {
        Self {
            client: Box::new(DefaultOllamaClient::new(endpoint, model)),
        }
    }

    #[cfg(test)]
    pub fn new_with_client(_endpoint: String, _model: String, client: Box<dyn OllamaClient>) -> Self {
        Self { client }
    }

    pub fn count_words(text: &str) -> usize {
        text.split_whitespace().count()
    }

    pub fn calculate_question_targets(word_count: usize) -> (usize, usize, usize) {
        let base_goal = (word_count as f64 / 10.0).ceil() as usize;
        let base_goal = base_goal.max(2);
        let extra_questions = (base_goal as f64 * 0.25).ceil() as usize;
        let extra_questions = extra_questions.max(2);
        let generation_target = base_goal + extra_questions;
        let min_acceptable = ((base_goal as f64 * 0.8).ceil() as usize).max(2);
        
        println!("Question targets for {} words:", word_count);
        println!("  Base goal: {} questions", base_goal);
        println!("  Generating: {} questions (+{} extra)", generation_target, extra_questions);
        println!("  Minimum acceptable: {} questions", min_acceptable);
        
        (base_goal, generation_target, min_acceptable)
    }

    fn split_into_sections(&self, content: &str) -> Vec<String> {
        let mut sections = Vec::new();
        let mut current_section = String::new();
        let header_regex = Regex::new(r"(?m)^#\s|^##\s").unwrap();
        
        if !header_regex.is_match(content.lines().next().unwrap_or("")) {
            current_section = String::new();
        }

        for line in content.lines() {
            if header_regex.is_match(line) {
                if !current_section.trim().is_empty() {
                    sections.push(current_section);
                }
                current_section = String::new();
            }
            current_section.push_str(line);
            current_section.push('\n');
        }
        
        if !current_section.trim().is_empty() {
            sections.push(current_section);
        }

        if sections.is_empty() {
            sections.push(content.to_string());
        }

        sections
    }

    fn split_by_headings(&self, content: &str) -> Vec<String> {
        let mut sections = Vec::new();
        let mut current_section = String::new();
        
        for line in content.lines() {
            if line.starts_with('#') {
                if !current_section.trim().is_empty() {
                    sections.push(current_section);
                    current_section = String::new();
                }
            }
            current_section.push_str(line);
            current_section.push('\n');
        }
        
        if !current_section.trim().is_empty() {
            sections.push(current_section);
        }
        
        if sections.is_empty() {
            sections.push(content.to_string());
        }
        
        sections
    }
    
    fn split_by_paragraphs(&self, content: &str) -> Vec<String> {
        let mut sections = Vec::new();
        let mut current_section = String::new();
        let mut empty_lines = 0;
        
        for line in content.lines() {
            if line.trim().is_empty() {
                empty_lines += 1;
                if empty_lines >= 2 && !current_section.trim().is_empty() {
                    sections.push(current_section);
                    current_section = String::new();
                    empty_lines = 0;
                }
            } else {
                empty_lines = 0;
            }
            current_section.push_str(line);
            current_section.push('\n');
        }
        
        if !current_section.trim().is_empty() {
            sections.push(current_section);
        }
        
        if sections.is_empty() {
            sections.push(content.to_string());
        }
        
        sections
    }

    async fn process_section_recursive(&self, section: &str, target_questions: usize) -> Result<Vec<ProcessedItem>> {
        let mut all_items = Vec::new();
        
        let items = self.client.generate_questions(section, target_questions).await?;
        println!("Got {} questions from full section (target: {})", items.len(), target_questions);
        
        if items.len() >= target_questions {
            return Ok(items);
        }
        
        println!("Splitting section by headings...");
        let heading_sections = self.split_by_headings(section);
        if heading_sections.len() > 1 {
            for (i, subsection) in heading_sections.iter().enumerate() {
                println!("Processing heading section {}/{}", i + 1, heading_sections.len());
                let words_ratio = Self::count_words(subsection) as f64 / Self::count_words(section) as f64;
                let subsection_target = (target_questions as f64 * words_ratio).ceil() as usize;
                println!("  Target {} questions ({:.1}% of content)", subsection_target, words_ratio * 100.0);
                
                match self.client.generate_questions(subsection, subsection_target).await {
                    Ok(mut items) => {
                        println!("  Got {} questions", items.len());
                        all_items.append(&mut items);
                    },
                    Err(e) => println!("Error processing heading section: {}", e),
                }
            }
            
            if all_items.len() >= target_questions {
                println!("Got enough questions from heading sections: {}", all_items.len());
                return Ok(all_items);
            }
        }
        
        println!("Splitting section by paragraphs...");
        all_items.clear();
        let paragraph_sections = self.split_by_paragraphs(section);
        if paragraph_sections.len() > 1 {
            for (i, subsection) in paragraph_sections.iter().enumerate() {
                println!("Processing paragraph section {}/{}", i + 1, paragraph_sections.len());
                let words_ratio = Self::count_words(subsection) as f64 / Self::count_words(section) as f64;
                let subsection_target = (target_questions as f64 * words_ratio).ceil() as usize;
                println!("  Target {} questions ({:.1}% of content)", subsection_target, words_ratio * 100.0);
                
                match self.client.generate_questions(subsection, subsection_target).await {
                    Ok(mut items) => {
                        println!("  Got {} questions", items.len());
                        all_items.append(&mut items);
                    },
                    Err(e) => println!("Error processing paragraph section: {}", e),
                }
            }
            
            if all_items.len() >= target_questions {
                println!("Got enough questions from paragraph sections: {}", all_items.len());
                return Ok(all_items);
            }
        }
        
        println!("Could not generate enough questions. Got {} out of {}", all_items.len(), target_questions);
        Ok(all_items)
    }

    fn get_qa_path(&self, file_path: &Path, extension: &str) -> PathBuf {
        let file_stem = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        file_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!("{}_qa.{}", file_stem, extension))
    }

    fn convert_json_to_jsonl(&self, json_path: &Path, jsonl_path: &Path) -> Result<Vec<ProcessedItem>> {
        println!("Converting {:?} to JSONL format at {:?}", json_path, jsonl_path);
        let content = fs::read_to_string(json_path)?;
        let items: Vec<ProcessedItem> = serde_json::from_str(&content)?;
        
        let mut output = String::new();
        for item in &items {
            if let Ok(json_line) = serde_json::to_string(item) {
                output.push_str(&json_line);
                output.push('\n');
            }
        }
        fs::write(jsonl_path, output)?;
        Ok(items)
    }

    fn check_existing_qa(&self, file_path: &Path, _required_questions: usize) -> Result<Option<Vec<ProcessedItem>>> {
        let jsonl_path = self.get_qa_path(file_path, "jsonl");
        
        if jsonl_path.exists() {
            println!("Found existing JSONL file: {:?}", jsonl_path);
            if let Ok(content) = fs::read_to_string(&jsonl_path) {
                let mut items = Vec::new();
                for line in content.lines() {
                    if let Ok(item) = serde_json::from_str::<ProcessedItem>(line) {
                        items.push(item);
                    }
                }
                if !items.is_empty() {
                    let content = fs::read_to_string(file_path)?;
                    let word_count = Self::count_words(&content);
                    let (_, _, min_acceptable) = Self::calculate_question_targets(word_count);
                    
                    if items.len() >= min_acceptable {
                        println!("Found existing JSONL file with {} questions (minimum acceptable: {}), skipping...", 
                            items.len(), min_acceptable);
                        return Ok(Some(items));
                    } else {
                        println!("Found existing JSONL file but only has {} questions (minimum needed: {}), regenerating with extra buffer...", 
                            items.len(), min_acceptable);
                    }
                } else {
                    println!("No valid items found in existing JSONL file");
                }
            }
        } else {
            let json_path = self.get_qa_path(file_path, "json");
            if json_path.exists() {
                println!("Found existing JSON file: {:?}", json_path);
                if let Ok(content) = fs::read_to_string(&json_path) {
                    if let Ok(items) = serde_json::from_str::<Vec<ProcessedItem>>(&content) {
                        let content = fs::read_to_string(file_path)?;
                        let word_count = Self::count_words(&content);
                        let (_, _, min_acceptable) = Self::calculate_question_targets(word_count);
                        
                        if items.len() >= min_acceptable {
                            println!("Found existing JSON file with {} questions (minimum acceptable: {}), converting to JSONL...", 
                                items.len(), min_acceptable);
                            match self.convert_json_to_jsonl(&json_path, &jsonl_path) {
                                Ok(items) => {
                                    println!("Successfully converted to JSONL format");
                                    return Ok(Some(items));
                                }
                                Err(e) => {
                                    println!("Failed to convert to JSONL format: {}", e);
                                }
                            }
                        } else {
                            println!("Found existing JSON file but only has {} questions (minimum needed: {}), regenerating with extra buffer...", 
                                items.len(), min_acceptable);
                        }
                    }
                }
            } else {
                println!("No existing QA file found");
            }
        }
        Ok(None)
    }
}

#[async_trait]
impl OllamaProcessor for DefaultOllamaProcessor {
    async fn process_file(&self, file_path: &Path) -> Result<Vec<ProcessedItem>> {
        let content = fs::read_to_string(file_path)?;
        let total_words = Self::count_words(&content);
        let (_, total_questions_needed, _) = Self::calculate_question_targets(total_words);

        if let Some(existing_items) = self.check_existing_qa(file_path, total_questions_needed)? {
            return Ok(existing_items);
        }

        let mut all_items = Vec::new();
        let sections = self.split_into_sections(&content);
        
        for (i, section) in sections.iter().enumerate() {
            if section.trim().is_empty() {
                continue;
            }
            
            let section_words = Self::count_words(section);
            let section_target = (total_questions_needed as f64 * 
                (section_words as f64 / total_words as f64)).ceil() as usize;
            
            println!("\nProcessing section {}/{} ({} words, target {} questions)", 
                i + 1, sections.len(), section_words, section_target);
            
            match self.process_section_recursive(section, section_target).await {
                Ok(questions) => {
                    all_items.extend(questions);
                    println!("Total questions so far: {}/{}", all_items.len(), total_questions_needed);
                }
                Err(e) => {
                    println!("Error processing section: {}", e);
                }
            }
        }

        // Always create the output file, even if empty
        let qa_path = self.get_qa_path(file_path, "jsonl");
        println!("Saving {} questions to {:?}", all_items.len(), qa_path);
        
        let mut file = fs::File::create(&qa_path)?;
        for item in &all_items {
            writeln!(file, "{}", serde_json::to_string(item)?)?;
        }

        Ok(all_items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use mockall::predicate;

    mock! {
        pub OllamaClient {}

        #[async_trait]
        impl OllamaClient for OllamaClient {
            async fn generate_questions(&self, content: &str, target_count: usize) -> Result<Vec<ProcessedItem>>;
        }
    }

    // Mock OllamaProcessor to override check_existing_qa
    struct TestOllamaProcessor {
        client: Box<dyn OllamaClient>,
    }

    impl TestOllamaProcessor {
        fn new(client: Box<dyn OllamaClient>) -> Self {
            Self { client }
        }

        async fn process_section_recursive(&self, section: &str, target_questions: usize) -> Result<Vec<ProcessedItem>> {
            self.client.generate_questions(section, target_questions).await
        }

        fn split_into_sections(&self, content: &str) -> Vec<String> {
            vec![content.to_string()]
        }

        fn get_qa_path(&self, file_path: &Path, extension: &str) -> PathBuf {
            let file_stem = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            file_path
                .parent()
                .unwrap_or(Path::new("."))
                .join(format!("{}_qa.{}", file_stem, extension))
        }
    }

    #[async_trait]
    impl OllamaProcessor for TestOllamaProcessor {
        async fn process_file(&self, file_path: &Path) -> Result<Vec<ProcessedItem>> {
            let content = fs::read_to_string(file_path)?;
            let total_words = DefaultOllamaProcessor::count_words(&content);
            let (_, total_questions_needed, _) = DefaultOllamaProcessor::calculate_question_targets(total_words);

            // Skip checking existing QA files in tests
            let sections = self.split_into_sections(&content);
            let mut all_items = Vec::new();

            for section in sections {
                match self.process_section_recursive(&section, total_questions_needed).await {
                    Ok(questions) => {
                        all_items.extend(questions);
                    }
                    Err(e) => {
                        println!("Error processing section: {}", e);
                        return Err(e);
                    }
                }
            }

            // Always create the output file, even if empty
            let qa_path = self.get_qa_path(file_path, "jsonl");
            println!("Saving {} questions to {:?}", all_items.len(), qa_path);
            
            let mut file = fs::File::create(&qa_path)?;
            for item in &all_items {
                writeln!(file, "{}", serde_json::to_string(item)?)?;
            }

            Ok(all_items)
        }
    }

    #[tokio::test]
    async fn test_process_file_success() {
        let mut mock_client = MockOllamaClient::new();
        mock_client
            .expect_generate_questions()
            .with(predicate::function(|content: &str| content.trim() == "test content"), predicate::eq(4))
            .times(1)
            .returning(|_, _| Ok(vec![
                ProcessedItem {
                    question: "Q1".to_string(),
                    answer: "A1".to_string(),
                },
                ProcessedItem {
                    question: "Q2".to_string(),
                    answer: "A2".to_string(),
                },
            ]));

        let processor = TestOllamaProcessor::new(Box::new(mock_client));

        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.md");
        fs::write(&test_file, "test content").unwrap();
        
        let result = processor.process_file(&test_file).await;
        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].question, "Q1");
        assert_eq!(items[0].answer, "A1");
    }

    #[tokio::test]
    async fn test_process_file_empty() {
        let mut mock_client = MockOllamaClient::new();
        mock_client
            .expect_generate_questions()
            .with(predicate::function(|content: &str| content.trim().is_empty()), predicate::eq(4))
            .times(1)
            .returning(|_, _| Ok(vec![]));

        let processor = TestOllamaProcessor::new(Box::new(mock_client));

        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("empty.md");
        fs::write(&test_file, "").unwrap();
        
        let result = processor.process_file(&test_file).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_process_file_error() {
        let mut mock_client = MockOllamaClient::new();
        mock_client
            .expect_generate_questions()
            .with(predicate::function(|content: &str| content.trim() == "test content"), predicate::eq(4))
            .times(1)
            .returning(|_, _| Err(anyhow!("API Error")));

        let processor = TestOllamaProcessor::new(Box::new(mock_client));

        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.md");
        fs::write(&test_file, "test content").unwrap();
        
        let result = processor.process_file(&test_file).await;
        assert!(result.is_err());
    }
}
