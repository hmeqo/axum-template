use axum::{
    Router,
    body::Body,
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::RustEmbed;

use crate::app::AppState;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

async fn serve_static(Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');

    if path.is_empty() {
        return index().await.into_response();
    }

    if let Some(prefix) = path.strip_suffix("index.html") {
        let redirect_path = if prefix.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", prefix.trim_end_matches('/'))
        };
        return Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, redirect_path)
            .body(Body::empty())
            .unwrap();
    }

    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content.data))
            .unwrap();
    }

    if std::path::Path::new(path).extension().is_none() && !path.ends_with('/') {
        let index_path = format!("{}/index.html", path);
        if let Some(content) = Assets::get(&index_path) {
            return Response::builder()
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(content.data))
                .unwrap();
        }
    }

    index().await.into_response()
}

async fn index() -> impl IntoResponse {
    match Assets::get("index.html") {
        Some(content) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(content.data))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("404 Not Found"))
            .unwrap(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/{*path}", get(serve_static))
}
