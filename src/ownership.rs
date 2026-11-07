use crate::{AppState, authenticated_user_in};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, post},
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transfer {
    version: i64,
    owner_id: i64,
}

async fn transfer(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    body: Result<Json<Transfer>, axum::extract::rejection::JsonRejection>,
) -> Result<
    (
        [(axum::http::HeaderName, &'static str); 1],
        Json<serde_json::Value>,
    ),
    StatusCode,
> {
    let mut db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let actor = authenticated_user_in(&tx, &headers, (state.clock)())?;
    if actor.role != "administrator" {
        return Err(StatusCode::FORBIDDEN);
    }
    let Json(body) = body.map_err(|_| StatusCode::BAD_REQUEST)?;
    if body.version < 1 || body.owner_id < 1 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (version, updated_at): (i64, i64) = tx
        .query_row(
            "SELECT version,updated_at FROM pages WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if version != body.version {
        return Err(StatusCode::CONFLICT);
    }
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id=?1)",
            [body.owner_id],
            |r| r.get(0),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !exists {
        return Err(StatusCode::NOT_FOUND);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .as_millis() as i64;
    tx.execute(
        "UPDATE pages SET owner_id=?1,version=version+1,updated_at=?2 WHERE id=?3",
        params![body.owner_id, now.max(updated_at + 1), id],
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.execute("DELETE FROM page_grants WHERE page_id=?1", [id])
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let page = tx.query_row("SELECT id,slug,markdown,owner_id,created_at,updated_at,version FROM pages WHERE id=?1",[id],|r|Ok(serde_json::json!({"id":r.get::<_,i64>(0)?,"slug":r.get::<_,String>(1)?,"markdown":r.get::<_,String>(2)?,"owner_id":r.get::<_,i64>(3)?,"created_at":r.get::<_,i64>(4)?,"updated_at":r.get::<_,i64>(5)?,"version":r.get::<_,i64>(6)?}))).map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    drop(db);
    tracing::info!(
        event = %"ownership_transferred",
        page_id = id,
        owner_id = body.owner_id,
        actor_id = actor.id,
    );
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(page),
    ))
}

async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let mut db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let actor = authenticated_user_in(&tx, &headers, (state.clock)())?;
    if actor.role != "administrator" {
        return Err(StatusCode::FORBIDDEN);
    }
    let role: String = tx
        .query_row("SELECT role FROM users WHERE id=?1", [id], |r| r.get(0))
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let pages: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pages WHERE owner_id=?1)",
            [id],
            |r| r.get(0),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let admins: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM users WHERE role='administrator'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if pages || (role == "administrator" && admins == 1) {
        return Err(StatusCode::CONFLICT);
    }
    tx.execute("DELETE FROM users WHERE id=?1", [id])
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tx.commit().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    drop(db);
    tracing::info!(event = %"user_deleted", user_id = id, actor_id = actor.id);
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/_/api/v1/pages/{id}/owner", post(transfer))
        .route("/_/api/v1/users/{id}", delete(delete_user))
}
