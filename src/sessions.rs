use crate::{
    Account, AppState, authenticated_user, authenticated_user_in, hash_password, new_token,
    token_hash, validate_password,
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    Json, Router,
    extract::{ConnectInfo, Path, Request, State},
    http::{HeaderMap, Method, StatusCode},
    middleware::Next,
    response::Response,
    routing::post,
};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashMap, net::SocketAddr};

const WEEK: i64 = 604800;
#[derive(Default)]
pub struct RateLimiter {
    entries: HashMap<String, (i64, u32)>,
}
impl RateLimiter {
    pub fn check(
        &mut self,
        scope: &str,
        source: &str,
        target: &str,
        now: i64,
    ) -> Result<(), StatusCode> {
        self.entries.retain(|_, (until, _)| *until > now);
        let keys = [
            format!("{scope}:source:{source}"),
            format!("{scope}:target:{target}"),
        ];
        if keys
            .iter()
            .any(|k| self.entries.get(k).is_some_and(|(_, n)| *n >= 10))
        {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        for key in keys {
            let e = self.entries.entry(key).or_insert((now + 60, 0));
            e.1 += 1;
        }
        Ok(())
    }
}
pub(crate) async fn csrf(request: Request, next: Next) -> Result<Response, StatusCode> {
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) && (request.uri().path() == "/_/api/v1/login"
        || (!request.headers().contains_key("authorization")
            && cookie(request.headers()).is_some()))
        && request
            .headers()
            .get("x-pagelet-csrf")
            .and_then(|h| h.to_str().ok())
            != Some("1")
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    Ok(response)
}
fn cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("cookie")?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|v| v.trim().strip_prefix("pagelet_session="))
}
pub(crate) fn authenticate_in(
    db: &rusqlite::Connection,
    headers: &HeaderMap,
    now: i64,
) -> Result<Account, StatusCode> {
    let token = cookie(headers).ok_or(StatusCode::UNAUTHORIZED)?;
    db.query_row(
        "SELECT u.id, u.username, u.role FROM users u JOIN sessions s ON u.id=s.user_id WHERE s.token_hash=?1 AND s.expires_at>?2",
        params![token_hash(token), now], |row| {
            Ok(Account { id: row.get(0)?, username: row.get(1)?, role: row.get(2)? })
        }
    ).optional().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::UNAUTHORIZED)
}

fn verified(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| {
        Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok()
    })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    username: String,
    password: String,
}
async fn login(
    State(state): State<AppState>,
    peer: Result<ConnectInfo<SocketAddr>, axum::extract::rejection::ExtensionRejection>,
    headers: HeaderMap,
    Json(body): Json<Login>,
) -> Result<(HeaderMap, Json<Value>), StatusCode> {
    let now = (state.clock)();
    state
        .limits
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .check(
            "login",
            &state
                .deployment
                .source(peer.ok().map(|p| p.0.ip()), &headers)?,
            &body.username,
            now,
        )?;
    if body.password.chars().count() > 128 {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let (id, hash) = {
        let db = state
            .db
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        db.query_row(
            "SELECT id,password_hash FROM users WHERE username=?1",
            [&body.username],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?
    };
    if !verified(&body.password, &hash) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let token = new_token();
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let changed=db.execute("INSERT INTO sessions(token_hash,user_id,expires_at) SELECT ?1,id,?2 FROM users WHERE id=?3 AND password_hash=?4",params![token_hash(&token),now+WEEK,id,hash]).map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if changed == 0 {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        "set-cookie",
        format!(
            "pagelet_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={WEEK}{}",
            state.deployment.cookie_suffix()
        )
        .parse()
        .unwrap(),
    );
    headers.insert("cache-control", "no-store".parse().unwrap());
    Ok((headers, Json(json!({"id":id}))))
}
async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(HeaderMap, StatusCode), StatusCode> {
    authenticated_user(&state, &headers)?;
    if let Some(token) = cookie(&headers) {
        state
            .db
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .execute(
                "DELETE FROM sessions WHERE token_hash=?1",
                [token_hash(token)],
            )
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let mut h = HeaderMap::new();
    h.insert(
        "set-cookie",
        format!(
            "pagelet_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
            state.deployment.cookie_suffix()
        )
        .parse()
        .unwrap(),
    );
    Ok((h, StatusCode::NO_CONTENT))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Password {
    password: String,
    current_password: Option<String>,
}
async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Password>,
) -> Result<Json<Value>, StatusCode> {
    let actor = authenticated_user(&state, &headers)?;
    replace_password(
        &state,
        actor.id,
        &body.password,
        Some(&headers),
        Some(
            body.current_password
                .as_deref()
                .ok_or(StatusCode::BAD_REQUEST)?,
        ),
    )
    .map(|token| Json(json!({"token":token})))
}
async fn reset_password(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(body): Json<Password>,
) -> Result<Json<Value>, StatusCode> {
    if authenticated_user(&state, &headers)?.role != "administrator" {
        return Err(StatusCode::FORBIDDEN);
    }
    replace_password(&state, id, &body.password, Some(&headers), None)
        .map(|token| Json(json!({"token":token})))
}
fn replace_password(
    state: &AppState,
    id: i64,
    password: &str,
    headers: Option<&HeaderMap>,
    current: Option<&str>,
) -> Result<String, StatusCode> {
    validate_password(password).map_err(|_| StatusCode::BAD_REQUEST)?;
    let old = {
        let db = state
            .db
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        db.query_row("SELECT password_hash FROM users WHERE id=?1", [id], |r| {
            r.get::<_, String>(0)
        })
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?
    };
    if current.is_some_and(|p| !verified(p, &old)) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let hash = hash_password(password).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let token = new_token();
    let mut db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(headers) = headers {
        let actor = authenticated_user_in(&tx, headers, (state.clock)())?;
        if (current.is_some() && actor.id != id)
            || (current.is_none() && actor.role != "administrator")
        {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    if tx
        .execute(
            "UPDATE users SET password_hash=?1,token_hash=?2 WHERE id=?3 AND password_hash=?4",
            params![hash, token_hash(&token), id, old],
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        == 0
    {
        return Err(StatusCode::CONFLICT);
    }
    tx.execute("DELETE FROM sessions WHERE user_id=?1", [id])
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(token)
}
async fn own_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let actor = authenticated_user(&state, &headers)?;
    rotate(&state, actor.id, &headers)
}
async fn user_token(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let actor = authenticated_user(&state, &headers)?;
    if actor.id != id && actor.role != "administrator" {
        return Err(StatusCode::FORBIDDEN);
    }
    rotate(&state, id, &headers)
}
fn rotate(state: &AppState, id: i64, headers: &HeaderMap) -> Result<Json<Value>, StatusCode> {
    let token = new_token();
    let mut db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let actor = authenticated_user_in(&tx, headers, (state.clock)())?;
    if actor.id != id && actor.role != "administrator" {
        return Err(StatusCode::FORBIDDEN);
    }
    if tx
        .execute(
            "UPDATE users SET token_hash=?1 WHERE id=?2",
            params![token_hash(&token), id],
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        == 0
    {
        return Err(StatusCode::NOT_FOUND);
    }
    tx.commit().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({"token":token})))
}
impl AppState {
    pub fn recover_administrator(&self, username: &str, password: &str) -> anyhow::Result<String> {
        let id = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("Database lock failed"))?
            .query_row(
                "SELECT id FROM users WHERE username=?1 AND role='administrator'",
                [username],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("Administrator not found"))?;
        replace_password(self, id, password, None, None)
            .map_err(|_| anyhow::anyhow!("Recovery failed; verify password requirements and retry"))
    }
}
pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/_/api/v1/login", post(login))
        .route("/_/api/v1/logout", post(logout))
        .route("/_/api/v1/me/password", post(change_password))
        .route("/_/api/v1/me/token", post(own_token))
        .route("/_/api/v1/users/{id}/password", post(reset_password))
        .route("/_/api/v1/users/{id}/token", post(user_token))
}
