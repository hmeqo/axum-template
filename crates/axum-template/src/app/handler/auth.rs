use axum::{Json, extract::State, response::IntoResponse};
use axum_extra::extract::cookie::CookieJar;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppState,
        dto::{request::*, response::*},
        error::ErrorResp,
        extractor::{AppJson, SessionCtx, set_session_cookie},
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
    let auth_user = state
        .srv()
        .auth
        .authenticate(&payload.username, &payload.password)
        .await?
        .ok_or(ErrorKind::InvalidCredentials)?;

    let session_id = state.srv().session.create(auth_user.user.id).await?;
    let jar = set_session_cookie(jar, &state, &session_id);

    Ok((
        jar,
        Json(AuthStateResp {
            user: UserResp::from(auth_user.user),
            permissions: auth_user.permissions,
        }),
    ))
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
    let auth_user = state.srv().auth.get_auth_user(ctx.user_id).await?;

    Ok(Json(AuthStateResp {
        user: UserResp::from(auth_user.user),
        permissions: auth_user.permissions,
    }))
}

pub fn router() -> EndpointRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes![login])
        .routes(routes![logout])
        .routes(routes![me])
        .with_tags(["auth"])
        .endpoint("/auth")
}
