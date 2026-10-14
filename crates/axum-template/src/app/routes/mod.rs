//! 路由组清单与 `/api` 未命中时的 JSON 404。

pub mod auth;
pub mod error;
pub mod files;
pub mod health;
pub mod jwt;
pub mod media;
pub mod static_site;
pub mod user;

use axum::response::{IntoResponse, Response};

use crate::error::AppError;

/// `/api` 未命中路径的兜底：JSON 404。
pub async fn api_not_found() -> Response {
    AppError::not_found("Not found").into_response()
}
