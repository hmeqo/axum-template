//! RPC 面的错误方言：状态与文案沿用 `app::http::error::respond`，外壳换成 Connect 的 canonical code。

use axum::http::{Extensions, StatusCode};
use connectrpc::{ConnectError, ErrorCode as ConnectCode, ErrorDetail};

use super::proto::template::v1::{ErrorDetail as WireErrorDetail, FieldErrors};
use crate::{
    app::{http::error::respond, middleware::request_id::RequestId},
    error::AppError,
};

/// [`AppError`] → Connect 错误：canonical code 由 REST 的状态码派生，业务码随 detail 一起发出。
pub(crate) fn to_connect(extensions: &Extensions, error: AppError) -> ConnectError {
    let (status, body) = respond(&error);

    if status.is_server_error() {
        let request_id = extensions
            .get::<RequestId>()
            .map(|id| id.0.as_str())
            .unwrap_or("-");
        tracing::error!(request_id, "Internal error: {error}");
    }

    let mut connect = ConnectError::new(connect_code(status), body.message);
    if let Some(code) = body.code {
        let detail = WireErrorDetail {
            code: code.wire_code(),
            errors: body
                .errors
                .unwrap_or_default()
                .into_iter()
                .map(|(field, messages)| {
                    (
                        field,
                        FieldErrors {
                            messages,
                            ..Default::default()
                        },
                    )
                })
                .collect(),
            ..Default::default()
        };
        connect = connect.with_detail(ErrorDetail::from_message(
            "template.v1.ErrorDetail",
            &detail,
        ));
    }
    connect
}

/// 状态码是唯一判据。
fn connect_code(status: StatusCode) -> ConnectCode {
    match status {
        StatusCode::BAD_REQUEST => ConnectCode::InvalidArgument,
        StatusCode::UNAUTHORIZED => ConnectCode::Unauthenticated,
        StatusCode::FORBIDDEN => ConnectCode::PermissionDenied,
        StatusCode::NOT_FOUND => ConnectCode::NotFound,
        StatusCode::CONFLICT => ConnectCode::AlreadyExists,
        StatusCode::PAYLOAD_TOO_LARGE | StatusCode::TOO_MANY_REQUESTS => {
            ConnectCode::ResourceExhausted
        }
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => ConnectCode::DeadlineExceeded,
        StatusCode::NOT_IMPLEMENTED => ConnectCode::Unimplemented,
        StatusCode::SERVICE_UNAVAILABLE => ConnectCode::Unavailable,
        _ => ConnectCode::Internal,
    }
}
