use axum::{Json, extract::State, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppState,
        dto::{request::*, response::*},
        error::ErrorResp,
        helper::{auth, auth::AuthCtx, extractor::AppJson},
    },
    domain::model::{Perm, RefreshToken},
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

    let config = state.config.load();
    let access_token =
        auth::encode_access_token(user.id, &user.username, &config.auth.jwt_secret)
            .map_err(|e| ErrorKind::Internal.msg(format!("Token generation failed: {e}")))?;

    let refresh_token = auth::generate_refresh_token();
    let expires_at = jiff::Timestamp::now() + jiff::Span::new().days(30);

    let mut db = state.domain.db.clone();
    toasty::create!(RefreshToken {
        user_id: user.id,
        token: refresh_token.clone(),
        expires_at,
    })
    .exec(&mut db)
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
    let mut db = state.domain.db.clone();

    // Find and delete the refresh token (single-use)
    let stored = match RefreshToken::filter_by_token(&payload.refresh_token).get(&mut db).await {
        Ok(t) => t,
        Err(_) => return Err(ErrorKind::Unauthorized.msg("Invalid refresh token")),
    };
    RefreshToken::filter_by_id(stored.id).delete().exec(&mut db).await?;

    let now = jiff::Timestamp::now();
    if stored.expires_at < now {
        return Err(ErrorKind::Unauthorized.msg("Refresh token expired"));
    }

    let user = state.services().user.get_by_id(stored.user_id).await?;
    let config = state.config.load();
    let access_token =
        auth::encode_access_token(user.id, &user.username, &config.auth.jwt_secret)
            .map_err(|e| ErrorKind::Internal.msg(format!("Token generation failed: {e}")))?;

    let new_refresh_token = auth::generate_refresh_token();
    let expires_at = jiff::Timestamp::now() + jiff::Span::new().days(30);

    toasty::create!(RefreshToken {
        user_id: user.id,
        token: new_refresh_token.clone(),
        expires_at,
    })
    .exec(&mut db)
    .await?;

    Ok(Json(RefreshResp {
        access_token,
        refresh_token: new_refresh_token,
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
