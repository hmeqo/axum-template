//! `/api/auth`：登录、会话/设备管理，以及自助的 `me` 与改密。

use axum::{Json, extract::State, response::IntoResponse};
use axum_extra::extract::cookie::CookieJar;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppContext,
        dto::*,
        extractor::{AppJson, AppPath},
        http::{error::ErrorResponses, openapi::OpenApiRouterExt},
        identity::{Auth, JwtOrSession, SessionAuth, remove_session_cookie, set_session_cookie},
        middleware::auth::auth,
    },
    domain::vo::{ClientInfo, Password, SessionId},
    error::{AppError, ErrorCode},
};

#[utoipa::path(post, path="/login", request_body = LoginReq, responses(
    ErrorResponses,
    (status = 200, body = AuthStateResp),
))]
pub async fn login(
    State(ctx): State<AppContext>,
    jar: CookieJar,
    client: ClientInfo,
    AppJson(payload): AppJson<LoginReq>,
) -> Result<impl IntoResponse, AppError> {
    let outcome = ctx
        .srv()
        .auth
        .login_session(&payload.username, &payload.password, &client)
        .await?
        .ok_or_else(|| AppError::business(ErrorCode::AuthInvalidCredentials))?;

    let jar = set_session_cookie(jar, &ctx, &outcome.session_token, outcome.expires_at);

    Ok((jar, Json(AuthStateResp::from(outcome.user))))
}

#[utoipa::path(post, path="/logout", responses(
    ErrorResponses,
    (status = 200, body = MessageResp),
))]
pub async fn logout(
    State(state): State<AppContext>,
    jar: CookieJar,
    Auth(auth): Auth<SessionAuth>,
) -> Result<impl IntoResponse, AppError> {
    state.srv().session.revoke_own(auth.session_id).await?;

    let jar = remove_session_cookie(jar, &state);

    Ok((jar, Json(MessageResp::new("Logged out"))))
}

#[utoipa::path(get, path="/me", responses(
    ErrorResponses,
    (status = 200, body = AuthStateResp),
))]
pub async fn me(
    State(ctx): State<AppContext>,
    Auth(auth): Auth<JwtOrSession>,
) -> Result<impl IntoResponse, AppError> {
    let auth_user = ctx.srv().auth.get_auth_user(auth.user_id).await?;

    Ok(Json(AuthStateResp::from(auth_user)))
}

#[utoipa::path(put, path="/password", request_body = ChangePasswordReq, responses(
    ErrorResponses,
    (status = 200, body = MessageResp),
))]
pub async fn change_password(
    Auth(auth): Auth<JwtOrSession>,
    session: Option<Auth<SessionAuth>>,
    client: ClientInfo,
    jar: CookieJar,
    State(ctx): State<AppContext>,
    AppJson(payload): AppJson<ChangePasswordReq>,
) -> Result<impl IntoResponse, AppError> {
    let rotated = ctx
        .srv()
        .auth
        .change_password(
            auth.user_id,
            &payload.old_password,
            Password::new(payload.new_password)?,
            &client,
            session.is_some(),
        )
        .await?;

    let body = Json(MessageResp::new("Password changed successfully"));
    match rotated {
        // 轮换：旧 cookie 在改密这一刻作废，新会话随这个响应下发。
        Some((token, expires_at)) => {
            Ok((set_session_cookie(jar, &ctx, &token, expires_at), body).into_response())
        }
        None => Ok(body.into_response()),
    }
}

#[utoipa::path(get, path="/sessions", responses(
    ErrorResponses,
    (status = 200, body = SessionListResp),
))]
pub async fn sessions(
    State(ctx): State<AppContext>,
    Auth(auth): Auth<SessionAuth>,
) -> Result<impl IntoResponse, AppError> {
    let sessions = ctx.srv().session.list_by_user_id(auth.user_id).await?;
    let sessions = sessions
        .iter()
        .map(|session| SessionResp::new(session, auth.session_id))
        .collect();

    Ok(Json(SessionListResp { sessions }))
}

#[utoipa::path(delete, path="/sessions/{id}", params(
    ("id" = SessionId, Path)
), responses(
    ErrorResponses,
    (status = 200, body = MessageResp),
))]
pub async fn revoke_session(
    State(ctx): State<AppContext>,
    jar: CookieJar,
    Auth(auth): Auth<SessionAuth>,
    AppPath(SessionIdPath { id }): AppPath<SessionIdPath>,
) -> Result<impl IntoResponse, AppError> {
    ctx.srv().session.revoke(id, auth.user_id).await?;

    let jar = if id == auth.session_id {
        remove_session_cookie(jar, &ctx)
    } else {
        jar
    };

    Ok((jar, Json(MessageResp::new("Session revoked"))))
}

#[utoipa::path(post, path="/sessions/revoke-others", responses(
    ErrorResponses,
    (status = 200, body = MessageResp),
))]
pub async fn revoke_other_sessions(
    State(ctx): State<AppContext>,
    Auth(auth): Auth<SessionAuth>,
) -> Result<impl IntoResponse, AppError> {
    ctx.srv()
        .session
        .delete_others(auth.user_id, auth.session_id)
        .await?;

    Ok(Json(MessageResp::new("Other sessions revoked")))
}

pub const PATH: &str = "/auth";

pub fn router(ctx: &AppContext) -> OpenApiRouter<AppContext> {
    let session_only = OpenApiRouter::new()
        .routes(routes![login])
        .routes(routes![logout])
        .routes(routes![sessions])
        .routes(routes![revoke_session])
        .routes(routes![revoke_other_sessions])
        .with_tags(["auth"])
        .route_layer(auth(ctx, SessionAuth));

    // 独立成组而不并入上面那组：`Bearer` 先解析，混带 token 的请求会在会话端点 401。
    let identity = OpenApiRouter::new()
        .routes(routes![me])
        .routes(routes![change_password])
        .with_tags(["auth"])
        .route_layer(auth(ctx, JwtOrSession));

    session_only.merge(identity)
}
