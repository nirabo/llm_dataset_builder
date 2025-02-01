use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of relationship between nodes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RelationType {
    Contains,   // Hierarchical relationship
    References, // Cross-reference relationship
    Precedes,   // Sequential relationship
    Related,    // Semantic relationship
    Implements, // Implementation relationship
    Explains,   // Explanatory relationship
}

/// Represents an edge in the document graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentEdge {
    /// Source node ID
    pub from: Uuid,
    /// Target node ID
    pub to: Uuid,
    /// Type of relationship
    pub relation_type: RelationType,
    /// Optional weight/strength of the relationship
    pub weight: Option<f32>,
}

impl DocumentEdge {
    /// Create a new document edge
    pub fn new(from: Uuid, to: Uuid, relation_type: RelationType) -> Self {
        Self {
            from,
            to,
            relation_type,
            weight: None,
        }
    }

    /// Create a new weighted document edge
    pub fn with_weight(from: Uuid, to: Uuid, relation_type: RelationType, weight: f32) -> Self {
        Self {
            from,
            to,
            relation_type,
            weight: Some(weight),
        }
    }

    /// Set the weight of the relationship
    pub fn set_weight(&mut self, weight: f32) {
        self.weight = Some(weight);
    }

    /// Get the weight of the relationship
    pub fn weight(&self) -> Option<f32> {
        self.weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_creation() {
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let edge = DocumentEdge::new(from, to, RelationType::Contains);

        assert_eq!(edge.from, from);
        assert_eq!(edge.to, to);
        assert_eq!(edge.relation_type, RelationType::Contains);
        assert!(edge.weight.is_none());
    }

    #[test]
    fn test_weighted_edge() {
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let weight = 0.8;
        let edge = DocumentEdge::with_weight(from, to, RelationType::Related, weight);

        assert_eq!(edge.from, from);
        assert_eq!(edge.to, to);
        assert_eq!(edge.relation_type, RelationType::Related);
        assert_eq!(edge.weight, Some(weight));
    }

    #[test]
    fn test_weight_operations() {
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let mut edge = DocumentEdge::new(from, to, RelationType::References);

        assert!(edge.weight().is_none());

        edge.set_weight(0.5);
        assert_eq!(edge.weight(), Some(0.5));
    }
}
