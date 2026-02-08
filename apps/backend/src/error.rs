use std::fmt::{Debug, Display};

use axum::{
    extract::rejection::{JsonRejection, PathRejection, QueryRejection},
    http::StatusCode,
};
use serde_json::Value;
use strum::{EnumString, IntoStaticStr};
use thiserror::Error;
use tower_sessions_seaorm_store::SeaOrmStoreError;
use validator::ValidationErrors;

type AnyError = dyn std::error::Error + Send + Sync + 'static;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, strum::Display, IntoStaticStr)]
pub enum ErrorKind {
    // ========================================================
    // Request & Data
    // ========================================================
    #[strum(serialize = "data.bad_request")]
    BadRequest,
    #[strum(serialize = "data.parse")]
    DataParse,
    #[strum(serialize = "data.validation_failed")]
    ValidationFailed,
    #[strum(serialize = "data.invalid")]
    InvalidParameter,

    #[strum(serialize = "auth.unauthorized")]
    Unauthorized,
    #[strum(serialize = "auth.permission_denied")]
    PermissionDenied,
    #[strum(serialize = "auth.invalid_credentials")]
    InvalidCredentials,

    // ========================================================
    // Resource
    // ========================================================
    #[strum(serialize = "res.not_found")]
    NotFound,
    #[strum(serialize = "res.already_exists")]
    AlreadyExists,

    // ========================================================
    // System & Environment
    // ========================================================
    #[strum(serialize = "sys.config")]
    Config,
    #[strum(serialize = "err.internal")]
    Internal,
}

impl ErrorKind {
    pub fn code(&self) -> &'static str {
        self.into()
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest
            | Self::DataParse
            | Self::InvalidParameter
            | Self::ValidationFailed => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::PermissionDenied | Self::InvalidCredentials => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::AlreadyExists => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn default_message(&self) -> &'static str {
        match self {
            Self::DataParse => "Data parsing failed",
            Self::InvalidParameter => "Invalid parameter",
            Self::Unauthorized => "Unauthorized",
            Self::PermissionDenied => "Permission denied",
            Self::InvalidCredentials => "Invalid credentials",
            Self::ValidationFailed => "Validation failed",
            Self::BadRequest => "Bad request",

            Self::NotFound => "Resource not found",
            Self::AlreadyExists => "Resource already exists",

            Self::Config => "Configuration error",
            _ => "Internal server error",
        }
    }

    pub fn is_internal_error(&self) -> bool {
        matches!(self, Self::Config | Self::Internal)
    }
}

impl ErrorKind {
    pub fn to_error(self) -> AppError {
        AppError {
            kind: self,
            message: None,
            errors: None,
            source: None,
        }
    }

    pub fn with_message(self, msg: impl Into<String>) -> AppError {
        AppError {
            kind: self,
            message: Some(msg.into()),
            errors: None,
            source: None,
        }
    }

    /// Wraps any error into an AppError of this kind
    pub fn with_error<E>(self, err: E) -> AppError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        AppError {
            kind: self,
            message: None,
            errors: None,
            source: Some(Box::new(err)),
        }
    }

    /// Wraps any error into an AppError of this kind with a custom message
    pub fn with_source<E>(self, err: E, msg: impl Into<String>) -> AppError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        AppError {
            kind: self,
            message: Some(msg.into()),
            errors: None,
            source: Some(Box::new(err)),
        }
    }

    pub fn wrap_internal<E>(e: E) -> AppError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        ErrorKind::Internal.with_error(e)
    }
}

#[derive(Error)]
pub struct AppError {
    kind: ErrorKind,

    message: Option<String>,

    errors: Option<Value>,

    #[source]
    source: Option<Box<AnyError>>,
}

impl AppError {
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    pub fn code(&self) -> &str {
        self.kind.into()
    }

    pub fn status_code(&self) -> StatusCode {
        self.kind.status_code()
    }

    pub fn message(&self) -> &str {
        self.message
            .as_deref()
            .unwrap_or_else(|| self.kind.default_message())
    }

    pub fn errors(&self) -> Option<Value> {
        if let Some(errors) = self.errors.as_ref() {
            return Some(errors.clone());
        }
        let err = self.source.as_ref()?;
        err.downcast_ref::<ValidationErrors>()
            .map(|err| serde_json::to_value(err).unwrap_or(Value::Null))
            .or(err
                .downcast_ref::<PathRejection>()
                .map(|err| Value::String(err.to_string())))
    }

    pub fn trace_source(&self) {
        if let Some(err) = self.source.as_ref() {
            tracing::error!(error = err)
        }
    }
}

impl Debug for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppError")
            .field("code", &self.code())
            .field("message", &self.message())
            .field("source", &self.source)
            .finish()
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.kind, self.message())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

impl From<ErrorKind> for AppError {
    fn from(kind: ErrorKind) -> Self {
        kind.to_error()
    }
}

macro_rules! register_errors {
    ( $( $err_type:ty => $kind:expr $(, $msg:literal)? );* $(;)? ) => {
        $(
            impl From<$err_type> for AppError {
                fn from(e: $err_type) -> Self {
                    let err = $kind;
                    $(
                        return err.with_source(e, $msg);
                    )?
                    #[allow(unreachable_code)]
                    err.with_error(e)
                }
            }
        )*
    };
}

// 注册错误转换
register_errors! {
    std::io::Error      => ErrorKind::Internal;
    serde_json::Error   => ErrorKind::DataParse;
    config::ConfigError => ErrorKind::Config;
    sea_orm::DbErr      => ErrorKind::Internal;
    SeaOrmStoreError    => ErrorKind::Internal;
    PathRejection       => ErrorKind::InvalidParameter;
    QueryRejection      => ErrorKind::InvalidParameter;
    JsonRejection       => ErrorKind::DataParse;
    ValidationErrors    => ErrorKind::ValidationFailed;
}
