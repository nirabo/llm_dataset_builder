# LLM Dataset Builder

A Rust application that automatically generates high-quality question-answer pairs from documentation, making it perfect for training Large Language Models (LLMs). The application uses Ollama with the Qwen v2.5 14B model to process various data sources and creates targeted questions based on content length and complexity.

## Features

### Smart Question Generation
- Automatically calculates the optimal number of questions based on content length
- Base target: 1 question per 10 words of content
- Adds 25% extra questions (minimum 2) to ensure quality coverage
- Example:
  ```
  100 words → 10 base questions + 3 extra = 13 questions
  20 words → 2 base questions + 2 extra = 4 questions
  ```

### Recursive Content Processing
If the initial question generation doesn't meet the target:
1. First attempts to process the entire section
2. If insufficient questions, splits content by headings
3. If still insufficient, splits content by paragraphs
4. Each subsection gets a proportional number of questions based on its word count

### Intelligent File Handling
- Outputs in JSONL format (one JSON object per line)
- Checks for existing question files before processing
- Converts older JSON files to JSONL format automatically
- Skips processing if sufficient questions already exist
- Maintains quality by ensuring minimum question thresholds

### Multiple Data Source Support
- Local files
- URLs (web pages)
- GitHub repositories
- GitHub release notes
- Handles both Markdown and plain text content

## Installation

### Option 1: Download Pre-built Binary (Recommended)
1. Go to the [Releases page](https://github.com/technovangelist/llm_dataset_builder/releases)
2. Download the latest binary for your platform:
   - `llm_dataset_builder-macos` for macOS
   - `llm_dataset_builder-linux` for Linux
3. Make the binary executable:
   ```bash
   chmod +x llm_dataset_builder-*
   ```

### Option 2: Build from Source
If you want to build from source:
1. Ensure you have Rust installed
2. Clone this repository
3. Build the project:
   ```bash
   cargo build --release
   ```

### Option 3: Development Setup
For development, you'll want to set up pre-commit hooks to ensure code quality:

1. Install pre-commit:
   ```bash
   pip install pre-commit
   ```

2. Install the git hooks:
   ```bash
   pre-commit install
   ```

This will set up the following checks to run before each commit:
- Trailing whitespace removal
- End of file fixing
- YAML validation
- Large file checks
- Rust formatting
- Cargo check
- Clippy lints

## Configuration

The application can be configured using environment variables or command line arguments. Command line arguments take precedence over environment variables.

### Environment Variables
Copy the `.env.example` file to `.env` and customize the values:
```bash
cp .env.example .env
```

Available environment variables:
- `OLLAMA_ENDPOINT`: Ollama API endpoint (default: "http://localhost:11434")
- `OLLAMA_MODEL`: Ollama model to use (default: "m/qwen2514bmax")
- `OUTPUT_DIR`: Output directory for collected data (default: "output")

### Command Line Arguments
Command line arguments override environment variables:
```bash
cargo run -- -e http://localhost:11434 -m m/qwen2514bmax -d output
```

Options:
- `-e, --ollama-endpoint`: Ollama API endpoint
- `-m, --model`: Ollama model to use
- `-d, --output-dir`: Output directory for collected data

## Usage

### Prerequisites
- [Ollama](https://ollama.ai) installed and running locally
- Qwen v2.5 14B model installed:
  ```bash
  ollama pull m/qwen2514bmax
  ```

### Running
1. Start your Ollama server
2. Run the application:
   ```bash
   ./llm_dataset_builder-macos  # or ./llm_dataset_builder-linux
   ```
   Or if built from source:
   ```bash
   cargo run
   ```
3. Enter data sources when prompted:
   ```
   Enter a data source (press Enter to finish):
   - URL (e.g., https://example.com/file.txt)
   - Local path (e.g., /path/to/file)
   - GitHub URL (e.g., https://github.com/user/repo/tree/branch/path)
   - GitHub releases URL (e.g., https://github.com/user/repo/releases)
   ```

### Output Format
Questions are saved in JSONL format:
```jsonl
{"question":"What is the main purpose of this application?","answer":"The application automatically generates question-answer pairs from documentation for training LLMs."}
{"question":"How does it calculate the base number of questions?","answer":"It generates one question for every 10 words of content, rounded up."}
```

### Processing Logic

1. **Content Analysis**
   - Counts total words in content
   - Calculates base questions (words/10)
   - Adds 25% extra questions (min 2)
   - Sets minimum acceptable at 80% of base goal

2. **Question Generation**
   ```
   Section (100 words):
   Base goal: 10 questions
   Extra questions: max(ceil(10 * 0.25), 2) = 3
   Generation target: 13 questions
   Minimum acceptable: 8 questions
   ```

3. **Recursive Processing**
   If initial generation falls short:
   ```
   1. Try whole section first
   2. If not enough questions:
      Split into heading sections
      Each section target = total_target * (section_words / total_words)
   3. If still not enough:
      Split into paragraphs
      Each paragraph target = total_target * (paragraph_words / total_words)
   ```

## Example Output

For a documentation file with 1000 words:
```
Processing file: docs.md
Total words: 1000
Base goal: 100 questions
Generation target: 125 questions (+25 extra)
Minimum acceptable: 80 questions

Processing section 1/3 (400 words, target 50 questions)
Got 45 questions from full section
Splitting section by headings...
- Heading 1 (250 words): 31 questions
- Heading 2 (150 words): 19 questions
Total: 50 questions

Processing section 2/3 (500 words, target 63 questions)
Got 63 questions from full section

Processing section 3/3 (100 words, target 12 questions)
Got 10 questions from full section
Splitting section by paragraphs...
- Paragraph 1: 7 questions
- Paragraph 2: 5 questions
Total: 12 questions

Final result: 125 questions generated
Saved to: output/docs_qa.jsonl
```

Example dataset generated here: https://huggingface.co/datasets/technovangelist/OllamaDocs


## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. All PRs are automatically tested with:
- Unit tests
- Integration tests
- Clippy lints
- Code formatting checks

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
