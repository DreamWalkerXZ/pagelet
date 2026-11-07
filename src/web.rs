use crate::AppState;
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(app_root))
        .route("/{slug}", get(reader))
        .route("/_/app", get(app_root))
        .route("/_/app/", get(app_root))
        .route("/_/app/{*path}", get(app))
        .route("/_/assets/{*path}", get(asset))
}
async fn app_root() -> Response {
    Redirect::to("/_/app/pages").into_response()
}
async fn app(
    State(state): State<AppState>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> Response {
    let editor = path
        .strip_prefix("pages/")
        .and_then(|p| p.strip_suffix("/edit"))
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|c| c.is_ascii_digit()));
    if path != "login"
        && (editor
            || matches!(
                path.as_str(),
                "new" | "pages" | "account" | "admin/pages" | "admin/users"
            ))
    {
        match crate::authenticated_user(&state, &headers) {
            Ok(_) => {}
            Err(StatusCode::UNAUTHORIZED) => return Redirect::to("/_/app/login").into_response(),
            Err(status) => return status.into_response(),
        }
    }
    if matches!(path.as_str(), "pages" | "account") {
        return content(
            "text/html; charset=utf-8",
            include_bytes!("../web/account.html"),
        );
    }
    if matches!(path.as_str(), "admin/pages" | "admin/users") {
        return content(
            "text/html; charset=utf-8",
            include_bytes!("../web/admin.html"),
        );
    }
    if !matches!(path.as_str(), "login" | "new") && !editor {
        return StatusCode::NOT_FOUND.into_response();
    }
    content(
        "text/html; charset=utf-8",
        include_bytes!("../web/app.html"),
    )
}
async fn reader(Path(slug): Path<String>) -> Response {
    if !crate::pages::valid_slug(&slug) {
        return StatusCode::NOT_FOUND.into_response();
    }
    content(
        "text/html; charset=utf-8",
        include_bytes!("../web/reader.html"),
    )
}
fn content(kind: &'static str, body: &'static [u8]) -> Response {
    ([
        ("content-type", kind),
        ("x-content-type-options", "nosniff"),
        ("content-security-policy", "default-src 'none'; script-src 'self'; style-src 'self'; img-src http: https:; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"),
        ("referrer-policy", "no-referrer"),
    ], body).into_response()
}
async fn asset(Path(path): Path<String>) -> Response {
    let (kind, bytes): (&str, &'static [u8]) = match path.as_str() {
        "ui.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../web/ui.js"),
        ),
        "editor-preview.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../web/editor-preview.js"),
        ),
        "account.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../web/account.js"),
        ),
        "admin.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../web/admin.js"),
        ),
        "app.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../web/app.js"),
        ),
        "api.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../web/api.js"),
        ),
        "app.css" => ("text/css; charset=utf-8", include_bytes!("../web/app.css")),
        "reader.css" => (
            "text/css; charset=utf-8",
            include_bytes!("../web/reader.css"),
        ),
        "reader.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../web/reader.js"),
        ),
        "renderer.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../web/renderer.js"),
        ),
        "vendor/marked.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../web/vendor/marked.js"),
        ),
        "vendor/purify.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../web/vendor/purify.js"),
        ),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    content(kind, bytes)
}
