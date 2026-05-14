use std::fmt::Debug;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

use crate::error::{AppError, ErrorKind};

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResp {
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object)]
    errors: Option<Value>,
}

impl ErrorResp {
    pub fn from_error(error: &AppError) -> Self {
        Self {
            code: error.code().into(),
            detail: Some(error.message().into()),
            errors: error.errors(),
        }
    }
}

fn error_status_code(kind: &ErrorKind) -> StatusCode {
    match kind {
        ErrorKind::BadRequest
        | ErrorKind::DataParse
        | ErrorKind::InvalidParameter
        | ErrorKind::ValidationFailed => StatusCode::BAD_REQUEST,
        ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
        ErrorKind::Forbidden | ErrorKind::PermissionDenied | ErrorKind::InvalidCredentials => {
            StatusCode::FORBIDDEN
        }
        ErrorKind::NotFound => StatusCode::NOT_FOUND,
        ErrorKind::AlreadyExists => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        self.trace_source();
        let status_code = error_status_code(self.kind());
        let response = ErrorResp::from_error(&self);
        (status_code, Json(response)).into_response()
    }
}
