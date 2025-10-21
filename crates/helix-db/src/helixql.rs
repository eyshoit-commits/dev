use std::time::Instant;

use serde::Deserialize;
use sha3::{Digest, Sha3_256};
use tracing::instrument;

use crate::{
    error::{HelixError, HelixResult},
    telemetry::{TelemetryEvent, TelemetryHub},
    vector::{SimilarityMetric, VectorIndex},
};

#[derive(Debug, Clone)]
pub struct HelixQuery {
    pub name: String,
    pub source: String,
}

impl From<HelixQueryLiteral> for HelixQuery {
    fn from(value: HelixQueryLiteral) -> Self {
        Self {
            name: value.name.to_string(),
            source: value.source.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HelixQueryLiteral {
    pub name: &'static str,
    pub source: &'static str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HelixQlRequest {
    pub query: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default = "default_metric")]
    pub metric: SimilarityMetric,
    #[serde(default = "default_k")]
    pub top_k: usize,
}

const fn default_metric() -> SimilarityMetric {
    SimilarityMetric::Cosine
}

const fn default_k() -> usize {
    10
}

pub struct HelixQlEngine {
    vector: VectorIndex,
    telemetry: TelemetryHub,
}

impl HelixQlEngine {
    pub fn new(vector: VectorIndex, telemetry: TelemetryHub) -> Self {
        Self { vector, telemetry }
    }

    #[instrument(skip(self, request))]
    pub async fn execute(&self, request: HelixQlRequest) -> HelixResult<serde_json::Value> {
        let start = Instant::now();
        let mut current_vector: Option<Vec<f32>> = None;
        for statement in request.query.split(';') {
            let stmt = statement.trim();
            if stmt.is_empty() {
                continue;
            }
            if stmt.starts_with("vec <- Embed") {
                let (text, model) = parse_embed(stmt)?;
                let vector = embed_text(&text, &model);
                current_vector = Some(vector);
            } else if stmt.starts_with("docs <- VectorSearch") {
                let vector = current_vector.clone().ok_or_else(|| {
                    HelixError::Query("Embed step required before VectorSearch".into())
                })?;
                let docs = self
                    .vector
                    .search(&vector, request.top_k, request.metric)?
                    .into_iter()
                    .map(|record| {
                        serde_json::json!({
                            "id": record.id,
                            "metadata": record.metadata,
                            "vector": record.values,
                        })
                    })
                    .collect::<Vec<_>>();
                let payload = serde_json::json!({ "documents": docs });
                self.telemetry.publish(TelemetryEvent::VectorSearch {
                    metric: format!("{:?}", request.metric),
                    top_k: request.top_k,
                    latency_ms: start.elapsed().as_secs_f64() * 1000.0,
                    timestamp: chrono::Utc::now(),
                })?;
                self.telemetry.publish(TelemetryEvent::QueryExecuted {
                    query: request.query.clone(),
                    latency_ms: start.elapsed().as_secs_f64() * 1000.0,
                    timestamp: chrono::Utc::now(),
                })?;
                return Ok(payload);
            } else if stmt.starts_with("RETURN") {
                return Ok(
                    serde_json::json!({ "return": stmt.trim_start_matches("RETURN").trim() }),
                );
            } else {
                return Err(HelixError::Query(format!(
                    "Unsupported HelixQL statement: {stmt}"
                )));
            }
        }
        Err(HelixError::Query(
            "No executable statements in query".into(),
        ))
    }
}

fn parse_embed(stmt: &str) -> HelixResult<(String, String)> {
    // Expect pattern vec <- Embed(text,'model') or "model"
    let start = stmt
        .find('(')
        .ok_or_else(|| HelixError::Query("Embed statement missing '('".into()))?;
    let end = stmt
        .rfind(')')
        .ok_or_else(|| HelixError::Query("Embed statement missing ')'".into()))?;
    let inner = &stmt[start + 1..end];
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
    if parts.len() != 2 {
        return Err(HelixError::Query("Embed expects two arguments".into()));
    }
    let text = parts[0].trim_matches(['"', '\''] as &[_]).to_string();
    let model = parts[1].trim_matches(['"', '\''] as &[_]).to_string();
    Ok((text, model))
}

fn embed_text(text: &str, model: &str) -> Vec<f32> {
    let mut hasher = Sha3_256::new();
    hasher.update(model.as_bytes());
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    digest
        .chunks(4)
        .map(|chunk| {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(chunk);
            f32::from_bits(u32::from_be_bytes(bytes))
        })
        .collect()
}
