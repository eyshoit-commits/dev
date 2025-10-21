use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DocumentRecord {
    #[validate(length(min = 1))]
    pub id: String,
    pub body: serde_json::Value,
    pub metadata: HashMap<String, serde_json::Value>,
    pub embedding: Option<Vec<f32>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DocumentRecord {
    pub fn new(id: impl Into<String>, body: serde_json::Value) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            body,
            metadata: HashMap::new(),
            embedding: None,
            created_at: now,
            updated_at: now,
        }
    }
}
