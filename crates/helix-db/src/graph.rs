use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    document::DocumentRecord,
    error::{HelixError, HelixResult},
    storage::StorageEngine,
};

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct NodeRecord {
    #[validate(length(min = 1))]
    pub id: String,
    pub labels: Vec<String>,
    pub properties: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl NodeRecord {
    pub fn new(id: impl Into<String>, labels: Vec<String>, properties: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            labels,
            properties,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct EdgeRecord {
    #[validate(length(min = 1))]
    pub id: String,
    #[validate(length(min = 1))]
    pub source: String,
    #[validate(length(min = 1))]
    pub target: String,
    pub label: String,
    pub weight: f32,
    pub properties: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl EdgeRecord {
    pub fn new(
        id: impl Into<String>,
        source: impl Into<String>,
        target: impl Into<String>,
        label: impl Into<String>,
        weight: f32,
        properties: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            label: label.into(),
            weight,
            properties,
            created_at: Utc::now(),
        }
    }
}

#[derive(Clone)]
pub struct GraphEngine {
    storage: Arc<StorageEngine>,
}

impl GraphEngine {
    pub fn new(storage: Arc<StorageEngine>) -> Self {
        Self { storage }
    }

    pub fn upsert_node(&self, node: &NodeRecord) -> HelixResult<()> {
        node.validate()
            .map_err(|err| HelixError::Storage(err.to_string()))?;
        self.storage.put_node(node)
    }

    pub fn upsert_edge(&self, edge: &EdgeRecord) -> HelixResult<()> {
        edge.validate()
            .map_err(|err| HelixError::Storage(err.to_string()))?;
        self.storage.put_edge(edge)
    }

    pub fn get_node(&self, id: &str) -> HelixResult<Option<NodeRecord>> {
        self.storage.get_node(id)
    }

    pub fn neighbors(&self, id: &str) -> HelixResult<Vec<NodeRecord>> {
        let edges = self.storage.get_edges_by_source(id)?;
        let mut neighbors = Vec::with_capacity(edges.len());
        for edge in edges {
            if let Some(node) = self.storage.get_node(&edge.target)? {
                neighbors.push(node);
            }
        }
        Ok(neighbors)
    }

    pub fn attach_document(&self, node_id: &str, mut document: DocumentRecord) -> HelixResult<()> {
        document
            .validate()
            .map_err(|err| HelixError::Storage(err.to_string()))?;
        self.storage.put_document_for_node(node_id, &document)
    }
}
