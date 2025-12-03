use axum::{Json, extract::State, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::app::{
    AppState,
    dto::{request::*, response::*},
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

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(index))
        .routes(routes!(hello))
}
