use clap::Parser;
use dotenv::dotenv;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use llm_dataset_builder::datasource::{
    DataSource, GitHubReleaseSource, GitHubSource, LocalSource, UrlSource,
};
use llm_dataset_builder::processor::{
    DefaultOllamaClient, DefaultOllamaProcessor, OllamaProcessor,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output directory for collected data
    #[arg(short = 'd', long)]
    output_dir: Option<String>,

    /// Ollama API endpoint
    #[arg(short = 'e', long)]
    ollama_endpoint: Option<String>,

    /// Ollama model to use
    #[arg(short = 'm', long)]
    model: Option<String>,

    /// Test mode (skips interactive input)
    #[arg(long, hide = true)]
    test_mode: bool,
}

async fn collect_sources() -> Result<Vec<Box<dyn DataSource>>, Box<dyn std::error::Error>> {
    let mut sources: Vec<Box<dyn DataSource>> = Vec::new();
    let mut buffer = String::new();

    loop {
        println!("\nEnter a data source (press Enter to finish):");
        println!("- URL (e.g., https://example.com/file.txt)");
        println!("- Local path (e.g., /path/to/file)");
        println!("- GitHub URL (e.g., https://github.com/user/repo/tree/branch/path)");
        println!("- GitHub releases URL (e.g., https://github.com/user/repo/releases)");
        print!("> ");
        std::io::stdout().flush()?;

        buffer.clear();
        std::io::stdin().read_line(&mut buffer)?;
        let input = buffer.trim();

        if input.is_empty() {
            break;
        }

        // Check if it's a GitHub releases URL
        if input.contains("/releases") {
            println!("Processing GitHub releases: {}", input);
            match GitHubReleaseSource::new(input) {
                Ok(source) => {
                    sources.push(Box::new(source) as Box<dyn DataSource>);
                    println!("Successfully added GitHub releases source: {}", input);
                }
                Err(e) => println!("Error adding GitHub releases source: {}", e),
            }
            continue;
        }

        // Check if it's a GitHub URL
        if input.starts_with("https://github.com/")
            && (input.contains("/tree/") || input.contains("/blob/"))
        {
            println!("Processing GitHub source: {}", input);
            sources.push(Box::new(GitHubSource::new(input, None, None)) as Box<dyn DataSource>);
            println!("Successfully added GitHub source: {}", input);
            continue;
        }

        // Check if it's a regular URL
        if input.starts_with("http://") || input.starts_with("https://") {
            println!("Processing URL source: {}", input);
            match UrlSource::new(input) {
                Ok(source) => {
                    sources.push(Box::new(source) as Box<dyn DataSource>);
                    println!("Successfully added URL source: {}", input);
                }
                Err(e) => println!("Error adding URL source: {}", e),
            }
            continue;
        }

        // Assume it's a local path if it doesn't match the above
        if Path::new(input).exists() {
            println!("Processing local source: {}", input);
            sources.push(Box::new(LocalSource::new(input)) as Box<dyn DataSource>);
            println!("Successfully added local source: {}", input);
        } else {
            println!("Invalid input. Please enter:");
            println!("- A GitHub URL (https://github.com/user/repo/tree/branch/path)");
            println!("- A GitHub releases URL (https://github.com/user/repo/releases)");
            println!("- A regular URL (http:// or https://)");
            println!("- A valid local file or directory path");
        }
    }

    Ok(sources)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenv().ok();

    let args = Args::parse();

    // Use command line args if provided, otherwise fall back to env vars, then defaults
    let output_dir = args
        .output_dir
        .or_else(|| env::var("OUTPUT_DIR").ok())
        .unwrap_or_else(|| "output".to_string());

    let ollama_endpoint = args
        .ollama_endpoint
        .or_else(|| env::var("OLLAMA_ENDPOINT").ok())
        .unwrap_or_else(|| "http://localhost:11434".to_string());

    let model = args
        .model
        .or_else(|| env::var("OLLAMA_MODEL").ok())
        .unwrap_or_else(|| "m/qwen2514bmax".to_string());

    // Create output directory if it doesn't exist
    fs::create_dir_all(&output_dir)?;

    // Initialize processor
    let processor = DefaultOllamaProcessor::new_with_client(
        ollama_endpoint.clone(),
        model.clone(),
        Box::new(DefaultOllamaClient::new(ollama_endpoint, model)),
        Some(PathBuf::from(&output_dir)),
    );

    // Collect data sources
    let sources = if args.test_mode {
        Vec::new()
    } else {
        collect_sources().await?
    };

    // Process each source
    let mut all_items = Vec::new();

    // If no sources added, check existing files
    if sources.is_empty() {
        println!("No new sources added. Processing existing files in output directory...");
        let mut existing_files = Vec::new();
        for entry in WalkDir::new(Path::new(&output_dir))
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "md" || ext == "txt")
                    .unwrap_or(false)
            })
        {
            existing_files.push(entry.path().to_path_buf());
        }

        if existing_files.is_empty() {
            println!("No markdown or text files found in output directory to process.");
            return Ok(());
        }

        println!(
            "Found {} markdown/text files to process.",
            existing_files.len()
        );
        for file_path in existing_files {
            println!("Processing file: {:?}", file_path);
            match processor.process_file(&file_path).await {
                Ok(items) => {
                    all_items.extend(items);
                }
                Err(e) => {
                    eprintln!("Error processing file {:?}: {}", file_path, e);
                }
            }
        }
    } else {
        // Process new sources
        for source in sources {
            println!("\nProcessing source...");

            // Collect files from source
            let files = source.collect(Path::new(&output_dir)).await?;
            println!("Found {} files", files.len());

            for file_path in files {
                println!("Processing file: {:?}", file_path);
                match processor.process_file(&file_path).await {
                    Ok(items) => {
                        all_items.extend(items);
                    }
                    Err(e) => {
                        eprintln!("Error processing file {:?}: {}", file_path, e);
                    }
                }
            }
        }
    }

    // Save combined results
    let output_file = Path::new(&output_dir).join("all_qa.jsonl");
    let mut output = String::new();
    for item in &all_items {
        if let Ok(json_line) = serde_json::to_string(item) {
            output.push_str(&json_line);
            output.push('\n');
        }
    }
    fs::write(&output_file, output)?;
    println!(
        "Saved {} question-answer pairs to {:?}",
        all_items.len(),
        output_file
    );
    println!("Individual file results saved as [filename]_qa.jsonl in the output directory");

    Ok(())
}
