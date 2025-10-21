use crate::config::SecurityConfig;
use anyhow::Context;
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{Duration, Utc};
use data_encoding::HEXLOWER;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use ring::digest::{Context as DigestContext, SHA256};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .context("failed to hash password")?;
    Ok(hash.to_string())
}

pub fn verify_password(hash: &str, password: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(hash).context("invalid password hash")?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn generate_api_key(prefix: &str) -> (String, String) {
    let random = Uuid::new_v4().to_string().replace('-', "");
    let key = format!("{}{}", prefix, random);
    let hash = hash_api_key(&key);
    (key, hash)
}

pub fn hash_api_key(key: &str) -> String {
    let mut ctx = DigestContext::new(&SHA256);
    ctx.update(key.as_bytes());
    let digest = ctx.finish();
    HEXLOWER.encode(digest.as_ref())
}

#[derive(Clone)]
pub struct JwtSigner {
    secret: String,
    issuer: String,
    audience: String,
    expiry_minutes: i64,
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl JwtSigner {
    pub fn new(config: &SecurityConfig) -> Self {
        let encoding = EncodingKey::from_secret(config.jwt_secret.as_bytes());
        let decoding = DecodingKey::from_secret(config.jwt_secret.as_bytes());
        Self {
            secret: config.jwt_secret.clone(),
            issuer: config.jwt_issuer.clone(),
            audience: config.jwt_audience.clone(),
            expiry_minutes: config.jwt_expiry_minutes,
            encoding,
            decoding,
        }
    }

    pub fn issue(&self, subject: &str, email: &str, scopes: &[String]) -> anyhow::Result<String> {
        let now = Utc::now();
        let claims = Claims {
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            sub: subject.to_string(),
            email: email.to_string(),
            scope: scopes.join(" "),
            iat: now.timestamp(),
            exp: (now + Duration::minutes(self.expiry_minutes)).timestamp(),
            jti: Uuid::new_v4().to_string(),
        };
        let token = jsonwebtoken::encode(&Header::default(), &claims, &self.encoding)?;
        Ok(token)
    }

    pub fn verify(&self, token: &str) -> anyhow::Result<Claims> {
        let mut validation = Validation::default();
        validation.set_audience(&[self.audience.clone()]);
        validation.set_issuer(&[self.issuer.clone()]);
        let token_data = jsonwebtoken::decode::<Claims>(token, &self.decoding, &validation)?;
        Ok(token_data.claims)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub email: String,
    pub scope: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

impl Claims {
    pub fn scopes(&self) -> Vec<String> {
        self.scope
            .split_whitespace()
            .map(|s| s.to_string())
            .collect()
    }
}

pub fn merge_scopes(default: &[String], provided: &[String]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for scope in default.iter().chain(provided.iter()) {
        set.insert(scope.clone());
    }
    set.into_iter().collect()
}
