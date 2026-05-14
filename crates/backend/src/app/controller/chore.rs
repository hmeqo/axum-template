use axum::{Json, extract::State, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppState,
        dto::{request::*, response::*},
        error::ErrorResp,
    },
    error::ErrorKind,
    ext::OpenApiRouterExt,
};

#[utoipa::path(get, path = "/")]
pub async fn index(State(_state): State<AppState>) -> impl IntoResponse {
    "Hello Axum!".to_string()
}

#[utoipa::path(post, path="/hello", request_body = HelloReq, responses(
    (status = 200, body = HelloResp))
)]
pub async fn hello(
    State(_state): State<AppState>,
    Json(payload): Json<HelloReq>,
) -> impl IntoResponse {
    Json(HelloResp {
        message: format!("Hello {}", payload.name),
    })
}

#[utoipa::path(get, path="/error", responses(
    (status = 200, body = ErrorResp))
)]
pub async fn error() -> impl IntoResponse {
    ErrorKind::BadRequest.msg("This is a bad request")
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes![index])
        .routes(routes![hello])
        .routes(routes![error])
        .with_tags(["chore"])
}
