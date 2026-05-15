use axum::{Json, extract::State, response::IntoResponse};
use axum_extra::extract::cookie::CookieJar;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppState,
        dto::{request::*, response::*},
        error::ErrorResp,
        helper::{AppJson, SessionCtx, set_session_cookie},
    },
    error::{AppError, ErrorKind},
    ext::{EndpointRouter, EndpointRouterT, OpenApiRouterExt},
};

#[utoipa::path(post, path="/login", request_body = LoginReq, responses(
    (status = 200, body = AuthStateResp),
    (status = 400, body = ErrorResp),
))]
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    AppJson(payload): AppJson<LoginReq>,
) -> Result<impl IntoResponse, AppError> {
    let user = state
        .srv()
        .auth
        .authenticate(&payload.username, &payload.password)
        .await?
        .ok_or(ErrorKind::InvalidCredentials)?;

    let session_id = state.srv().session.create(user.id).await?;
    let jar = set_session_cookie(jar, &state, &session_id);

    let permissions = state.srv().role.get_user_permissions(user.id).await?;

    let response = AuthStateResp {
        user: UserResp::from(user),
        permissions,
    };

    Ok((jar, Json(response)))
}

#[utoipa::path(post, path="/logout", responses(
    (status = 200),
    (status = 401, body = ErrorResp),
))]
pub async fn logout(
    State(state): State<AppState>,
    ctx: SessionCtx,
) -> Result<impl IntoResponse, AppError> {
    state.srv().session.delete_by_user_id(ctx.user_id).await?;

    Ok(Json(serde_json::json!({"message": "Logged out"})))
}

#[utoipa::path(get, path="/me", responses(
    (status = 200, body = AuthStateResp),
    (status = 401, body = ErrorResp),
))]
pub async fn me(
    State(state): State<AppState>,
    ctx: SessionCtx,
) -> Result<impl IntoResponse, AppError> {
    let user = ctx.user(state.srv()).await?;
    let permissions = state.srv().role.get_user_permissions(user.id).await?;

    let response = AuthStateResp {
        user: UserResp::from(user),
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
