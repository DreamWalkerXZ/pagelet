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
        .route("/_/assets/{*path}", get(asset))
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
