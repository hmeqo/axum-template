use std::collections::HashMap;

use thiserror::Error;

mod code;
mod mapping;

pub use code::ErrorCode;

pub(super) type BoxedDynError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub type FieldErrors = HashMap<String, Vec<String>>;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppError {
    #[error("{message}")]
    Business {
        code: ErrorCode,
        message: String,
        errors: Option<FieldErrors>,
    },

    #[error("{0}")]
    MalformedRequest(String),
    #[error("{0}")]
    Unauthenticated(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("service temporarily unavailable")]
    Unavailable {
        #[source]
        source: BoxedDynError,
    },

    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    Database(toasty::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Inquire(#[from] inquire::error::InquireError),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
    #[error("{0}")]
    Internal(String),
    #[error(transparent)]
    Any(#[from] BoxedDynError),
}

impl AppError {
    pub fn business(code: ErrorCode) -> Self {
        Self::Business {
            message: code.message().to_owned(),
            code,
            errors: None,
        }
    }

    pub fn business_msg(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Business {
            code,
            message: message.into(),
            errors: None,
        }
    }

    pub fn malformed_request(msg: impl Into<String>) -> Self {
        Self::MalformedRequest(msg.into())
    }

    pub fn unauthenticated(msg: impl Into<String>) -> Self {
        Self::Unauthenticated(msg.into())
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(config::ConfigError::Message(msg.into()))
    }

    pub fn wrap(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Any(Box::new(err))
    }

    pub fn code(&self) -> Option<ErrorCode> {
        match self {
            Self::Business { code, .. } => Some(*code),
            _ => None,
        }
    }

    pub fn field_errors(&self) -> Option<&FieldErrors> {
        match self {
            Self::Business { errors, .. } => errors.as_ref(),
            _ => None,
        }
    }
}

pub type Result<T, E = AppError> = std::result::Result<T, E>;

#[macro_export]
macro_rules! bail {
    ($err:expr $(,)?) => {
        return ::core::result::Result::Err($err)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn business_defaults_to_the_code_message() {
        let err = AppError::business(ErrorCode::UserUsernameTaken);
        assert_eq!(err.to_string(), "Username already exists");
        assert_eq!(err.code(), Some(ErrorCode::UserUsernameTaken));
    }

    #[test]
    fn business_msg_overrides_the_default() {
        let err = AppError::business_msg(ErrorCode::FileTooLarge, "File exceeds 20 MiB");
        assert_eq!(err.to_string(), "File exceeds 20 MiB");
        assert_eq!(err.code(), Some(ErrorCode::FileTooLarge));
    }

    #[test]
    fn transport_and_internal_failures_carry_no_code() {
        assert_eq!(AppError::not_found("gone").code(), None);
        assert_eq!(AppError::internal("boom").code(), None);
        assert_eq!(AppError::malformed_request("bad json").code(), None);
    }

    #[test]
    fn field_errors_only_on_the_validation_code() {
        let err = AppError::from(validator::ValidationErrors::new());
        assert!(err.field_errors().is_some());
        assert!(AppError::internal("m").field_errors().is_none());
    }
}
