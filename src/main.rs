use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::io::{self, Write};
use std::path::Path;
use walkdir::WalkDir;

mod datasource;
mod processor;

use datasource::{DataSource, UrlSource, LocalSource, GitHubSource, GitHubReleaseSource};
use processor::OllamaProcessor;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output directory for collected data
    #[arg(short = 'd', long, default_value = "output")]
    output_dir: String,

    /// Ollama API endpoint
    #[arg(short = 'e', long, default_value = "http://localhost:11434")]
    ollama_endpoint: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessedItem {
    #[serde(rename = "question")]
    pub question: String,
    #[serde(rename = "answer")]
    pub answer: String,
}

async fn collect_sources() -> Result<Vec<Box<dyn DataSource>>> {
    let mut sources: Vec<Box<dyn DataSource>> = Vec::new();
    let mut buffer = String::new();

    loop {
        println!("\nEnter a data source (press Enter to finish):");
        println!("- URL (e.g., https://example.com/file.txt)");
        println!("- Local path (e.g., /path/to/file)");
        println!("- GitHub URL (e.g., https://github.com/user/repo/tree/branch/path)");
        println!("- GitHub releases URL (e.g., https://github.com/user/repo/releases)");
        print!("> ");
        io::stdout().flush()?;
        
        buffer.clear();
        io::stdin().read_line(&mut buffer)?;
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
        if input.starts_with("https://github.com/") && (input.contains("/tree/") || input.contains("/blob/")) {
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
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Create output directory if it doesn't exist
    std::fs::create_dir_all(&args.output_dir)?;
    
    // Initialize the data collection process
    println!("Welcome to LLM Dataset Builder!");
    println!("Please add your data sources. Supported types:");
    println!("1. URLs");
    println!("2. Local documents");
    println!("3. GitHub repositories");
    println!("4. GitHub releases");
    
    let sources = collect_sources().await?;
    let mut collected_files = Vec::new();

    if sources.is_empty() {
        // If no sources added, check for existing files in output directory
        println!("No new sources added. Checking for existing files in output directory...");
        let output_dir = PathBuf::from(&args.output_dir);
        for entry in WalkDir::new(&output_dir)
            .min_depth(1)  // Skip the output directory itself
            .into_iter()
            .filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let path = entry.path().to_path_buf();
                    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if extension == "md" || extension == "txt" {
                        println!("Found existing file: {:?}", entry.path());
                        collected_files.push(path);
                    }
                }
        }
        
        if collected_files.is_empty() {
            println!("No files found in output directory. Exiting.");
            return Ok(());
        }
        println!("Found {} existing files to process.", collected_files.len());
    } else {
        // Collect data from all sources
        println!("\nCollecting data from sources...");
        for source in sources {
            let files = source.collect(args.output_dir.as_ref()).await?;
            collected_files.extend(files);
        }
        println!("Collected {} files.", collected_files.len());
    }
    
    // Process files with Ollama
    println!("\nProcessing files with Ollama...");
    let processor = OllamaProcessor::new(args.ollama_endpoint);
    
    let mut all_items = Vec::new();
    for file in collected_files {
        println!("\nProcessing file: {:?}", file);
        let items = processor.process_file(&file).await?;
        
        println!("\nGenerated Questions and Answers:");
        println!("--------------------------------");
        for (i, item) in items.iter().enumerate() {
            println!("\nQ{}: {}", i + 1, item.question);
            println!("A{}: {}", i + 1, item.answer);
        }
        println!("\n{} question-answer pairs generated.", items.len());
        println!("--------------------------------");
        
        all_items.extend(items);
    }
    
    // Save results
    let output_file = PathBuf::from(&args.output_dir).join("processed_data.json");
    std::fs::write(&output_file, serde_json::to_string_pretty(&all_items)?)?;
    
    println!("\nProcessing complete! Generated {} question-answer pairs.", all_items.len());
    println!("Results saved to: {:?}", output_file);
    
    Ok(())
}
