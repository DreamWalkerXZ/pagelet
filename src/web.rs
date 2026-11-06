use crate::AppState;
use axum::{
    Router,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/{slug}", get(reader))
        .route("/_/app", get(app_root))
        .route("/_/app/", get(app_root))
        .route("/_/app/{*path}", get(app))
        .route("/_/assets/{*path}", get(asset))
}
async fn app_root() -> Response {
    axum::response::Redirect::to("/_/app/new").into_response()
}
async fn app(Path(path): Path<String>) -> Response {
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
    let editor = path
        .strip_prefix("pages/")
        .and_then(|p| p.strip_suffix("/edit"))
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|c| c.is_ascii_digit()));
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
