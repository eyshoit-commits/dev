use std::{cmp::Ordering, collections::BinaryHeap, sync::Arc};

use nalgebra::DVector;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    error::{HelixError, HelixResult},
    storage::StorageEngine,
};

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct VectorRecord {
    #[validate(length(min = 1))]
    pub id: String,
    pub values: Vec<f32>,
    pub metadata: serde_json::Value,
}

impl VectorRecord {
    pub fn new(id: impl Into<String>, values: Vec<f32>, metadata: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            values,
            metadata,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimilarityMetric {
    Cosine,
    L2,
    Dot,
}

#[derive(Clone)]
pub struct VectorIndex {
    storage: Arc<StorageEngine>,
}

impl VectorIndex {
    pub fn new(storage: Arc<StorageEngine>) -> Self {
        Self { storage }
    }

    pub fn upsert_vector(&self, record: &VectorRecord) -> HelixResult<()> {
        record
            .validate()
            .map_err(|err| HelixError::Storage(err.to_string()))?;
        self.storage.put_vector(record)
    }

    pub fn search(
        &self,
        query: &[f32],
        k: usize,
        metric: SimilarityMetric,
    ) -> HelixResult<Vec<VectorRecord>> {
        if query.is_empty() {
            return Err(HelixError::Query("query vector must not be empty".into()));
        }
        let mut heap: BinaryHeap<ScoredVector> = BinaryHeap::new();
        for record in self.storage.iter_vectors()? {
            let score = similarity(query, &record.values, metric);
            heap.push(ScoredVector { score, record });
            if heap.len() > k {
                heap.pop();
            }
        }
        let mut results: Vec<_> = heap
            .into_sorted_vec()
            .into_iter()
            .map(|sv| sv.record)
            .collect();
        results.reverse();
        Ok(results)
    }
}

fn similarity(lhs: &[f32], rhs: &[f32], metric: SimilarityMetric) -> f32 {
    match metric {
        SimilarityMetric::Cosine => {
            let a = DVector::from_vec(lhs.to_vec());
            let b = DVector::from_vec(rhs.to_vec());
            let denom = a.norm() * b.norm();
            if denom == 0.0 {
                0.0
            } else {
                a.dot(&b) / denom
            }
        }
        SimilarityMetric::L2 => {
            let a = DVector::from_vec(lhs.to_vec());
            let b = DVector::from_vec(rhs.to_vec());
            -(a - b).norm()
        }
        SimilarityMetric::Dot => {
            let a = DVector::from_vec(lhs.to_vec());
            let b = DVector::from_vec(rhs.to_vec());
            a.dot(&b)
        }
    }
}

#[derive(Debug)]
struct ScoredVector {
    score: f32,
    record: VectorRecord,
}

impl PartialEq for ScoredVector {
    fn eq(&self, other: &Self) -> bool {
        self.score.eq(&other.score)
    }
}

impl Eq for ScoredVector {}

impl PartialOrd for ScoredVector {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.score.partial_cmp(&other.score)
    }
}

impl Ord for ScoredVector {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}
