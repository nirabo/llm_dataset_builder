#[cfg(not(test))]
use crate::external::vectordb::VectorDB;
use anyhow::Result;
use qdrant_client::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(test)]
use mockall::automock;

#[cfg_attr(test, automock)]
pub trait VectorDBTrait {
    fn init_collection(&self) -> Result<()>;
    fn insert_vectors(
        &self,
        vectors: Vec<Vec<f32>>,
        metadata: Vec<HashMap<String, String>>,
    ) -> Result<Vec<String>>;
    fn search_vectors(&self, vector: Vec<f32>, limit: u64) -> Result<Vec<(String, f32)>>;
    fn delete_vectors(&self, ids: Vec<String>) -> Result<()>;
}

#[cfg(not(test))]
impl VectorDBTrait for VectorDB {
    fn init_collection(&self) -> Result<()> {
        tokio::runtime::Runtime::new()?.block_on(self.init_collection())
    }

    fn insert_vectors(
        &self,
        vectors: Vec<Vec<f32>>,
        metadata: Vec<HashMap<String, String>>,
    ) -> Result<Vec<String>> {
        tokio::runtime::Runtime::new()?.block_on(self.insert_vectors(vectors, metadata))
    }

    fn search_vectors(&self, vector: Vec<f32>, limit: u64) -> Result<Vec<(String, f32)>> {
        tokio::runtime::Runtime::new()?.block_on(self.search_vectors(vector, limit))
    }

    fn delete_vectors(&self, ids: Vec<String>) -> Result<()> {
        tokio::runtime::Runtime::new()?.block_on(self.delete_vectors(ids))
    }
}

pub struct VectorStore {
    #[cfg(not(test))]
    db: VectorDB,
    #[cfg(test)]
    db: MockVectorDBTrait,
    collection_name: String,
}

impl VectorStore {
    #[cfg(not(test))]
    pub async fn new(config: crate::external::vectordb::VectorDBConfig, collection_name: &str) -> Result<Self> {
        let db = VectorDB::new(config).await?;
        db.init_collection().await?;
        Ok(Self {
            db,
            collection_name: collection_name.to_string(),
        })
    }

    #[cfg(test)]
    pub fn new_with_mock(mock: MockVectorDBTrait, collection_name: &str) -> Self {
        Self {
            db: mock,
            collection_name: collection_name.to_string(),
        }
    }

    pub async fn add_embedding(
        &self,
        _id: &Uuid,
        embedding: Vec<f32>,
        metadata: Value,
    ) -> Result<()> {
        let metadata_map: HashMap<String, String> = serde_json::from_value(metadata)?;
        let ids = self
            .db
            .insert_vectors(vec![embedding], vec![metadata_map])
            .await?;
        if ids.is_empty() {
            anyhow::bail!("Failed to insert embedding");
        }
        Ok(())
    }

    pub async fn search_similar(&self, embedding: &[f32], limit: u64) -> Result<Vec<(String, f32)>> {
        self.db.search_vectors(embedding.to_vec(), limit).await
    }

    pub async fn delete_embedding(&self, id: &Uuid) -> Result<()> {
        self.db.delete_vectors(vec![id.to_string()]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_vector_store_creation() {
        let mut mock = MockVectorDBTrait::new();
        mock.expect_init_collection().times(1).returning(|| Ok(()));

        let store = VectorStore::new_with_mock(mock, "test_collection");
        assert_eq!(store.collection_name, "test_collection");
    }

    #[tokio::test]
    async fn test_embedding_operations() {
        let mut mock = MockVectorDBTrait::new();

        // Setup expectations
        mock.expect_insert_vectors()
            .with(predicate::always(), predicate::always())
            .times(1)
            .returning(|vectors, _| {
                Ok(vectors
                    .iter()
                    .enumerate()
                    .map(|(i, _)| i.to_string())
                    .collect())
            });

        mock.expect_search_vectors()
            .with(predicate::always(), predicate::eq(2u64))
            .times(1)
            .returning(|_, _| Ok(vec![("0".to_string(), 0.9), ("1".to_string(), 0.8)]));

        let store = VectorStore::new_with_mock(mock, "test_collection");

        // Test storing embeddings
        let id = Uuid::new_v4();
        let embedding = vec![1.0, 0.0];
        let metadata = serde_json::json!({
            "key": "value1"
        });

        assert!(store
            .add_embedding(&id, embedding.clone(), metadata)
            .await
            .is_ok());

        // Test querying similar embeddings
        let results = store.search_similar(&embedding, 2).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "0");
        assert_eq!(results[1].0, "1");
    }
}
