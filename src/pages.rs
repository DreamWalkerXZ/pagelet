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
    #[serde(skip)]
    access_password_hash: Option<String>,
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
        access_password_hash: row.get(7)?,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreatePage {
    markdown: String,
    slug: Option<String>,
    access_password: Option<String>,
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<CreatePage>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, StatusCode> {
    authenticated_user(&state, &headers)?;
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
    let password_hash = password_hash(body.access_password.as_deref())?;
    let mut db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let account = crate::authenticated_user_in(&tx, &headers, (state.clock)())?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .as_millis() as i64;
    loop {
        let slug = body
            .slug
            .clone()
            .unwrap_or_else(|| (state.slug_generator)());
        match tx.execute("INSERT INTO pages (slug, markdown, owner_id, created_at, updated_at, version, access_password_hash) VALUES (?1, ?2, ?3, ?4, ?4, 1, ?5)", params![slug, body.markdown, account.id, timestamp, password_hash]) {
            Ok(_) => {
                let page = Page { id: tx.last_insert_rowid(), slug, markdown: body.markdown, owner_id: account.id, created_at: timestamp, updated_at: timestamp, version: 1, access_password_hash: password_hash };
                tx.commit().map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
                drop(db);
                crate::log_event("INFO", format_args!("event=page_published page_id={} actor_id={}", page.id, account.id));
                return Ok((StatusCode::CREATED, [(header::CACHE_CONTROL, "no-store")], Json(page)).into_response());
            }
            Err(rusqlite::Error::SqliteFailure(error, _)) if error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE => {
                if body.slug.is_some() { return Err(StatusCode::CONFLICT); }
            }
            Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

fn find(state: &AppState, slug: &str, headers: &HeaderMap) -> Result<Page, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let page = db.query_row("SELECT id, slug, markdown, owner_id, created_at, updated_at, version, access_password_hash FROM pages WHERE slug = ?1", [slug], row_page).optional().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    if page.access_password_hash.is_none() {
        return Ok(page);
    }
    if crate::authenticated_user_in(&db, headers, (state.clock)())
        .is_ok_and(|a| a.id == page.owner_id || a.role == "administrator")
    {
        return Ok(page);
    }
    let name = format!("pagelet_grant_{}=", page.id);
    let token = headers
        .get("x-pagelet-grant")
        .and_then(|h| h.to_str().ok())
        .or_else(|| {
            headers
                .get("cookie")
                .and_then(|h| h.to_str().ok())
                .and_then(|c| c.split(';').find_map(|v| v.trim().strip_prefix(&name)))
        });
    if let Some(token) = token {
        let valid = db.query_row("SELECT EXISTS(SELECT 1 FROM page_grants WHERE token_hash=?1 AND page_id=?2 AND expires_at>?3)",params![crate::token_hash(token),page.id,(state.clock)()],|r|r.get::<_,bool>(0)).map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
        if valid {
            return Ok(page);
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}
fn read_error(status: StatusCode) -> Response {
    if status == StatusCode::UNAUTHORIZED {
        (
            status,
            Json(serde_json::json!({"error":"password_required"})),
        )
            .into_response()
    } else {
        status.into_response()
    }
}
pub(crate) async fn read(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    headers: HeaderMap,
) -> Response {
    match find(&state, &slug, &headers) {
        Ok(page) => ([(header::CACHE_CONTROL, "no-store")], Json(page)).into_response(),
        Err(status) => read_error(status),
    }
}
pub(crate) async fn raw(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    headers: HeaderMap,
) -> Response {
    match find(&state, &slug, &headers) {
        Ok(page) => (
            [
                (header::CACHE_CONTROL, "no-store"),
                (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            page.markdown,
        )
            .into_response(),
        Err(status) => read_error(status),
    }
}

fn password_hash(password: Option<&str>) -> Result<Option<String>, StatusCode> {
    match password {
        None | Some("") => Ok(None),
        Some(p) if p.chars().count() <= 128 => crate::hash_password(p)
            .map(Some)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR),
        _ => Err(StatusCode::BAD_REQUEST),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VerifyPassword {
    password: String,
}
pub(crate) async fn verify_password(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    peer: Result<
        axum::extract::ConnectInfo<std::net::SocketAddr>,
        axum::extract::rejection::ExtensionRejection,
    >,
    headers: HeaderMap,
    body: Result<Json<VerifyPassword>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, StatusCode> {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let Json(body) = body.map_err(|_| StatusCode::BAD_REQUEST)?;
    let (id, hash, version) = {
        let db = state
            .db
            .lock()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        db.query_row(
            "SELECT id,access_password_hash,version FROM pages WHERE slug=?1",
            [slug],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?
    };
    let now = (state.clock)();
    let source = state
        .deployment
        .source(peer.ok().map(|p| p.0.ip()), &headers)?;
    crate::sessions::check_rate(&state, "page-password", &source, &id.to_string(), now)?;
    let hash = hash.ok_or(StatusCode::BAD_REQUEST)?;
    if body.password.chars().count() > 128
        || !PasswordHash::new(&hash).is_ok_and(|h| {
            Argon2::default()
                .verify_password(body.password.as_bytes(), &h)
                .is_ok()
        })
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let grant = crate::new_token();
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // Recheck the verifier atomically so a concurrent policy change cannot issue a stale grant.
    let changed = db.execute("INSERT INTO page_grants(token_hash,page_id,expires_at) SELECT ?1,id,?2 FROM pages WHERE id=?3 AND access_password_hash=?4 AND version=?5",params![crate::token_hash(&grant),now+86400,id,hash,version]).map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if changed == 0 {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok((
        [
            (
                header::SET_COOKIE,
                format!(
                    "pagelet_grant_{id}={grant}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400{}",
                    state.deployment.cookie_suffix()
                ),
            ),
            (header::CACHE_CONTROL, "no-store".into()),
        ],
        Json(serde_json::json!({"grant":grant,"expires_at":now+86400})),
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
    let (page, per_page, offset) = crate::pagination(query.page, query.per_page)?;
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let search = query.search.unwrap_or_default();
    let result = (|| -> rusqlite::Result<serde_json::Value> {
        let total: i64 = db.query_row("SELECT COUNT(*) FROM pages WHERE (?1 IS NULL OR owner_id = ?1) AND instr(slug, ?2) > 0", params![owner, search], |r| r.get(0))?;
        let mut statement = db.prepare("SELECT id, slug, markdown, owner_id, created_at, updated_at, version, access_password_hash FROM pages WHERE (?1 IS NULL OR owner_id = ?1) AND instr(slug, ?2) > 0 ORDER BY updated_at DESC, id DESC LIMIT ?3 OFFSET ?4")?;
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
    access_password: Option<String>,
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
    let page = db.query_row("SELECT id, slug, markdown, owner_id, created_at, updated_at, version, access_password_hash FROM pages WHERE id = ?1", [id], row_page).optional().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
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
    let new_hash = password_hash(body.access_password.as_deref())?;
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
    if body.markdown.is_none() && body.slug.is_none() && body.access_password.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(markdown) = body.markdown {
        page.markdown = markdown;
    }
    if let Some(slug) = body.slug {
        page.slug = slug;
    }
    if body.access_password.is_some() {
        page.access_password_hash = new_hash;
        tx.execute("DELETE FROM page_grants WHERE page_id=?1", [id])
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    page.version += 1;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .as_millis() as i64;
    page.updated_at = now.max(page.updated_at + 1);
    tx.execute(
        "UPDATE pages SET markdown = ?1, slug = ?2, version = ?3, updated_at = ?4, access_password_hash = ?6 WHERE id = ?5",
        params![page.markdown, page.slug, page.version, page.updated_at, id, page.access_password_hash],
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
    drop(db);
    crate::log_event(
        "INFO",
        format_args!("event=page_deleted page_id={id} actor_id={}", account.id),
    );
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) fn valid_slug(slug: &str) -> bool {
    (1..=64).contains(&slug.len())
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && !slug.contains("--")
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
