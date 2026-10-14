//! 唯一知道 HTTP 存在的地方：领域侧只产出 [`AppError`]。

use std::sync::Arc;

use axum::{
    Json,
    extract::rejection::{JsonRejection, PathRejection, QueryRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::{IntoResponses, ToSchema};

use crate::error::{AppError, ErrorCode, FieldErrors};

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResp {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<ErrorCode>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object)]
    pub errors: Option<FieldErrors>,
}

#[derive(IntoResponses)]
pub enum ErrorResponses {
    #[response(
        status = "4XX",
        description = "The request was rejected; `code` is present when the failure is actionable"
    )]
    ClientError(ErrorResp),
    #[response(
        status = "5XX",
        description = "The server failed; the body carries a generic message and no `code`"
    )]
    ServerError(ErrorResp),
}

impl ErrorCode {
    fn status(self) -> StatusCode {
        match self {
            Self::InputInvalidFields => StatusCode::BAD_REQUEST,
            Self::UserInvalidOldPassword => StatusCode::BAD_REQUEST,
            Self::AuthInvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::AuthSessionExpired => StatusCode::UNAUTHORIZED,
            Self::AuthRefreshTokenInvalid => StatusCode::UNAUTHORIZED,
            Self::UserUsernameTaken => StatusCode::CONFLICT,
            Self::RoleNameTaken => StatusCode::CONFLICT,
            Self::RoleAlreadyAssigned => StatusCode::CONFLICT,
            Self::FileTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::RealtimeUnknownFrame => StatusCode::BAD_REQUEST,
            Self::RealtimeTopicForbidden => StatusCode::FORBIDDEN,
        }
    }
}

pub(crate) fn respond(e: &AppError) -> (StatusCode, ErrorResp) {
    parts(e)
}

fn parts(e: &AppError) -> (StatusCode, ErrorResp) {
    let (status, code, message, errors) = match e {
        AppError::Business {
            code,
            message,
            errors,
        } => (code.status(), Some(*code), message.clone(), errors.clone()),
        AppError::MalformedRequest(message) => {
            (StatusCode::BAD_REQUEST, None, message.clone(), None)
        }
        AppError::Unauthenticated(message) => {
            (StatusCode::UNAUTHORIZED, None, message.clone(), None)
        }
        AppError::Forbidden(message) => (StatusCode::FORBIDDEN, None, message.clone(), None),
        AppError::NotFound(message) => (StatusCode::NOT_FOUND, None, message.clone(), None),
        AppError::Unavailable { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            None,
            "Service temporarily unavailable".to_owned(),
            None,
        ),
        AppError::Config(_)
        | AppError::Database(_)
        | AppError::Json(_)
        | AppError::Io(_)
        | AppError::Inquire(_)
        | AppError::Join(_)
        | AppError::Internal(_)
        | AppError::Any(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            None,
            "Internal server error".to_owned(),
            None,
        ),
    };

    (
        status,
        ErrorResp {
            code,
            message,
            errors,
        },
    )
}

/// 线上错误体 `{code?, message, errors?}`；长连接面用它构造错误帧，客户端只认这一份形状。
pub(crate) fn body(e: &AppError) -> ErrorResp {
    parts(e).1
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = respond(&self);
        let mut response = (status, Json(body)).into_response();
        if status.is_server_error() {
            response.extensions_mut().insert(Arc::new(self));
        }
        response
    }
}

/// 原生提取器的拒绝一律变成无码 `400`，只有体积上限是例外：它和多部件那条路径同码。
macro_rules! rejection {
    ($($rejection:ty),+ $(,)?) => {
        $(
            impl From<$rejection> for AppError {
                fn from(e: $rejection) -> Self {
                    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                        return AppError::business(ErrorCode::FileTooLarge);
                    }
                    AppError::malformed_request(e.body_text())
                }
            }
        )+
    };
}

rejection!(PathRejection, QueryRejection, JsonRejection);

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(e: &AppError) -> (u16, Option<ErrorCode>) {
        let (status, body) = respond(e);
        (status.as_u16(), body.code)
    }

    #[test]
    fn every_code_pins_its_status() {
        use strum::IntoEnumIterator;

        for code in ErrorCode::iter() {
            let (status, body) = respond(&AppError::business(code));
            assert!(
                status.is_client_error(),
                "{code:?} is a client-visible failure and must not map to {status}"
            );
            assert_eq!(
                body.code,
                Some(code),
                "a business failure must echo its code in the response body"
            );
            assert!(
                !body.message.is_empty(),
                "{code:?} must carry readable text"
            );
        }
    }

    #[test]
    fn transport_failures_have_no_code() {
        let unavailable = AppError::Unavailable { source: "m".into() };

        assert_eq!(wire(&AppError::malformed_request("m")), (400, None));
        assert_eq!(wire(&AppError::unauthenticated("m")), (401, None));
        assert_eq!(wire(&AppError::forbidden("m")), (403, None));
        assert_eq!(wire(&AppError::not_found("m")), (404, None));
        assert_eq!(wire(&unavailable), (503, None));
    }

    #[test]
    fn internal_failures_genericise_and_carry_no_code() {
        let (status, body) = respond(&AppError::internal("secret detail"));
        assert_eq!(status.as_u16(), 500);
        assert_eq!(body.message, "Internal server error");
        assert!(body.code.is_none());
    }
}
