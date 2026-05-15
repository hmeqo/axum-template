use axum::{Json, extract::State, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppState,
        dto::{request::*, response::*},
        error::ErrorResp,
        helper::{auth::AuthCtx, extractor::AppJson},
    },
    domain::model::Perm,
    error::{AppError, ErrorKind},
    ext::{EndpointRouter, EndpointRouterT, OpenApiRouterExt},
};

#[utoipa::path(post, path="/login", request_body = LoginReq, responses(
    (status = 200, body = LoginResp),
    (status = 400, body = ErrorResp),
))]
pub async fn login(
    State(state): State<AppState>,
    AppJson(payload): AppJson<LoginReq>,
) -> Result<impl IntoResponse, AppError> {
    let user = state
        .services()
        .auth
        .authenticate(&payload.username, &payload.password)
        .await?
        .ok_or(ErrorKind::InvalidCredentials)?;

    let access_token = state.services().token.encode_access_token(&user)?;
    let refresh_token = state
        .services()
        .token
        .generate_refresh_token(user.id)
        .await?;

    let permissions = state
        .services()
        .role
        .get_user_permissions(user.id)
        .await?
        .iter()
        .filter_map(|p| Perm::try_from(p).ok())
        .collect();

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

#[utoipa::path(post, path="/refresh", request_body = RefreshReq, responses(
    (status = 200, body = RefreshResp),
    (status = 400, body = ErrorResp),
))]
pub async fn refresh(
    State(state): State<AppState>,
    AppJson(payload): AppJson<RefreshReq>,
) -> Result<impl IntoResponse, AppError> {
    let rotated = state
        .services()
        .token
        .rotate_refresh_token(&payload.refresh_token)
        .await?;

    Ok(Json(RefreshResp {
        access_token: rotated.access_token,
        refresh_token: rotated.refresh_token,
    }))
}

#[utoipa::path(get, path="/me", responses(
    (status = 200, body = AuthStateResp),
    (status = 400, body = ErrorResp),
))]
pub async fn me(
    State(state): State<AppState>,
    ctx: AuthCtx,
) -> Result<impl IntoResponse, AppError> {
    let user = ctx.user(state.services()).await?;
    let permissions = state
        .services()
        .role
        .get_user_permissions(user.id)
        .await?
        .iter()
        .filter_map(|p| Perm::try_from(p).ok())
        .collect();

    let response = AuthStateResp {
        user: UserResp::from(user),
        permissions,
    };

    Ok(Json(response))
}

pub fn router() -> EndpointRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes![login])
        .routes(routes![refresh])
        .routes(routes![me])
        .with_tags(["auth"])
        .endpoint("/auth")
}
