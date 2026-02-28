use axum::{Json, extract::State, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppState,
        dto::{request::*, response::*},
        error::ErrorResponse,
    },
    error::ErrorKind,
    ext::OpenApiRouterExt,
};

#[utoipa::path(get, path = "/")]
pub async fn index(State(_state): State<AppState>) -> impl IntoResponse {
    "Hello Axum!".to_string()
}

#[utoipa::path(post, path="/hello", request_body = HelloRequest, responses(
    (status = 200, body = HelloResponse))
)]
pub async fn hello(
    State(_state): State<AppState>,
    Json(payload): Json<HelloRequest>,
) -> impl IntoResponse {
    Json(HelloResponse {
        message: format!("Hello {}", payload.name),
    })
}

#[utoipa::path(get, path="/error", responses(
    (status = 200, body = ErrorResponse))
)]
pub async fn error() -> impl IntoResponse {
    ErrorKind::BadRequest.with_message("This is a bad request")
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes![index])
        .routes(routes![hello])
        .routes(routes![error])
        .with_tags(["chore"])
}
