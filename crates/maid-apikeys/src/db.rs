use crate::security::hash_password;
use anyhow::Context;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT NOT NULL UNIQUE,
                display_name TEXT,
                password_hash TEXT NOT NULL,
                scopes_json TEXT NOT NULL,
                roles_json TEXT NOT NULL,
                active INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                name TEXT,
                prefix TEXT NOT NULL,
                key_hash TEXT NOT NULL,
                scopes_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT,
                revoked INTEGER NOT NULL DEFAULT 0,
                last_used TEXT
            );
            CREATE TABLE IF NOT EXISTS audit_events (
                id TEXT PRIMARY KEY,
                user_id TEXT,
                event_type TEXT NOT NULL,
                details_json TEXT,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn create_user(&self, input: NewUser<'_>) -> anyhow::Result<User> {
        let mut conn = self.conn.lock();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let password_hash = hash_password(input.password)?;
        conn.execute(
            "INSERT INTO users (id, email, display_name, password_hash, scopes_json, roles_json, active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?7)",
            params![
                id,
                input.email,
                input.display_name,
                password_hash,
                serde_json::to_string(&input.scopes)?,
                serde_json::to_string(&input.roles)?,
                now.to_rfc3339(),
            ],
        )?;
        Ok(User {
            id,
            email: input.email.to_string(),
            display_name: input.display_name.map(|s| s.to_string()),
            password_hash,
            scopes: input.scopes.to_vec(),
            roles: input.roles.to_vec(),
            active: true,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn find_user_by_email(&self, email: &str) -> anyhow::Result<Option<User>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, email, display_name, password_hash, scopes_json, roles_json, active, created_at, updated_at
             FROM users WHERE email = ?1",
        )?;
        let mut rows = stmt.query([email])?;
        if let Some(row) = rows.next()? {
            Ok(Some(User {
                id: row.get(0)?,
                email: row.get(1)?,
                display_name: row.get::<_, Option<String>>(2)?,
                password_hash: row.get(3)?,
                scopes: serde_json::from_str(&row.get::<_, String>(4)?)?,
                roles: serde_json::from_str(&row.get::<_, String>(5)?)?,
                active: row.get::<_, i64>(6)? == 1,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)?
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)?
                    .with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn find_user_by_id(&self, user_id: &str) -> anyhow::Result<Option<User>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, email, display_name, password_hash, scopes_json, roles_json, active, created_at, updated_at
             FROM users WHERE id = ?1",
        )?;
        let mut rows = stmt.query([user_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(User {
                id: row.get(0)?,
                email: row.get(1)?,
                display_name: row.get::<_, Option<String>>(2)?,
                password_hash: row.get(3)?,
                scopes: serde_json::from_str(&row.get::<_, String>(4)?)?,
                roles: serde_json::from_str(&row.get::<_, String>(5)?)?,
                active: row.get::<_, i64>(6)? == 1,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)?
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)?
                    .with_timezone(&Utc),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn create_api_key(
        &self,
        user_id: &str,
        name: Option<&str>,
        scopes: &[String],
        prefix: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<ApiKeyRecord> {
        let mut conn = self.conn.lock();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let (plain, hash) = crate::security::generate_api_key(prefix);
        conn.execute(
            "INSERT INTO api_keys (id, user_id, name, prefix, key_hash, scopes_json, created_at, expires_at, revoked)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
            params![
                id,
                user_id,
                name,
                prefix,
                hash,
                serde_json::to_string(scopes)?,
                now.to_rfc3339(),
                expires_at.map(|dt| dt.to_rfc3339()),
            ],
        )?;
        Ok(ApiKeyRecord {
            id,
            user_id: user_id.to_string(),
            name: name.map(|s| s.to_string()),
            prefix: prefix.to_string(),
            key_hash: hash,
            scopes: scopes.to_vec(),
            created_at: now,
            expires_at,
            revoked: false,
            last_used: None,
            plain_key: Some(plain),
        })
    }

    pub fn find_api_key_by_hash(&self, hash: &str) -> anyhow::Result<Option<ApiKeyRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, name, prefix, key_hash, scopes_json, created_at, expires_at, revoked, last_used
             FROM api_keys WHERE key_hash = ?1",
        )?;
        let mut rows = stmt.query([hash])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ApiKeyRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                name: row.get(2)?,
                prefix: row.get(3)?,
                key_hash: row.get(4)?,
                scopes: serde_json::from_str(&row.get::<_, String>(5)?)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)?
                    .with_timezone(&Utc),
                expires_at: row
                    .get::<_, Option<String>>(7)?
                    .map(|v| DateTime::parse_from_rfc3339(&v).map(|dt| dt.with_timezone(&Utc)))
                    .transpose()?,
                revoked: row.get::<_, i64>(8)? == 1,
                last_used: row
                    .get::<_, Option<String>>(9)?
                    .map(|v| DateTime::parse_from_rfc3339(&v).map(|dt| dt.with_timezone(&Utc)))
                    .transpose()?,
                plain_key: None,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn find_api_key_by_id(&self, id: &str) -> anyhow::Result<Option<ApiKeyRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, name, prefix, key_hash, scopes_json, created_at, expires_at, revoked, last_used
             FROM api_keys WHERE id = ?1",
        )?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ApiKeyRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                name: row.get(2)?,
                prefix: row.get(3)?,
                key_hash: row.get(4)?,
                scopes: serde_json::from_str(&row.get::<_, String>(5)?)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)?
                    .with_timezone(&Utc),
                expires_at: row
                    .get::<_, Option<String>>(7)?
                    .map(|v| DateTime::parse_from_rfc3339(&v).map(|dt| dt.with_timezone(&Utc)))
                    .transpose()?,
                revoked: row.get::<_, i64>(8)? == 1,
                last_used: row
                    .get::<_, Option<String>>(9)?
                    .map(|v| DateTime::parse_from_rfc3339(&v).map(|dt| dt.with_timezone(&Utc)))
                    .transpose()?,
                plain_key: None,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_api_keys(&self, user_id: &str) -> anyhow::Result<Vec<ApiKeyRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, name, prefix, key_hash, scopes_json, created_at, expires_at, revoked, last_used
             FROM api_keys WHERE user_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([user_id], |row| {
            Ok(ApiKeyRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                name: row.get(2)?,
                prefix: row.get(3)?,
                key_hash: row.get(4)?,
                scopes: serde_json::from_str(&row.get::<_, String>(5)?)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)?
                    .with_timezone(&Utc),
                expires_at: row
                    .get::<_, Option<String>>(7)?
                    .map(|v| DateTime::parse_from_rfc3339(&v).map(|dt| dt.with_timezone(&Utc)))
                    .transpose()?,
                revoked: row.get::<_, i64>(8)? == 1,
                last_used: row
                    .get::<_, Option<String>>(9)?
                    .map(|v| DateTime::parse_from_rfc3339(&v).map(|dt| dt.with_timezone(&Utc)))
                    .transpose()?,
                plain_key: None,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn touch_api_key(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE api_keys SET last_used = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    pub fn revoke_api_key(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute("UPDATE api_keys SET revoked = 1 WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn rotate_api_key(&self, id: &str, prefix: &str) -> anyhow::Result<ApiKeyRecord> {
        let mut conn = self.conn.lock();
        let (plain, hash) = crate::security::generate_api_key(prefix);
        let now = Utc::now();
        conn.execute(
            "UPDATE api_keys SET key_hash = ?1, prefix = ?2, last_used = NULL, revoked = 0 WHERE id = ?3",
            params![hash, prefix, id],
        )?;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, name, prefix, key_hash, scopes_json, created_at, expires_at, revoked, last_used
             FROM api_keys WHERE id = ?1",
        )?;
        let mut rows = stmt.query([id])?;
        let row = rows.next()?.context("api key not found")?;
        Ok(ApiKeyRecord {
            id: row.get(0)?,
            user_id: row.get(1)?,
            name: row.get(2)?,
            prefix: row.get(3)?,
            key_hash: row.get(4)?,
            scopes: serde_json::from_str(&row.get::<_, String>(5)?)?,
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)?
                .with_timezone(&Utc),
            expires_at: row
                .get::<_, Option<String>>(7)?
                .map(|v| DateTime::parse_from_rfc3339(&v).map(|dt| dt.with_timezone(&Utc)))
                .transpose()?,
            revoked: row.get::<_, i64>(8)? == 1,
            last_used: row
                .get::<_, Option<String>>(9)?
                .map(|v| DateTime::parse_from_rfc3339(&v).map(|dt| dt.with_timezone(&Utc)))
                .transpose()?,
            plain_key: Some(plain),
        })
    }

    pub fn audit(&self, event: AuditEvent<'_>) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO audit_events (id, user_id, event_type, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                event.user_id,
                event.event_type,
                event.details.map(serde_json::to_string).transpose()?,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub password_hash: String,
    pub scopes: Vec<String>,
    pub roles: Vec<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct NewUser<'a> {
    pub email: &'a str,
    pub password: &'a str,
    pub display_name: Option<&'a str>,
    pub scopes: &'a [String],
    pub roles: &'a [String],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiKeyRecord {
    pub id: String,
    pub user_id: String,
    pub name: Option<String>,
    pub prefix: String,
    pub key_hash: String,
    pub scopes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked: bool,
    pub last_used: Option<DateTime<Utc>>,
    #[serde(skip_serializing)]
    pub plain_key: Option<String>,
}

pub struct AuditEvent<'a> {
    pub user_id: Option<&'a str>,
    pub event_type: &'a str,
    pub details: Option<serde_json::Value>,
}
