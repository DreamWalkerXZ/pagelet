mod users;

use anyhow::{Context, Result, bail};
use argon2::{
    Argon2, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::get,
};
use rand::RngCore;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
}

#[derive(Serialize)]
pub struct Account {
    pub id: i64,
    pub username: String,
    pub role: String,
}

impl AppState {
    pub fn open(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory)?;
        let mut db = Connection::open(directory.join("pagelet.sqlite3"))?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        db.pragma_update(None, "foreign_keys", "ON")?;
        let transaction = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let version: i64 =
            transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 1 {
            bail!("Database schema is newer than this executable supports");
        }
        if version < 1 {
            transaction.execute_batch(
                "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                role TEXT NOT NULL CHECK(role IN ('administrator', 'user')),
                password_hash TEXT NOT NULL,
                token_hash BLOB NOT NULL UNIQUE
            ); PRAGMA user_version = 1;",
            )?;
        }
        transaction.commit()?;
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
        })
    }

    pub fn require_initialized(&self) -> Result<()> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE role = 'administrator')",
            [],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("Instance is not initialized; run pagelet init first");
        }
        Ok(())
    }

    pub fn initialize(&self, username: &str, password: &str) -> Result<String> {
        validate_username(username)?;
        validate_password(password)?;
        let hash = hash_password(password)?;
        let token = new_token();
        let mut db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        let transaction = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let count: i64 =
            transaction.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
        if count != 0 {
            bail!("Instance is already initialized");
        }
        transaction.execute("INSERT INTO users (username, role, password_hash, token_hash) VALUES (?1, 'administrator', ?2, ?3)", params![username, hash, token_hash(&token)])?;
        transaction.commit()?;
        Ok(token)
    }
}

pub fn validate_username(username: &str) -> Result<()> {
    if !(3..=32).contains(&username.len())
        || username.starts_with('-')
        || username.ends_with('-')
        || username.contains("--")
        || !username
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        bail!(
            "Username must contain 3–32 lowercase ASCII letters, digits, or single interior hyphens"
        );
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<()> {
    if !(12..=128).contains(&password.chars().count()) {
        bail!("Password must contain 12–128 characters");
    }
    Ok(())
}

pub fn hash_password(password: &str) -> Result<String> {
    Argon2::default()
        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|hash| hash.to_string())
        .map_err(|_| anyhow::anyhow!("Password hashing failed"))
}

pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub fn authenticated_user(state: &AppState, headers: &HeaderMap) -> Result<Account, StatusCode> {
    let token = headers
        .get("authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db.query_row(
        "SELECT id, username, role FROM users WHERE token_hash = ?1",
        [token_hash(token)],
        |row| {
            Ok(Account {
                id: row.get(0)?,
                username: row.get(1)?,
                role: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::UNAUTHORIZED)
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Account>, StatusCode> {
    authenticated_user(&state, &headers).map(Json)
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/_/api/v1/me", get(me))
        .route("/_/api/v1/users", get(users::list).post(users::create))
        .route("/_/openapi.json", get(|| async { Json(openapi()) }))
        .with_state(state)
}

pub fn openapi() -> serde_json::Value {
    serde_json::from_str(include_str!("../openapi.json"))
        .expect("Bundled OpenAPI must be valid JSON")
}

pub async fn serve(state: AppState, listen: &str) -> Result<()> {
    state.require_initialized()?;
    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .context("Could not bind HTTP listener")?;
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
