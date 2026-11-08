mod deployment;
pub use deployment::Deployment;
mod ownership;
mod pages;
mod sessions;
mod users;
mod web;

use anyhow::{Context, Result, bail};
use argon2::{
    Argon2, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    Json, Router,
    extract::{MatchedPath, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
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
    pub deployment: Deployment,
    pub db: Arc<Mutex<Connection>>,
    pub clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    pub limits: Arc<Mutex<sessions::RateLimiter>>,
    pub slug_generator: Arc<dyn Fn() -> String + Send + Sync>,
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
        if version > 4 {
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
        if version < 2 {
            transaction.execute_batch(
                "CREATE TABLE pages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                slug TEXT NOT NULL UNIQUE,
                markdown TEXT NOT NULL,
                owner_id INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                version INTEGER NOT NULL DEFAULT 1,
                access_password_hash TEXT
            ); PRAGMA user_version = 2;",
            )?;
        }
        if version < 3 {
            transaction.execute_batch("CREATE TABLE sessions (token_hash BLOB PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE, expires_at INTEGER NOT NULL); PRAGMA user_version = 3;")?;
        }
        if version < 4 {
            transaction.execute_batch("CREATE TABLE page_grants (token_hash BLOB PRIMARY KEY, page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE, expires_at INTEGER NOT NULL); PRAGMA user_version = 4;")?;
        }
        transaction.commit()?;
        Ok(Self {
            deployment: Deployment::default(),
            db: Arc::new(Mutex::new(db)),
            clock: Arc::new(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
            }),
            limits: Arc::new(Mutex::new(sessions::RateLimiter::default())),
            slug_generator: Arc::new(pages::random_slug),
        })
    }

    pub fn is_initialized(&self) -> Result<bool> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?;
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE role = 'administrator')",
            [],
            |row| row.get(0),
        )?;
        Ok(exists)
    }

    pub fn require_initialized(&self) -> Result<()> {
        if !self.is_initialized()? {
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

fn pagination(page: Option<u64>, per_page: Option<u64>) -> Result<(u64, u64, i64), StatusCode> {
    let page = page.unwrap_or(1);
    let per_page = per_page.unwrap_or(20).min(100);
    let offset = page
        .checked_sub(1)
        .and_then(|page| page.checked_mul(per_page))
        .and_then(|offset| i64::try_from(offset).ok())
        .filter(|_| per_page != 0)
        .ok_or(StatusCode::BAD_REQUEST)?;
    Ok((page, per_page, offset))
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
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    authenticated_user_in(&db, headers, (state.clock)())
}

pub fn authenticated_user_in(
    db: &Connection,
    headers: &HeaderMap,
    now: i64,
) -> Result<Account, StatusCode> {
    if !headers.contains_key("authorization") {
        return sessions::authenticate_in(db, headers, now);
    }
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .filter(|t| !t.is_empty())
        .ok_or(StatusCode::UNAUTHORIZED)?;
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
        .merge(web::routes())
        .route("/_/api/v1/me", get(me))
        .merge(sessions::routes())
        .merge(ownership::routes())
        .route("/_/api/v1/users", get(users::list).post(users::create))
        .route(
            "/_/api/v1/pages",
            get(pages::list)
                .post(pages::create)
                .layer(axum::extract::DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route(
            "/_/api/v1/pages/{id}",
            get(pages::management_read)
                .patch(pages::edit)
                .delete(pages::delete)
                .layer(axum::extract::DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route("/_/api/v1/pages/by-slug/{slug}", get(pages::read))
        .route(
            "/_/api/v1/pages/by-slug/{slug}/verify-password",
            axum::routing::post(pages::verify_password),
        )
        .route("/_/raw/{slug}", get(pages::raw))
        .route("/_/openapi.json", get(|| async { Json(openapi()) }))
        .layer(axum::middleware::from_fn(sessions::csrf))
        .layer(axum::middleware::from_fn(log_request))
        .with_state(state)
}

pub fn openapi() -> serde_json::Value {
    serde_json::from_str(include_str!("../openapi.json"))
        .expect("Bundled OpenAPI must be valid JSON")
}

async fn log_request(path: Option<MatchedPath>, request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let start = std::time::Instant::now();
    let response = next.run(request).await;
    tracing::debug!(
        event = %"request_completed",
        %method,
        route = %path.as_ref().map_or("unmatched", MatchedPath::as_str),
        status = response.status().as_u16(),
        elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
    );
    response
}

pub async fn serve(state: AppState, listen: &str) -> Result<()> {
    state.require_initialized()?;
    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .context("Could not bind HTTP listener")?;
    let address = listener.local_addr()?;
    tracing::info!(event = %"listening", %address);
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => tracing::info!(event = %"shutdown_requested", signal = %"SIGINT"),
            Err(error) => tracing::error!(event = %"shutdown_signal_error", %error),
        }
    })
    .await?;
    tracing::info!(event = %"shutdown_completed");
    Ok(())
}
