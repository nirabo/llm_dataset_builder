use anyhow::Result;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};
use std::path::Path;

use crate::graph::{node::NodeType, DocumentGraph, DocumentNode};

/// Parse a markdown file into a document graph
pub fn parse_markdown_file(path: &Path) -> Result<DocumentGraph> {
    let content = std::fs::read_to_string(path)?;
    parse_markdown(&content)
}

/// Parse markdown content into a document graph
pub fn parse_markdown(content: &str) -> Result<DocumentGraph> {
    let mut graph = DocumentGraph::new();
    let mut current_section: Option<DocumentNode> = None;
    let mut current_code_block: Option<DocumentNode> = None;
    let mut list_stack: Vec<DocumentNode> = Vec::new();
    // Initialize parser with all extensions enabled
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(content, options);
    let mut current_text = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading(level, ..)) => {
                // Create a new section node
                if !current_text.is_empty() {
                    let text_node = DocumentNode::new(
                        NodeType::Text,
                        current_text.clone(),
                        None,
                        None,
                        0,
                        vec![],
                    );
                    graph.add_node(text_node);
                    current_text.clear();
                }

                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };

                current_section = Some(DocumentNode::new(
                    NodeType::Section,
                    String::new(),
                    None,
                    Some(level),
                    0,
                    vec![],
                ));
            }
            Event::End(Tag::Heading(..)) => {
                if let Some(mut section) = current_section.take() {
                    section.content = current_text.clone();
                    graph.add_node(section);
                    current_text.clear();
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                current_code_block = Some(DocumentNode::new(
                    NodeType::Code,
                    String::new(),
                    None,
                    None,
                    0,
                    match kind {
                        CodeBlockKind::Fenced(lang) => {
                            let lang_str = lang.to_string();
                            if !lang_str.is_empty() {
                                vec![format!("language:{}", lang_str)]
                            } else {
                                vec![]
                            }
                        }
                        CodeBlockKind::Indented => vec!["indented".to_string()],
                    },
                ));
            }
            Event::End(Tag::CodeBlock(_)) => {
                if let Some(mut code_block) = current_code_block.take() {
                    code_block.content = current_text.trim().to_string();
                    graph.add_node(code_block);
                    current_text.clear();
                }
            }
            Event::Start(Tag::List(ordered)) => {
                let list_node = DocumentNode::new(
                    NodeType::List,
                    String::new(),
                    None,
                    None,
                    0,
                    if ordered.is_some() {
                        vec!["ordered".to_string()]
                    } else {
                        vec!["unordered".to_string()]
                    },
                );
                list_stack.push(list_node);
            }

            Event::Start(Tag::Item) => {
                if let Some(list_node) = list_stack.last_mut() {
                    let item_node = DocumentNode::new(
                        NodeType::ListItem,
                        String::new(),
                        Some(list_node.id.to_string()),
                        None,
                        0,
                        vec![],
                    );
                    list_stack.push(item_node);
                }
            }

            Event::End(Tag::Item) => {
                if let Some(mut item_node) = list_stack.pop() {
                    if let Some(parent_node) = list_stack.last_mut() {
                        item_node.content = current_text.trim().to_string();
                        graph.add_edge(
                            &parent_node.id.to_string(),
                            &item_node.id.to_string(),
                            "contains".to_string(),
                        );
                        graph.add_node(item_node);
                        current_text.clear();
                    }
                }
            }

            Event::End(Tag::List(_)) => {
                if let Some(list_node) = list_stack.pop() {
                    if let Some(parent_node) = list_stack.last_mut() {
                        graph.add_edge(
                            &parent_node.id.to_string(),
                            &list_node.id.to_string(),
                            "contains".to_string(),
                        );
                    }
                    graph.add_node(list_node);
                }
            }
            Event::Text(text) => {
                // Accumulate text content
                current_text.push_str(&text);
            }
            Event::Code(code) => {
                // Handle inline code blocks
                current_text.push('`');
                current_text.push_str(&code);
                current_text.push('`');
            }
            Event::SoftBreak | Event::HardBreak => {
                // Handle line breaks
                current_text.push('\n');
            }
            _ => {}
        }
    }

    // Handle any remaining text
    if !current_text.is_empty() {
        let text_node = DocumentNode::new(NodeType::Text, current_text, None, None, 0, vec![]);
        graph.add_node(text_node);
    }

    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_basic() {
        let markdown = r#"# Title
This is a paragraph.

## Section 1
Some text.

```rust
fn main() {
    println!("Hello, world!");
}
```

### Subsection
- List item 1
- List item 2
"#;

        let graph = parse_markdown(markdown).unwrap();

        // Check if we have the correct number of nodes
        let sections = graph.get_nodes_by_type(NodeType::Section);
        let code_blocks = graph.get_nodes_by_type(NodeType::Code);
        let lists = graph.get_nodes_by_type(NodeType::List);
        let texts = graph.get_nodes_by_type(NodeType::Text);

        assert_eq!(sections.len(), 3); // Title, Section 1, Subsection
        assert_eq!(code_blocks.len(), 1); // Rust code block
        assert_eq!(lists.len(), 1); // One list
        assert!(texts.len() > 0); // At least one text node
    }
}
