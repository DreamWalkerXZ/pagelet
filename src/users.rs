use crate::{
    Account, AppState, authenticated_user, hash_password, new_token, token_hash, validate_password,
    validate_username,
};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateUser {
    username: String,
    password: String,
    role: String,
}

#[derive(Serialize)]
pub(crate) struct CreatedUser {
    user: Account,
    token: String,
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<CreateUser>, axum::extract::rejection::JsonRejection>,
) -> Result<(StatusCode, Json<CreatedUser>), StatusCode> {
    require_administrator(&state, &headers)?;
    let Json(body) = body.map_err(|_| StatusCode::BAD_REQUEST)?;
    validate_username(&body.username).map_err(|_| StatusCode::BAD_REQUEST)?;
    validate_password(&body.password).map_err(|_| StatusCode::BAD_REQUEST)?;
    if !matches!(body.role.as_str(), "administrator" | "user") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let hash = hash_password(&body.password).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let token = new_token();
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db.execute(
        "INSERT INTO users (username, role, password_hash, token_hash) VALUES (?1, ?2, ?3, ?4)",
        params![body.username, body.role, hash, token_hash(&token)],
    )
    .map_err(|error| match error {
        rusqlite::Error::SqliteFailure(error, _)
            if error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
        {
            StatusCode::CONFLICT
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedUser {
            user: Account {
                id: db.last_insert_rowid(),
                username: body.username,
                role: body.role,
            },
            token,
        }),
    ))
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UserQuery {
    search: Option<String>,
    page: Option<u64>,
    per_page: Option<u64>,
}

#[derive(Serialize)]
pub(crate) struct UserList {
    users: Vec<Account>,
    total: i64,
    page: u64,
    per_page: u64,
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<UserQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<UserList>, StatusCode> {
    require_administrator(&state, &headers)?;
    let Query(query) = query.map_err(|_| StatusCode::BAD_REQUEST)?;
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = page
        .checked_sub(1)
        .and_then(|page| page.checked_mul(per_page))
        .and_then(|offset| i64::try_from(offset).ok())
        .ok_or(StatusCode::BAD_REQUEST)?;
    if per_page == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let search = query.search.unwrap_or_default();
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let result = (|| -> rusqlite::Result<UserList> {
        let total = db.query_row(
            "SELECT COUNT(*) FROM users WHERE instr(username, ?1) > 0",
            [&search],
            |row| row.get(0),
        )?;
        let mut statement = db.prepare("SELECT id, username, role FROM users WHERE instr(username, ?1) > 0 ORDER BY username, id LIMIT ?2 OFFSET ?3")?;
        let users = statement
            .query_map(params![search, per_page, offset], |row| {
                Ok(Account {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    role: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(UserList {
            users,
            total,
            page,
            per_page,
        })
    })();
    result
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn require_administrator(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    if authenticated_user(state, headers)?.role != "administrator" {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}
