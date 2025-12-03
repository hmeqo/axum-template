use axum::{
    Json,
    extract::rejection::{JsonRejection, PathRejection, QueryRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;
use strum::{AsRefStr, Display};
use thiserror::Error;
use tower_sessions_seaorm_store::SeaOrmStoreError;
use utoipa::ToSchema;
use validator::ValidationErrors;

#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, Display)]
#[strum(serialize_all = "snake_case")]
pub enum ErrorKind {
    // Auth
    #[strum(serialize = "auth.unauthenticated")]
    Unauthenticated,
    #[strum(serialize = "auth.invalid_credentials")]
    InvalidCredentials,
    #[strum(serialize = "auth.permission_denied")]
    PermissionDenied,

    // User
    #[strum(serialize = "user.not_found")]
    UserNotFound,
    #[strum(serialize = "user.exists")]
    UserExists,

    // Generic
    #[strum(serialize = "err.not_found")]
    NotFound,
    #[strum(serialize = "err.exists")]
    Exists,
    #[strum(serialize = "err.conflict")]
    Conflict,
    #[strum(serialize = "err.bad_request")]
    BadRequest,

    /// Do not use directly in business logic.
    /// Use `AppError::internal()` instead for internal server errors.
    #[strum(serialize = "err.internal")]
    Internal,
}

impl ErrorKind {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthenticated | Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::PermissionDenied => StatusCode::FORBIDDEN,
            Self::UserNotFound | Self::NotFound => StatusCode::NOT_FOUND,
            Self::Exists | Self::UserExists | Self::Conflict => StatusCode::CONFLICT,
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn default_message(&self) -> &'static str {
        match self {
            Self::Unauthenticated => "Authentication required",
            Self::InvalidCredentials => "Invalid username or password",
            Self::PermissionDenied => "Permission denied",
            Self::UserNotFound => "User not found",
            Self::UserExists => "User already exists",
            Self::NotFound => "Resource not found",
            Self::Exists => "Resource already exists",
            Self::Conflict => "Operation conflict",
            Self::BadRequest => "Bad request",
            Self::Internal => "Internal server error",
        }
    }

    pub fn into_response(self) -> ErrorResponse {
        ErrorResponse::new(self).with_detail(self.default_message())
    }

    pub fn with_detail(self, detail: impl Into<String>) -> ErrorResponse {
        ErrorResponse::new(self).with_detail(detail)
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    #[serde(skip)]
    status_code: StatusCode,
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Value>,
}

impl ErrorResponse {
    pub fn new(code: ErrorKind) -> Self {
        Self {
            status_code: code.status_code(),
            code: code.to_string(),
            detail: None,
            errors: None,
        }
    }

    pub fn status_code(&self) -> StatusCode {
        self.status_code
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_errors(mut self, errors: Value) -> Self {
        self.errors = Some(errors);
        self
    }

    pub fn with_status_code(mut self, code: StatusCode) -> Self {
        self.status_code = code;
        self
    }

    #[inline]
    pub fn json(self) -> Json<Self> {
        Json(self)
    }

    pub fn into_error(self) -> AppError {
        self.into()
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Error")]
    Response(ErrorResponse),

    // Extraction and validation errors
    #[error(transparent)]
    PathRejection(#[from] PathRejection),

    #[error(transparent)]
    QueryRejection(#[from] QueryRejection),

    #[error(transparent)]
    JsonRejection(#[from] JsonRejection),

    #[error(transparent)]
    Validation(#[from] ValidationErrors),

    // Infrastructure errors
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Db(#[from] sea_orm::DbErr),

    #[error("SeaOrmStore error: {0}")]
    SeaOrmStore(#[from] SeaOrmStoreError),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl From<ErrorKind> for AppError {
    fn from(value: ErrorKind) -> Self {
        Self::Response(value.into_response())
    }
}

impl From<ErrorResponse> for AppError {
    fn from(value: ErrorResponse) -> Self {
        Self::Response(value)
    }
}

impl AppError {
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Wraps any error into an `AppError::Internal`
    pub fn wrap(e: impl std::error::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

impl AppError {
    fn log_internal(&self) {
        match self {
            Self::Config(e) => tracing::error!(error = %e),
            Self::Io(e) => tracing::error!(error = %e),
            Self::Db(e) => tracing::error!(error = %e),
            Self::SeaOrmStore(e) => tracing::error!(error = %e),
            Self::Internal(msg) => tracing::error!(error = %msg),
            _ => {}
        }
    }

    fn into_error_response(self) -> ErrorResponse {
        match self {
            Self::Response(e) => e,
            Self::Validation(errors) => ErrorKind::BadRequest
                .into_response()
                .with_status_code(StatusCode::UNPROCESSABLE_ENTITY)
                .with_errors(serde_json::to_value(errors).unwrap_or(Value::Null)),
            Self::PathRejection(e) => ErrorKind::BadRequest.with_detail(e.to_string()),
            Self::QueryRejection(e) => ErrorKind::BadRequest.with_detail(e.to_string()),
            Self::JsonRejection(e) => ErrorKind::BadRequest.with_detail(e.to_string()),
            Self::Serde(e) => ErrorKind::BadRequest.with_detail(e.to_string()),
            Self::Config(_)
            | Self::Io(_)
            | Self::Db(_)
            | Self::SeaOrmStore(_)
            | Self::Internal(_) => ErrorKind::Internal.into_response(),
        }
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (self.status_code, self.json()).into_response()
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        self.log_internal();
        self.into_error_response().into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
