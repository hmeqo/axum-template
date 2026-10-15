//! `/api/auth/jwt`：访问令牌与刷新令牌。

use axum::{Json, extract::State, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppContext,
        dto::*,
        extractor::AppJson,
        http::{error::ErrorResponses, openapi::OpenApiRouterExt},
        identity::{Auth, JwtAuth},
        middleware::auth::auth,
    },
    domain::vo::ClientInfo,
    error::{AppError, ErrorCode},
};

#[utoipa::path(post, path="/login", request_body = LoginReq, responses(
    ErrorResponses,
    (status = 200, body = LoginResp),
))]
pub async fn login(
    State(ctx): State<AppContext>,
    client: ClientInfo,
    AppJson(payload): AppJson<LoginReq>,
) -> Result<impl IntoResponse, AppError> {
    let outcome = ctx
        .srv()
        .auth
        .login_jwt(&payload.username, &payload.password, &client)
        .await?
        .ok_or_else(|| AppError::business(ErrorCode::AuthInvalidCredentials))?;

    Ok(Json(LoginResp {
        access_token: outcome.access_token,
        refresh_token: outcome.refresh_token,
        state: AuthStateResp::from(outcome.user),
    }))
}

/// Exchanges a refresh token for a new pair; the presented token is single-use, so a replay is a
/// `auth.refresh_token_invalid`.
#[utoipa::path(post, path="/refresh", request_body = RefreshReq, responses(
    ErrorResponses,
    (status = 200, body = RefreshResp),
))]
pub async fn refresh(
    State(ctx): State<AppContext>,
    client: ClientInfo,
    AppJson(payload): AppJson<RefreshReq>,
) -> Result<impl IntoResponse, AppError> {
    let rotated = ctx
        .srv()
        .token
        .rotate_refresh_token(&payload.refresh_token, &client)
        .await?;

    Ok(Json(RefreshResp {
        access_token: rotated.access_token,
        refresh_token: rotated.refresh_token,
    }))
}

/// Revokes every refresh token of the account.
#[utoipa::path(post, path="/logout", security(("bearer" = [])), responses(
    ErrorResponses,
    (status = 200, body = MessageResp),
))]
pub async fn logout(
    State(ctx): State<AppContext>,
    Auth(auth): Auth<JwtAuth>,
) -> Result<impl IntoResponse, AppError> {
    ctx.srv()
        .auth
        .logout_all_refresh_tokens(auth.user_id)
        .await?;

    Ok(Json(MessageResp::new("Logged out")))
}

pub const PATH: &str = "/auth/jwt";

pub fn router(ctx: &AppContext) -> OpenApiRouter<AppContext> {
    OpenApiRouter::new()
        .routes(routes![login])
        .routes(routes![refresh])
        .routes(routes![logout])
        .with_tags(["jwt"])
        .route_layer(auth(ctx, JwtAuth))
}
