use anyhow::Result;
use uuid::Uuid;

use crate::external::{VectorDB, VectorDBConfig};

/// Wrapper around VectorDB
pub struct VectorStore {
    db: VectorDB,
}

impl VectorStore {
    /// Create a new vector store with a specific configuration
    pub async fn new(config: VectorDBConfig) -> Result<Self> {
        let db = VectorDB::new(config).await?;
        db.init_collection().await?;
        Ok(Self { db })
    }

    /// Add or update a document embedding
    pub async fn add_embedding(
        &self,
        _id: &Uuid,
        embedding: Vec<f32>,
        metadata: serde_json::Value,
    ) -> Result<()> {
        let metadata_map = serde_json::from_value(metadata)?;
        self.db
            .insert_vectors(vec![embedding], vec![metadata_map])
            .await?;
        Ok(())
    }

    /// Search for similar documents
    pub async fn search_similar(
        &self,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(String, f32)>> {
        self.db
            .search_vectors(embedding.to_vec(), limit as u64)
            .await
    }

    /// Delete an embedding
    pub async fn delete_embedding(&self, id: &Uuid) -> Result<()> {
        self.db.delete_vectors(vec![id.to_string()]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vector_store_creation() {
        let config = VectorDBConfig::default();
        let store = VectorStore::new(config).await;
        assert!(store.is_ok());
    }

    #[tokio::test]
    async fn test_embedding_operations() {
        let config = VectorDBConfig::default();
        let store = VectorStore::new(config).await.unwrap();
        let id = Uuid::new_v4();
        let embedding = vec![1.0, 2.0, 3.0];
        let metadata = serde_json::json!({
            "text": "Test document",
            "source": "test",
        });

        // Add embedding
        store
            .add_embedding(&id, embedding.clone(), metadata)
            .await
            .unwrap();

        // Search similar
        let similar = store.search_similar(&embedding, 1).await.unwrap();
        assert!(!similar.is_empty());

        // Delete embedding
        store.delete_embedding(&id).await.unwrap();
    }
}
