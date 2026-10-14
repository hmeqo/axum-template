use std::sync::Arc;

use axum::{extract::Request, middleware::Next, response::Response};

use crate::{app::middleware::request_id::RequestId, error::AppError};

pub async fn log_app_errors(request: Request, next: Next) -> Response {
    let response = next.run(request).await;

    if let Some(err) = response.extensions().get::<Arc<AppError>>() {
        let request_id = response
            .extensions()
            .get::<RequestId>()
            .map(|id| id.0.as_str())
            .unwrap_or("-");
        tracing::error!(request_id, "Internal error: {err}");
    }

    response
}
