//! `/api/error`：把错误契约挂在一条具体状态码的响应上，公开。

use axum::{Json, http::StatusCode, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppContext,
        http::{
            error::{ErrorResp, ErrorResponses, respond},
            openapi::OpenApiRouterExt,
        },
    },
    error::{AppError, ErrorCode},
};

/// Returns the error body shape with `200`, so a client built from this document emits `ErrorResp`
/// and `ErrorCode`: the wildcard `4XX` / `5XX` responses of every other operation are dropped by
/// some generators.
#[utoipa::path(get, path="/", responses(ErrorResponses, (status = 200, body = ErrorResp)))]
pub async fn sample() -> impl IntoResponse {
    // 样张取自错误映射本身，文案与业务码不会和真实响应漂移。
    let (_, body) = respond(&AppError::business(ErrorCode::InputInvalidFields));
    (StatusCode::OK, Json(body))
}

pub const PATH: &str = "/error";

pub fn router() -> OpenApiRouter<AppContext> {
    OpenApiRouter::new()
        .routes(routes![sample])
        .with_tags(["error"])
}
