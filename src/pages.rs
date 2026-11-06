use crate::{AppState, authenticated_user};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use rand::Rng;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct Page {
    id: i64,
    slug: String,
    markdown: String,
    owner_id: i64,
    created_at: i64,
    updated_at: i64,
    version: i64,
}

fn row_page(row: &rusqlite::Row<'_>) -> rusqlite::Result<Page> {
    Ok(Page {
        id: row.get(0)?,
        slug: row.get(1)?,
        markdown: row.get(2)?,
        owner_id: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        version: row.get(6)?,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreatePage {
    markdown: String,
    slug: Option<String>,
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<CreatePage>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, StatusCode> {
    let account = authenticated_user(&state, &headers)?;
    let Json(body) = body.map_err(|error| {
        if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
            StatusCode::PAYLOAD_TOO_LARGE
        } else {
            StatusCode::BAD_REQUEST
        }
    })?;
    if body.markdown.len() > 1024 * 1024 {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    if body.slug.as_ref().is_some_and(|slug| !valid_slug(slug)) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .as_millis() as i64;
    loop {
        let slug = body
            .slug
            .clone()
            .unwrap_or_else(|| (state.slug_generator)());
        match db.execute("INSERT INTO pages (slug, markdown, owner_id, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?4, 1)", params![slug, body.markdown, account.id, timestamp]) {
            Ok(_) => {
                let page = Page { id: db.last_insert_rowid(), slug, markdown: body.markdown, owner_id: account.id, created_at: timestamp, updated_at: timestamp, version: 1 };
                return Ok((StatusCode::CREATED, [(header::CACHE_CONTROL, "no-store")], Json(page)).into_response());
            }
            Err(rusqlite::Error::SqliteFailure(error, _)) if error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE => {
                if body.slug.is_some() { return Err(StatusCode::CONFLICT); }
            }
            Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

fn find(state: &AppState, slug: &str) -> Result<Page, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db.query_row("SELECT id, slug, markdown, owner_id, created_at, updated_at, version FROM pages WHERE slug = ?1", [slug], row_page).optional().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)
}

pub(crate) async fn read(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Response, StatusCode> {
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(find(&state, &slug)?),
    )
        .into_response())
}

pub(crate) async fn raw(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Response, StatusCode> {
    Ok((
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        find(&state, &slug)?.markdown,
    )
        .into_response())
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PageQuery {
    search: Option<String>,
    owner_id: Option<i64>,
    page: Option<u64>,
    per_page: Option<u64>,
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<PageQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let account = authenticated_user(&state, &headers)?;
    let Query(query) = query.map_err(|_| StatusCode::BAD_REQUEST)?;
    if account.role != "administrator" && query.owner_id.is_some_and(|id| id != account.id) {
        return Err(StatusCode::FORBIDDEN);
    }
    let owner = if account.role == "administrator" {
        query.owner_id
    } else {
        Some(account.id)
    };
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = page
        .checked_sub(1)
        .and_then(|p| p.checked_mul(per_page))
        .and_then(|o| i64::try_from(o).ok())
        .filter(|_| per_page != 0)
        .ok_or(StatusCode::BAD_REQUEST)?;
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let search = query.search.unwrap_or_default();
    let result = (|| -> rusqlite::Result<serde_json::Value> {
        let total: i64 = db.query_row("SELECT COUNT(*) FROM pages WHERE (?1 IS NULL OR owner_id = ?1) AND instr(slug, ?2) > 0", params![owner, search], |r| r.get(0))?;
        let mut statement = db.prepare("SELECT id, slug, markdown, owner_id, created_at, updated_at, version FROM pages WHERE (?1 IS NULL OR owner_id = ?1) AND instr(slug, ?2) > 0 ORDER BY updated_at DESC, id DESC LIMIT ?3 OFFSET ?4")?;
        let pages = statement.query_map(params![owner, search, per_page, offset], |row| {
            Ok(serde_json::json!({"id":row.get::<_,i64>(0)?,"slug":row.get::<_,String>(1)?,"owner_id":row.get::<_,i64>(3)?,"created_at":row.get::<_,i64>(4)?,"updated_at":row.get::<_,i64>(5)?,"version":row.get::<_,i64>(6)?}))
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(serde_json::json!({"pages":pages,"total":total,"page":page,"per_page":per_page}))
    })();
    result
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub(crate) fn random_slug() -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rngs::OsRng;
    (0..12)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditPage {
    version: i64,
    markdown: Option<String>,
    slug: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeletePage {
    version: i64,
}

fn management_page(
    db: &rusqlite::Connection,
    id: i64,
    account: &crate::Account,
) -> Result<Page, StatusCode> {
    let page = db.query_row("SELECT id, slug, markdown, owner_id, created_at, updated_at, version FROM pages WHERE id = ?1", [id], row_page).optional().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    if page.owner_id != account.id && account.role != "administrator" {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(page)
}

pub(crate) async fn management_read(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let account = crate::authenticated_user_in(&db, &headers, (state.clock)())?;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(management_page(&db, id, &account)?),
    )
        .into_response())
}

pub(crate) async fn edit(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    body: Result<Json<EditPage>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, StatusCode> {
    let Json(body) = body.map_err(|e| {
        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            StatusCode::PAYLOAD_TOO_LARGE
        } else {
            StatusCode::BAD_REQUEST
        }
    })?;
    if body
        .markdown
        .as_ref()
        .is_some_and(|text| text.len() > 1024 * 1024)
    {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    if body.slug.as_ref().is_some_and(|slug| !valid_slug(slug)) || body.version < 1 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let account = crate::authenticated_user_in(&tx, &headers, (state.clock)())?;
    let mut page = management_page(&tx, id, &account)?;
    if page.version != body.version {
        return Err(StatusCode::CONFLICT);
    }
    if body.markdown.is_none() && body.slug.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(markdown) = body.markdown {
        page.markdown = markdown;
    }
    if let Some(slug) = body.slug {
        page.slug = slug;
    }
    page.version += 1;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .as_millis() as i64;
    page.updated_at = now.max(page.updated_at + 1);
    tx.execute(
        "UPDATE pages SET markdown = ?1, slug = ?2, version = ?3, updated_at = ?4 WHERE id = ?5",
        params![page.markdown, page.slug, page.version, page.updated_at, id],
    )
    .map_err(|error| match error {
        rusqlite::Error::SqliteFailure(error, _)
            if error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
        {
            StatusCode::CONFLICT
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;
    tx.commit().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(page)).into_response())
}

pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    body: Result<Json<DeletePage>, axum::extract::rejection::JsonRejection>,
) -> Result<StatusCode, StatusCode> {
    let Json(body) = body.map_err(|_| StatusCode::BAD_REQUEST)?;
    if body.version < 1 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let account = crate::authenticated_user_in(&tx, &headers, (state.clock)())?;
    let page = management_page(&tx, id, &account)?;
    if page.version != body.version {
        return Err(StatusCode::CONFLICT);
    }
    tx.execute("DELETE FROM pages WHERE id = ?1", [id])
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

fn valid_slug(slug: &str) -> bool {
    (1..=64).contains(&slug.len())
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && !slug.contains("--")
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
