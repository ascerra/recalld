//! The built SPA (`ui/dist`), embedded in release builds and read from disk
//! in debug builds. Unknown non-API paths fall back to `index.html`.

use axum::{
    http::{HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};

#[derive(rust_embed::Embed)]
#[folder = "ui/dist"]
#[allow_missing = true]
struct Assets;

/// Whether the UI was built into (or next to) this binary.
pub fn available() -> bool {
    Assets::get("index.html").is_some()
}

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let (file, path) = match Assets::get(path) {
        Some(file) if !path.is_empty() => (file, path),
        // SPA fallback: routes like /map have no file of their own.
        _ if path.is_empty() || !path.rsplit('/').next().unwrap_or("").contains('.') => {
            match Assets::get("index.html") {
                Some(file) => (file, "index.html"),
                None => return not_built(),
            }
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let cache = if path.starts_with("_app/immutable/") {
        "public, max-age=31536000, immutable"
    } else if path == "index.html" {
        "no-store"
    } else {
        "no-cache"
    };
    let mut response = file.data.into_owned().into_response();
    let headers = response.headers_mut();
    if let Ok(mime) = HeaderValue::from_str(file.metadata.mimetype()) {
        headers.insert(header::CONTENT_TYPE, mime);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

fn not_built() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "The recalld UI was not built into this binary. Run `make ui`, then rebuild with `--features ui`.\n",
    )
        .into_response()
}
