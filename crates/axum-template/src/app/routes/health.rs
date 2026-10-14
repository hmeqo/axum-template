//! `/health`、`/health/ready`：存活与就绪探针，公开。

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};

use crate::app::AppContext;

async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

async fn ready(State(ctx): State<AppContext>) -> impl IntoResponse {
    match ctx.db().connection().await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ready" })),
        ),
        Err(e) => {
            tracing::warn!("readiness probe failed: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({ "status": "unavailable" })),
            )
        }
    }
}

pub fn router() -> Router<AppContext> {
    Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(ready))
}
