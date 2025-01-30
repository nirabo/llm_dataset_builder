use anyhow::{anyhow, Result};
use async_trait::async_trait;
use llm_dataset_builder::processor::{
    DefaultOllamaProcessor, OllamaClient, OllamaProcessor, ProcessedItem,
};
use mockall::mock;
use mockall::predicate;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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

    async fn process_section_recursive(
        &self,
        section: &str,
        target_questions: usize,
    ) -> Result<Vec<ProcessedItem>> {
        self.client
            .generate_questions(section, target_questions)
            .await
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
        let (_, total_questions_needed, _) =
            DefaultOllamaProcessor::calculate_question_targets(total_words);

        // Skip checking existing QA files in tests
        let sections = self.split_into_sections(&content);
        let mut all_items = Vec::new();

        for section in sections {
            match self
                .process_section_recursive(&section, total_questions_needed)
                .await
            {
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
        .with(
            predicate::function(|content: &str| content.contains("Test Document")),
            predicate::eq(8),
        )
        .times(1)
        .returning(|_, _| {
            Ok(vec![
                ProcessedItem {
                    question: "Q1".to_string(),
                    answer: "A1".to_string(),
                },
                ProcessedItem {
                    question: "Q2".to_string(),
                    answer: "A2".to_string(),
                },
            ])
        });

    let processor = TestOllamaProcessor::new(Box::new(mock_client));
    let test_file = Path::new("tests/data/test.md");

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
        .with(
            predicate::function(|content: &str| content.trim().is_empty()),
            predicate::eq(4),
        )
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
        .with(
            predicate::function(|content: &str| content.contains("Test Document")),
            predicate::eq(8),
        )
        .times(1)
        .returning(|_, _| Err(anyhow!("API Error")));

    let processor = TestOllamaProcessor::new(Box::new(mock_client));
    let test_file = Path::new("tests/data/test.md");

    let result = processor.process_file(&test_file).await;
    assert!(result.is_err());
}
