use axum::{Json, extract::State, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppState,
        dto::{request::*, response::*},
        error::ErrorResp,
        helper::{AppJson, auth::JwtCtx},
    },
    error::{AppError, ErrorKind},
    ext::{EndpointRouter, EndpointRouterT, OpenApiRouterExt},
};

#[utoipa::path(post, path="/jwt/login", request_body = LoginReq, responses(
    (status = 200, body = LoginResp),
    (status = 400, body = ErrorResp),
))]
pub async fn login(
    State(state): State<AppState>,
    AppJson(payload): AppJson<LoginReq>,
) -> Result<impl IntoResponse, AppError> {
    let user = state
        .srv()
        .auth
        .authenticate(&payload.username, &payload.password)
        .await?
        .ok_or(ErrorKind::InvalidCredentials)?;

    let access_token = state.srv().token.encode_access_token(&user)?;
    let refresh_token = state.srv().token.generate_refresh_token(user.id).await?;

    let permissions = state.srv().role.get_user_permissions(user.id).await?;

    let response = LoginResp {
        access_token,
        refresh_token,
        state: AuthStateResp {
            user: UserResp::from(user),
            permissions,
        },
    };

    Ok(Json(response))
}

#[utoipa::path(post, path="/jwt/refresh", request_body = RefreshReq, responses(
    (status = 200, body = RefreshResp),
    (status = 401, body = ErrorResp),
))]
pub async fn refresh(
    State(state): State<AppState>,
    AppJson(payload): AppJson<RefreshReq>,
) -> Result<impl IntoResponse, AppError> {
    let rotated = state
        .srv()
        .token
        .rotate_refresh_token(&payload.refresh_token)
        .await?;

    Ok(Json(RefreshResp {
        access_token: rotated.access_token,
        refresh_token: rotated.refresh_token,
    }))
}

#[utoipa::path(post, path="/jwt/logout", responses(
    (status = 200),
    (status = 401, body = ErrorResp),
))]
pub async fn logout(
    State(state): State<AppState>,
    ctx: JwtCtx,
) -> Result<impl IntoResponse, AppError> {
    state
        .srv()
        .token
        .delete_all_refresh_tokens(ctx.user_id)
        .await?;

    Ok(Json(serde_json::json!({"message": "Logged out"})))
}

#[utoipa::path(get, path="/jwt/me", responses(
    (status = 200, body = AuthStateResp),
    (status = 401, body = ErrorResp),
))]
pub async fn me(State(state): State<AppState>, ctx: JwtCtx) -> Result<impl IntoResponse, AppError> {
    let user = ctx.user(state.srv()).await?;
    let permissions = state.srv().role.get_user_permissions(user.id).await?;

    Ok(Json(AuthStateResp {
        user: UserResp::from(user),
        permissions,
    }))
}

#[utoipa::path(get, path="/jwt/echo", responses(
    (status = 200),
    (status = 401, body = ErrorResp),
))]
pub async fn echo(ctx: JwtCtx) -> Result<impl IntoResponse, AppError> {
    Ok(Json(serde_json::json!({
        "message": "Authenticated",
        "user_id": ctx.user_id,
        "username": ctx.username(),
    })))
}

pub fn router() -> EndpointRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes![login])
        .routes(routes![refresh])
        .routes(routes![logout])
        .routes(routes![me])
        .routes(routes![echo])
        .with_tags(["jwt-demo"])
        .endpoint("/auth")
}
