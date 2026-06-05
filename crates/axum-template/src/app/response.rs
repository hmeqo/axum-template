use axum::{
    body::Body,
    http::{StatusCode, header},
    response::Response,
};

// ── Cache-Control helpers ──

#[derive(Clone, Copy, strum::Display)]
pub enum CacheControl {
    #[strum(to_string = "public, max-age=31536000, immutable")]
    Year,
    #[strum(to_string = "public, max-age=86400")]
    Day,
    #[strum(to_string = "no-store")]
    NoStore,
}

// ── Content-Disposition helpers ──

#[derive(Clone, Copy, strum::Display)]
pub enum Disposition {
    #[strum(to_string = "inline")]
    Inline,
    #[strum(to_string = "attachment")]
    Attachment,
}

// ── Response builders ──

pub fn bytes(
    data: &[u8],
    mime_type: &str,
    original_name: &str,
    cache: CacheControl,
    disposition: Disposition,
) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, mime_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "{}; filename=\"{}\"",
                disposition,
                original_name.replace('"', "\\\"")
            ),
        )
        .header(header::CACHE_CONTROL, cache.to_string())
        .body(Body::from(data.to_vec()))
        .unwrap()
}

pub fn not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}
