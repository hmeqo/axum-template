use std::fmt::Debug;

use axum::{
    Json,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

use crate::error::AppError;

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Value>,
}

impl ErrorResponse {
    pub fn from_error(error: &AppError) -> Self {
        Self {
            code: error.code().into(),
            detail: Some(error.message().into()),
            errors: error.errors(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if self.kind().is_internal_error() {
            self.trace_source();
        }
        let status_code = self.status_code();
        let response = ErrorResponse::from_error(&self);
        (status_code, Json(response)).into_response()
    }
}
