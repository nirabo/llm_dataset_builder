use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of document node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeType {
    Document,
    Section,
    Text,
    Code,
    List,
    ListItem,
    Table,
    TableRow,
    TableCell,
    Link,
    Image,
    Quote,
    Footnote,
    Subsection,
    Paragraph,
    CodeBlock,
}

/// Metadata associated with a document node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub title: Option<String>,
    pub level: Option<i32>,
    pub position: usize,
    pub tags: Vec<String>,
}

/// Represents a node in the document graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentNode {
    /// Unique identifier for the node
    pub id: Uuid,
    /// Type of the node
    pub node_type: NodeType,
    /// Actual content of the node
    pub content: String,
    /// Node metadata
    pub metadata: NodeMetadata,
    /// Vector embedding of the node content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
}

impl DocumentNode {
    /// Create a new document node
    pub fn new(
        node_type: NodeType,
        content: String,
        title: Option<String>,
        level: Option<i32>,
        position: usize,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_type,
            content,
            metadata: NodeMetadata {
                title,
                level,
                position,
                tags,
            },
            embedding: None,
        }
    }

    /// Set the vector embedding for this node
    pub fn set_embedding(&mut self, embedding: Vec<f32>) {
        self.embedding = Some(embedding);
    }

    /// Get the vector embedding if it exists
    pub fn embedding(&self) -> Option<&Vec<f32>> {
        self.embedding.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let node = DocumentNode::new(
            NodeType::Section,
            "Test content".to_string(),
            Some("Test title".to_string()),
            Some(1),
            0,
            vec!["test".to_string()],
        );

        assert_eq!(node.node_type, NodeType::Section);
        assert_eq!(node.content, "Test content");
        assert_eq!(node.metadata.title, Some("Test title".to_string()));
        assert_eq!(node.metadata.level, Some(1));
        assert_eq!(node.metadata.position, 0);
        assert_eq!(node.metadata.tags, vec!["test"]);
        assert!(node.embedding.is_none());
    }

    #[test]
    fn test_embedding_operations() {
        let mut node = DocumentNode::new(
            NodeType::Paragraph,
            "Test content".to_string(),
            None,
            None,
            0,
            vec![],
        );

        assert!(node.embedding().is_none());

        let embedding = vec![1.0, 2.0, 3.0];
        node.set_embedding(embedding.clone());

        assert_eq!(node.embedding(), Some(&embedding));
    }
}
