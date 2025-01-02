# LLM Dataset Builder

A Rust-based tool for building question-answer datasets from various sources using Ollama. This tool helps you create training datasets for Large Language Models by generating question-answer pairs from:

- GitHub repositories (markdown and text files)
- GitHub release notes
- Web URLs
- Local files and directories

## Features

- Multiple data source support:
  - GitHub repository files (markdown/text)
  - GitHub release notes
  - Web URLs
  - Local files and directories
- Automatic question-answer pair generation using Ollama
- Customizable number of questions per document (default: 20)
- JSON output format for easy integration
- Interactive command-line interface

## Prerequisites

- Rust (latest stable version)
- [Ollama](https://ollama.ai) installed and running locally
- Internet connection for fetching online resources

## Installation

1. Clone the repository:
```bash
git clone https://github.com/technovangelist/llm_dataset_builder.git
cd llm_dataset_builder
```

2. Build the project:
```bash
cargo build --release
```

## Usage

1. Start Ollama and pull your preferred model (e.g., exaone35max):
```bash
ollama pull exaone35max
```

2. Run the dataset builder:
```bash
cargo run
```

3. Enter your data sources when prompted. Supported formats:
   - GitHub repository: `https://github.com/user/repo/tree/branch/path`
   - GitHub releases: `https://github.com/user/repo/releases`
   - Web URL: `https://example.com/document.txt`
   - Local file: `/path/to/file.txt`

4. The tool will:
   - Download/collect all specified documents
   - Generate question-answer pairs using Ollama
   - Save the results to `processed_data.json`

## Output Format

The generated dataset is saved in JSON format with the following structure:
```json
[
  {
    "question": "What is X?",
    "answer": "X is..."
  },
  {
    "question": "How does Y work?",
    "answer": "Y works by..."
  }
]
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
