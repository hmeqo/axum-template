//! 内嵌 SPA 与末位兜底路由。

use std::borrow::Cow;

use axum::{
    Router,
    body::Body,
    extract::OriginalUri,
    http::{StatusCode, header},
    response::Response,
    routing::get,
};
use rust_embed::RustEmbed;

use crate::app::AppContext;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

/// SPA 兜底：未命中任何路由的请求按静态资源解析，最终回落索引页。
pub async fn spa(OriginalUri(uri): OriginalUri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if path.is_empty() {
        return index().await;
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
            .expect("static asset response is well formed");
    }

    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return asset(mime.as_ref(), content.data);
    }

    if std::path::Path::new(path).extension().is_none() && !path.ends_with('/') {
        let index_path = format!("{}/index.html", path);
        if let Some(content) = Assets::get(&index_path) {
            return asset("text/html; charset=utf-8", content.data);
        }
    }

    index().await
}

fn asset(content_type: &str, body: Cow<'static, [u8]>) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .expect("static asset response is well formed")
}

async fn index() -> Response {
    match Assets::get("index.html") {
        Some(content) => asset("text/html; charset=utf-8", content.data),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("404 Not Found"))
            .expect("static asset response is well formed"),
    }
}

pub fn router() -> Router<AppContext> {
    Router::new().route("/", get(index))
}
