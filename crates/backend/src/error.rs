use std::fmt::{Debug, Display};

use serde_json::Value;
use strum::{EnumString, IntoStaticStr};
use thiserror::Error;

type DynError = dyn std::error::Error + Send + Sync + 'static;
type BoxedDynError = Box<DynError>;

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

    // ========================================================
    // Auth
    // ========================================================
    #[strum(serialize = "auth.unauthorized")]
    Unauthorized,
    #[strum(serialize = "auth.forbidden")]
    Forbidden,
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
    #[strum(serialize = "err.external")]
    External,
    #[strum(serialize = "err.internal")]
    Internal,
}

impl ErrorKind {
    pub fn code(&self) -> &'static str {
        self.into()
    }

    pub fn default_message(&self) -> &'static str {
        match self {
            Self::DataParse => "Data parsing failed",
            Self::InvalidParameter => "Invalid parameter",
            Self::Unauthorized => "Unauthorized",
            Self::Forbidden => "Forbidden",
            Self::PermissionDenied => "Permission denied",
            Self::InvalidCredentials => "Invalid credentials",
            Self::ValidationFailed => "Validation failed",
            Self::BadRequest => "Bad request",

            Self::NotFound => "Resource not found",
            Self::AlreadyExists => "Resource already exists",

            Self::Config => "Configuration error",
            Self::External => "External service error",
            Self::Internal => "Internal server error",
        }
    }

    pub fn is_internal_error(&self) -> bool {
        matches!(self, Self::Config | Self::Internal)
    }

    pub fn to_error(self) -> AppError {
        AppError::new(self)
    }

    pub fn msg(self, msg: impl Into<String>) -> AppError {
        AppError::new(self).with_msg(msg)
    }

    pub fn err<E>(self, err: E) -> AppError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        AppError::new(self).with_err(err)
    }

    pub fn dyn_err(self, err: BoxedDynError) -> AppError {
        AppError::new(self).with_dyn_err(err)
    }

    pub fn err_msg<E>(self, err: E, msg: impl Into<String>) -> AppError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        AppError::new(self).with_msg(msg).with_err(err)
    }

    pub fn wrap_internal<E>(err: E) -> AppError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Internal.err(err)
    }
}

#[derive(Error)]
pub struct AppError {
    kind: ErrorKind,
    message: Option<String>,
    errors: Option<Value>,
    #[source]
    source: Option<BoxedDynError>,
}

impl AppError {
    fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            message: None,
            errors: None,
            source: None,
        }
    }

    fn with_msg(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    // Auto-set message from the source error, UNLESS it's an internal error.
    // Internal errors should not leak their details to the client.
    fn with_err<E>(mut self, err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        if self.message.is_none() && !self.kind.is_internal_error() {
            self.message = Some(err.to_string());
        }
        self.source = Some(Box::new(err));
        self
    }

    fn with_dyn_err(mut self, err: BoxedDynError) -> Self {
        if self.message.is_none() && !self.kind.is_internal_error() {
            self.message = Some(err.to_string());
        }
        self.source = Some(err);
        self
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    pub fn code(&self) -> &str {
        self.kind.into()
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
        if let Some(v) = err.downcast_ref::<validator::ValidationErrors>() {
            return Some(serde_json::to_value(v).unwrap_or(Value::Null));
        }
        None
    }

    pub fn trace_source(&self) {
        if self.kind.is_internal_error() {
            tracing::error!("Internal error: {}", self);
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
        write!(f, "[{}] {}", self.kind, self.message()).and_then(|_| {
            if let Some(err) = self.source.as_ref() {
                write!(f, "\nCause: {}", err)
            } else {
                Ok(())
            }
        })
    }
}

pub trait OptionAppExt<T> {
    fn ok_or_err(self, kind: ErrorKind) -> Result<T>;
    fn ok_or_err_msg(self, kind: ErrorKind, msg: impl Into<String>) -> Result<T>;
}

impl<T> OptionAppExt<T> for Option<T> {
    fn ok_or_err(self, kind: ErrorKind) -> Result<T> {
        self.ok_or_else(|| kind.to_error())
    }

    fn ok_or_err_msg(self, kind: ErrorKind, msg: impl Into<String>) -> Result<T> {
        self.ok_or_else(|| kind.msg(msg))
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

pub trait ResultExt<T> {
    fn err_kind(self, kind: ErrorKind) -> Result<T>;
    fn err_kind_msg(self, kind: ErrorKind, msg: impl Into<String>) -> Result<T>;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn err_kind(self, kind: ErrorKind) -> Result<T> {
        self.map_err(|e| kind.err(e))
    }

    fn err_kind_msg(self, kind: ErrorKind, msg: impl Into<String>) -> Result<T> {
        self.map_err(|e| kind.err_msg(e, msg))
    }
}

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
                    let kind = $kind;
                    $(
                        return kind.err_msg(e, $msg);
                    )?
                    #[allow(unreachable_code)]
                    kind.err(e)
                }
            }
        )*
    };
}

register_errors! {
    std::io::Error                                 => ErrorKind::Internal;
    serde_json::Error                              => ErrorKind::DataParse;
    config::ConfigError                            => ErrorKind::Config;
    toasty::Error                                  => ErrorKind::Internal;
    axum::extract::rejection::PathRejection        => ErrorKind::InvalidParameter;
    axum::extract::rejection::QueryRejection       => ErrorKind::InvalidParameter;
    axum::extract::rejection::JsonRejection        => ErrorKind::DataParse;
    validator::ValidationErrors                    => ErrorKind::ValidationFailed, "Validation failed";
}
