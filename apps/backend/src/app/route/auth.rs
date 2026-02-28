use axum::{Json, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppState,
        dto::{request::*, response::*},
        error::ErrorResponse,
        helper::{
            auth::{AuthSession, Credentials},
            extractor::AppJson,
        },
    },
    error::{AppError, ErrorKind},
    ext::{EndpointRouter, EndpointRouterT, OpenApiRouterExt},
};

#[utoipa::path(post, path="/login", request_body = LoginRequest, responses(
    (status = 200, body = LoginResponse),
    (status = 400, body = ErrorResponse),
))]
#[axum::debug_handler]
pub async fn login(
    mut auth_session: AuthSession,
    AppJson(payload): AppJson<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth_session
        .authenticate(Credentials {
            username: payload.username,
            password: payload.password,
        })
        .await
        .map_err(ErrorKind::wrap_internal)?
        .ok_or(ErrorKind::InvalidCredentials)?;

    auth_session
        .login(&user)
        .await
        .map_err(ErrorKind::wrap_internal)?;
    let permissions = user.permissions_as_enum().await;

    let response = LoginResponse {
        state: AuthStateResponse {
            user: UserResponse::from(user.user),
            permissions,
        },
    };

    Ok(Json(response))
}

#[utoipa::path(post, path="/logout", responses(
    (status = 200, body = MessageResponse),
    (status = 400, body = ErrorResponse),
))]
pub async fn logout(mut auth_session: AuthSession) -> Result<impl IntoResponse, AppError> {
    // Check if user is logged in
    if auth_session.user.is_none() {
        return Err(ErrorKind::Unauthorized.into());
    }

    auth_session
        .logout()
        .await
        .map_err(ErrorKind::wrap_internal)?;

    let response = MessageResponse {
        message: "Logged out successfully".to_string(),
    };

    Ok(Json(response))
}

#[utoipa::path(get, path="/me", responses(
    (status = 200, body = AuthStateResponse),
    (status = 400, body = ErrorResponse),
))]
pub async fn me(auth_session: AuthSession) -> Result<impl IntoResponse, AppError> {
    let user = auth_session.user.ok_or(ErrorKind::Unauthorized)?;
    let permissions = user.permissions_as_enum().await;

    let response = AuthStateResponse {
        user: UserResponse::from(user.user),
        permissions,
    };
    Ok(Json(response))
}

pub fn router() -> EndpointRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes![login])
        .routes(routes![logout])
        .routes(routes![me])
        .with_tags(["auth"])
        .endpoint("/auth")
}
