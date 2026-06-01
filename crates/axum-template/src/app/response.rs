use axum::{
    body::Body,
    http::{StatusCode, header},
    response::Response,
};

use crate::domain::service::StoredFile;

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
    stored: &StoredFile,
    cache: CacheControl,
    disposition: Disposition,
) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, &stored.file.mime_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "{}; filename=\"{}\"",
                disposition, stored.file.original_name
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
