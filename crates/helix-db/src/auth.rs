use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use lazy_static::lazy_static;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use tokio::sync::RwLock;
use validator::Validate;

use crate::{
    config::{ApiKeyConfig, HelixConfig},
    error::{HelixError, HelixResult},
};

lazy_static! {
    static ref DEFAULT_SCOPES: HashSet<String> = HashSet::from([
        "query.read".to_string(),
        "documents.write".to_string(),
        "metrics.read".to_string(),
        "plugins.register".to_string(),
    ]);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub name: String,
    pub hashed_key: String,
    pub scopes: HashSet<String>,
}

impl ApiKey {
    pub fn new(
        name: impl Into<String>,
        raw_key: impl AsRef<[u8]>,
        scopes: HashSet<String>,
    ) -> Self {
        let mut hasher = Sha3_256::new();
        hasher.update(raw_key.as_ref());
        let hashed = hex::encode(hasher.finalize());
        Self {
            name: name.into(),
            hashed_key: hashed,
            scopes,
        }
    }

    pub fn generate_secure(name: impl Into<String>, scopes: HashSet<String>) -> (Self, String) {
        let raw: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        let api_key = Self::new(name, raw.as_bytes(), scopes);
        (api_key, raw)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct JwtClaims {
    #[validate(length(min = 1))]
    pub sub: String,
    #[validate(length(min = 1))]
    pub iss: String,
    #[validate(length(min = 1))]
    pub aud: String,
    pub exp: usize,
    pub scopes: Option<Vec<String>>,
    pub roles: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct AuthManager {
    keys: Arc<RwLock<HashMap<String, ApiKey>>>,
    issuer: Option<String>,
    audience: Option<String>,
    rbac_roles: HashMap<String, HashSet<String>>,
    jwt_decoding: Option<DecodingKey>,
}

impl AuthManager {
    pub fn from_config(config: &HelixConfig) -> HelixResult<Self> {
        let keys = config
            .security
            .api_keys
            .iter()
            .map(|cfg| {
                let scopes = if cfg.scopes.is_empty() {
                    DEFAULT_SCOPES.clone()
                } else {
                    cfg.scopes.iter().cloned().collect()
                };
                let api_key = ApiKey::new(&cfg.name, cfg.key.as_bytes(), scopes);
                (cfg.name.clone(), api_key)
            })
            .collect();
        let issuer = config.security.jwt_issuer.clone();
        let audience = config.security.jwt_audience.clone();
        let jwt_decoding = config
            .security
            .jwt_issuer
            .as_ref()
            .map(|_| DecodingKey::from_secret(b"helix-shared-secret"));
        let rbac_roles = config
            .security
            .rbac_roles
            .iter()
            .map(|(role, scopes)| (role.clone(), scopes.iter().cloned().collect()))
            .collect();
        Ok(Self {
            keys: Arc::new(RwLock::new(keys)),
            issuer,
            audience,
            rbac_roles,
            jwt_decoding,
        })
    }

    pub async fn insert_key(&self, key: ApiKey) {
        self.keys.write().await.insert(key.name.clone(), key);
    }

    pub async fn authenticate_api_key(
        &self,
        presented: &str,
        required_scope: &str,
    ) -> HelixResult<()> {
        let mut hasher = Sha3_256::new();
        hasher.update(presented.as_bytes());
        let hashed = hex::encode(hasher.finalize());
        let keys = self.keys.read().await;
        if keys
            .values()
            .any(|key| key.hashed_key == hashed && key.scopes.contains(required_scope))
        {
            Ok(())
        } else {
            Err(HelixError::Authentication)
        }
    }

    pub async fn authenticate_jwt(
        &self,
        token: &str,
        required_scope: &str,
    ) -> HelixResult<JwtClaims> {
        let decoding = self
            .jwt_decoding
            .clone()
            .ok_or_else(|| HelixError::Authentication)?;
        let mut validation = Validation::new(Algorithm::HS256);
        if let Some(iss) = &self.issuer {
            validation.set_issuer(&[iss.clone()]);
        }
        if let Some(aud) = &self.audience {
            validation.set_audience(&[aud.clone()]);
        }
        let data = decode::<JwtClaims>(token, &decoding, &validation)
            .map_err(|_| HelixError::Authentication)?;
        data.claims
            .validate()
            .map_err(|_| HelixError::Authentication)?;
        let scopes: HashSet<String> = data
            .claims
            .scopes
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let roles: HashSet<String> = data
            .claims
            .roles
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect();
        if scopes.contains(required_scope) || self.role_allows(&roles, required_scope) {
            Ok(data.claims)
        } else {
            Err(HelixError::Authorization)
        }
    }

    pub fn role_allows(&self, roles: &HashSet<String>, scope: &str) -> bool {
        roles.iter().any(|role| {
            self.rbac_roles
                .get(role)
                .map(|set| set.contains(scope))
                .unwrap_or(false)
        })
    }
}
