use anyhow::Result;
use qdrant_client::{
    config::QdrantConfig,
    qdrant::{
        point_id::PointIdOptions, points_selector::PointsSelectorOneOf, vectors_config::Config,
        CreateCollection, DeletePoints, Distance, PointId, PointStruct, PointsIdsList,
        PointsSelector, SearchPoints, UpsertPoints, Value, VectorParams, VectorsConfig,
        WithPayloadSelector, WithVectorsSelector, WriteOrdering,
    },
    Qdrant,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

use crate::external::error::ExternalError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDBConfig {
    pub collection_name: String,
    pub host: String,
    pub port: u16,
    pub vector_size: usize,
}

impl VectorDBConfig {
    /// Get the full URL for the Qdrant service
    pub fn get_url(&self) -> Result<String> {
        let url = if self.host.starts_with("http://") || self.host.starts_with("https://") {
            format!("{}:{}", self.host.trim_end_matches('/'), self.port)
        } else {
            format!("http://{}:{}", self.host, self.port)
        };

        // Validate the URL
        Url::parse(&url).map_err(|e| ExternalError::ConfigError(format!("Invalid URL: {}", e)))?;

        Ok(url)
    }
}

impl Default for VectorDBConfig {
    fn default() -> Self {
        Self {
            collection_name: "documents".to_string(),
            host: "localhost".to_string(),
            port: 6334,
            vector_size: 384,
        }
    }
}

/// Wrapper for Qdrant vector database
pub struct VectorDB {
    client: Qdrant,
    config: VectorDBConfig,
}

impl VectorDB {
    /// Create a new vector database client with the given configuration
    pub async fn new(config: VectorDBConfig) -> Result<Self> {
        let url = config.get_url()?;
        let qdrant_config = QdrantConfig::from_url(&url);
        let client = Qdrant::new(qdrant_config)
            .map_err(|e| ExternalError::ConnectionError(e.to_string()))?;

        Ok(Self { client, config })
    }

    /// Initialize the collection with the given configuration
    pub async fn init_collection(&self) -> Result<()> {
        let vectors_config = VectorsConfig {
            config: Some(Config::Params(VectorParams {
                size: self.config.vector_size as u64,
                distance: Distance::Cosine.into(),
                ..Default::default()
            })),
        };

        let create_collection = CreateCollection {
            collection_name: self.config.collection_name.clone(),
            vectors_config: Some(vectors_config),
            ..Default::default()
        };

        self.client
            .create_collection(create_collection)
            .await
            .map_err(|e| ExternalError::VectorDBError(e.to_string()))?;

        Ok(())
    }

    /// Insert vectors with metadata into the database
    pub async fn insert_vectors(
        &self,
        vectors: Vec<Vec<f32>>,
        metadata: Vec<HashMap<String, String>>,
    ) -> Result<Vec<String>> {
        let points: Vec<PointStruct> = vectors
            .into_iter()
            .zip(metadata)
            .enumerate()
            .map(|(i, (vector, meta))| {
                let payload: HashMap<String, Value> =
                    meta.into_iter().map(|(k, v)| (k, Value::from(v))).collect();

                PointStruct {
                    id: Some(PointId {
                        point_id_options: Some(PointIdOptions::Num(i as u64)),
                    }),
                    payload,
                    vectors: Some(vector.into()),
                }
            })
            .collect();

        let upsert_points = UpsertPoints {
            collection_name: self.config.collection_name.clone(),
            points: points.clone(),
            ordering: Some(WriteOrdering::default()),
            ..Default::default()
        };

        self.client
            .upsert_points(upsert_points)
            .await
            .map_err(|e| ExternalError::VectorDBError(e.to_string()))?;

        Ok(points
            .into_iter()
            .filter_map(|p| {
                p.id.map(|id| {
                    if let Some(PointIdOptions::Num(num)) = id.point_id_options {
                        num.to_string()
                    } else {
                        String::new()
                    }
                })
            })
            .collect())
    }

    /// Search for similar vectors
    pub async fn search_vectors(&self, vector: Vec<f32>, limit: u64) -> Result<Vec<(String, f32)>> {
        let search_request = SearchPoints {
            collection_name: self.config.collection_name.clone(),
            vector,
            limit,
            with_payload: Some(WithPayloadSelector::from(true)),
            with_vectors: Some(WithVectorsSelector::from(true)),
            ..Default::default()
        };

        let results = self
            .client
            .search_points(search_request)
            .await
            .map_err(|e| ExternalError::VectorDBError(e.to_string()))?;

        Ok(results
            .result
            .into_iter()
            .filter_map(|r| {
                r.id.and_then(|id| {
                    if let Some(PointIdOptions::Num(num)) = id.point_id_options {
                        Some((num.to_string(), r.score))
                    } else {
                        None
                    }
                })
            })
            .collect())
    }

    /// Delete vectors by their IDs
    pub async fn delete_vectors(&self, ids: Vec<String>) -> Result<()> {
        let point_ids: Vec<PointId> = ids
            .into_iter()
            .filter_map(|id| {
                id.parse::<u64>().ok().map(|num| PointId {
                    point_id_options: Some(PointIdOptions::Num(num)),
                })
            })
            .collect();

        let points_selector = PointsSelector {
            points_selector_one_of: Some(PointsSelectorOneOf::Points(PointsIdsList {
                ids: point_ids,
            })),
        };

        let delete_points = DeletePoints {
            collection_name: self.config.collection_name.clone(),
            points: Some(points_selector),
            ordering: Some(WriteOrdering::default()),
            ..Default::default()
        };

        self.client
            .delete_points(delete_points)
            .await
            .map_err(|e| ExternalError::VectorDBError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_generation() {
        // Test with plain hostname
        let config = VectorDBConfig {
            host: "localhost".to_string(),
            port: 6334,
            collection_name: "test".to_string(),
            vector_size: 384,
        };
        assert_eq!(config.get_url().unwrap(), "http://localhost:6334");

        // Test with http:// prefix
        let config = VectorDBConfig {
            host: "http://example.com".to_string(),
            port: 6334,
            collection_name: "test".to_string(),
            vector_size: 384,
        };
        assert_eq!(config.get_url().unwrap(), "http://example.com:6334");

        // Test with https:// prefix
        let config = VectorDBConfig {
            host: "https://example.com".to_string(),
            port: 6334,
            collection_name: "test".to_string(),
            vector_size: 384,
        };
        assert_eq!(config.get_url().unwrap(), "https://example.com:6334");
    }

    #[tokio::test]
    async fn test_vector_operations() {
        let config = VectorDBConfig::default();
        let db = VectorDB::new(config).await.unwrap();

        // Initialize collection
        db.init_collection().await.unwrap();

        // Test vector insertion
        let vectors = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let metadata = vec![
            [("key".to_string(), "value1".to_string())]
                .into_iter()
                .collect(),
            [("key".to_string(), "value2".to_string())]
                .into_iter()
                .collect(),
        ];

        let ids = db.insert_vectors(vectors.clone(), metadata).await.unwrap();
        assert_eq!(ids.len(), 2);

        // Test vector search
        let results = db.search_vectors(vec![1.0, 0.0], 2).await.unwrap();
        assert_eq!(results.len(), 2);

        // Test vector deletion
        db.delete_vectors(ids).await.unwrap();
    }
}
