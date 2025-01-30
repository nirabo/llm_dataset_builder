use anyhow::Result;
use petgraph::{
    graph::{DiGraph, NodeIndex},
    Direction,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::graph::{
    edge::{DocumentEdge, RelationType},
    error::GraphError,
    node::{DocumentNode, NodeType},
};

/// Represents a document as a directed graph
pub struct DocumentGraph {
    /// The underlying graph structure
    graph: DiGraph<DocumentNode, DocumentEdge>,
    /// Mapping from UUID to node index for quick lookups
    node_map: HashMap<Uuid, NodeIndex>,
}

impl Default for DocumentGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentGraph {
    /// Create a new empty document graph
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: DocumentNode) -> NodeIndex {
        let id = node.id;
        let idx = self.graph.add_node(node);
        self.node_map.insert(id, idx);
        idx
    }

    /// Add an edge between two nodes
    pub fn add_edge(&mut self, edge: DocumentEdge) -> Result<()> {
        let from_idx = self
            .node_map
            .get(&edge.from)
            .ok_or_else(|| GraphError::NodeNotFound(edge.from.to_string()))?;
        let to_idx = self
            .node_map
            .get(&edge.to)
            .ok_or_else(|| GraphError::NodeNotFound(edge.to.to_string()))?;

        self.graph.add_edge(*from_idx, *to_idx, edge);
        Ok(())
    }

    /// Get a reference to a node by its UUID
    pub fn get_node(&self, id: &Uuid) -> Option<&DocumentNode> {
        self.node_map.get(id).map(|idx| &self.graph[*idx])
    }

    /// Get a mutable reference to a node by its UUID
    pub fn get_node_mut(&mut self, id: &Uuid) -> Option<&mut DocumentNode> {
        self.node_map.get(id).map(|idx| &mut self.graph[*idx])
    }

    /// Get all nodes of a specific type
    pub fn get_nodes_by_type(&self, node_type: NodeType) -> Vec<&DocumentNode> {
        self.graph
            .node_indices()
            .filter_map(|idx| {
                let node = &self.graph[idx];
                if node.node_type == node_type {
                    Some(node)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all children of a node (nodes connected by Contains edges)
    pub fn get_children(&self, id: &Uuid) -> Result<Vec<&DocumentNode>> {
        let node_idx = self
            .node_map
            .get(id)
            .ok_or_else(|| GraphError::NodeNotFound(id.to_string()))?;

        Ok(self
            .graph
            .neighbors_directed(*node_idx, Direction::Outgoing)
            .filter_map(|idx| {
                let edge = self.graph.find_edge(*node_idx, idx)?;
                if self.graph[edge].relation_type == RelationType::Contains {
                    Some(&self.graph[idx])
                } else {
                    None
                }
            })
            .collect())
    }

    /// Get the parent of a node (node connected by incoming Contains edge)
    pub fn get_parent(&self, id: &Uuid) -> Result<Option<&DocumentNode>> {
        let node_idx = self
            .node_map
            .get(id)
            .ok_or_else(|| GraphError::NodeNotFound(id.to_string()))?;

        Ok(self
            .graph
            .neighbors_directed(*node_idx, Direction::Incoming)
            .find_map(|idx| {
                let edge = self.graph.find_edge(idx, *node_idx)?;
                if self.graph[edge].relation_type == RelationType::Contains {
                    Some(&self.graph[idx])
                } else {
                    None
                }
            }))
    }

    /// Get all related nodes (nodes connected by Related edges)
    pub fn get_related_nodes(&self, id: &Uuid) -> Result<Vec<&DocumentNode>> {
        let node_idx = self
            .node_map
            .get(id)
            .ok_or_else(|| GraphError::NodeNotFound(id.to_string()))?;

        Ok(self
            .graph
            .neighbors_directed(*node_idx, Direction::Outgoing)
            .filter_map(|idx| {
                let edge = self.graph.find_edge(*node_idx, idx)?;
                if self.graph[edge].relation_type == RelationType::Related {
                    Some(&self.graph[idx])
                } else {
                    None
                }
            })
            .collect())
    }

    /// Get the path from root to this node
    pub fn get_path_to_root(&self, id: &Uuid) -> Result<Vec<&DocumentNode>> {
        let mut path = Vec::new();
        let mut current_id = *id;

        while let Some(node) = self.get_parent(&current_id)? {
            path.push(node);
            current_id = node.id;
        }

        path.reverse();
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node(node_type: NodeType, content: &str) -> DocumentNode {
        DocumentNode::new(node_type, content.to_string(), None, None, 0, vec![])
    }

    #[test]
    fn test_graph_creation() {
        let graph = DocumentGraph::new();
        assert!(graph.node_map.is_empty());
    }

    #[test]
    fn test_add_node() {
        let mut graph = DocumentGraph::new();
        let node = create_test_node(NodeType::Section, "Test Section");
        let id = node.id;

        graph.add_node(node);
        assert!(graph.get_node(&id).is_some());
    }

    #[test]
    fn test_add_edge() {
        let mut graph = DocumentGraph::new();

        let node1 = create_test_node(NodeType::Section, "Parent Section");
        let node2 = create_test_node(NodeType::Subsection, "Child Section");

        let id1 = node1.id;
        let id2 = node2.id;

        graph.add_node(node1);
        graph.add_node(node2);

        let edge = DocumentEdge::new(id1, id2, RelationType::Contains);
        assert!(graph.add_edge(edge).is_ok());
    }

    #[test]
    fn test_get_children() {
        let mut graph = DocumentGraph::new();

        let parent = create_test_node(NodeType::Section, "Parent");
        let child1 = create_test_node(NodeType::Subsection, "Child 1");
        let child2 = create_test_node(NodeType::Subsection, "Child 2");

        let parent_id = parent.id;
        let child1_id = child1.id;
        let child2_id = child2.id;

        graph.add_node(parent);
        graph.add_node(child1);
        graph.add_node(child2);

        graph
            .add_edge(DocumentEdge::new(
                parent_id,
                child1_id,
                RelationType::Contains,
            ))
            .unwrap();
        graph
            .add_edge(DocumentEdge::new(
                parent_id,
                child2_id,
                RelationType::Contains,
            ))
            .unwrap();

        let children = graph.get_children(&parent_id).unwrap();
        assert_eq!(children.len(), 2);
    }
}
