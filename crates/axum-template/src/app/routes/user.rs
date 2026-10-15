//! `/api/users`：用户 CRUD 与管理员重置他人密码。

use axum::{Json, extract::State, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppContext,
        dto::*,
        extractor::{AppJson, AppPath, AppQuery, Require},
        http::{error::ErrorResponses, openapi::OpenApiRouterExt},
        identity::JwtOrSession,
        middleware::auth::auth,
    },
    domain::vo::{
        Pagination, Password, UserDelete, UserFilter, UserId, UserPasswordReset, UserRead,
        UserWrite, Username,
    },
    error::AppError,
};

#[utoipa::path(get, path="/", security(("sessionCookie" = []), ("bearer" = [])), params(PaginationReq, UserFilterReq), responses(
    ErrorResponses,
    (status = 200, body = UserListResp),
))]
pub async fn list(
    _perm: Require<UserRead>,
    State(ctx): State<AppContext>,
    AppQuery(pagination): AppQuery<PaginationReq>,
    AppQuery(filter): AppQuery<UserFilterReq>,
) -> Result<impl IntoResponse, AppError> {
    let pagination = Pagination::try_from(pagination)?;
    let filter = UserFilter::try_from(filter)?;

    let users = ctx
        .srv()
        .user
        .page(pagination, &filter)
        .await?
        .map(UserResp::from);

    Ok(Json(UserListResp::from(users)))
}

#[utoipa::path(post, path="/", security(("sessionCookie" = []), ("bearer" = [])), request_body = CreateUserReq, responses(
    ErrorResponses,
    (status = 200, body = UserResp),
))]
pub async fn create(
    _perm: Require<UserWrite>,
    State(ctx): State<AppContext>,
    AppJson(payload): AppJson<CreateUserReq>,
) -> Result<impl IntoResponse, AppError> {
    let user = ctx
        .srv()
        .user
        .create(
            Username::new(payload.username)?,
            Password::new(payload.password)?,
        )
        .await?;
    let response = UserResp::from(user);
    Ok(Json(response))
}

#[utoipa::path(get, path="/{id}", security(("sessionCookie" = []), ("bearer" = [])), params(
    ("id" = UserId, Path)
), responses(
    ErrorResponses,
    (status = 200, body = UserResp),
))]
pub async fn get(
    _perm: Require<UserRead>,
    State(ctx): State<AppContext>,
    AppPath(UserIdPath { id }): AppPath<UserIdPath>,
) -> Result<impl IntoResponse, AppError> {
    let user = ctx.srv().user.get_by_id(id).await?;
    let response = UserResp::from(user);
    Ok(Json(response))
}

#[utoipa::path(put, path="/{id}/username", security(("sessionCookie" = []), ("bearer" = [])), params(
    ("id" = UserId, Path)
), request_body = UpdateUsernameReq, responses(
    ErrorResponses,
    (status = 200, body = UserResp),
))]
pub async fn update_username(
    _perm: Require<UserWrite>,
    State(ctx): State<AppContext>,
    AppPath(UserIdPath { id }): AppPath<UserIdPath>,
    AppJson(payload): AppJson<UpdateUsernameReq>,
) -> Result<impl IntoResponse, AppError> {
    let user = ctx
        .srv()
        .user
        .update_username(id, Username::new(payload.username)?)
        .await?;
    let response = UserResp::from(user);
    Ok(Json(response))
}

// 与 `/api/auth/password` 相对：这条针对别人，故不验旧密码。
/// Sets another user's password without the old one, and revokes the target's sessions and refresh
/// tokens.
#[utoipa::path(post, path="/{id}/password/reset", params(
    ("id" = UserId, Path)
), security(("sessionCookie" = []), ("bearer" = [])), request_body = ResetPasswordReq, responses(
    ErrorResponses,
    (status = 200, body = MessageResp),
))]
pub async fn reset_password(
    _perm: Require<UserPasswordReset>,
    State(ctx): State<AppContext>,
    AppPath(UserIdPath { id }): AppPath<UserIdPath>,
    AppJson(payload): AppJson<ResetPasswordReq>,
) -> Result<impl IntoResponse, AppError> {
    ctx.srv()
        .auth
        .reset_password(id, Password::new(payload.new_password)?)
        .await?;

    Ok(Json(MessageResp::new("Password reset successfully")))
}

/// Deletes the user together with everything the relations cascade: sessions, refresh tokens, files.
#[utoipa::path(delete, path="/{id}", security(("sessionCookie" = []), ("bearer" = [])), params(
    ("id" = UserId, Path)
), responses(
    ErrorResponses,
    (status = 200, body = MessageResp),
))]
pub async fn delete(
    _perm: Require<UserDelete>,
    State(ctx): State<AppContext>,
    AppPath(UserIdPath { id }): AppPath<UserIdPath>,
) -> Result<impl IntoResponse, AppError> {
    ctx.srv().auth.delete_user(id).await?;
    Ok(Json(MessageResp::new("User deleted successfully")))
}

pub const PATH: &str = "/users";

pub fn router(ctx: &AppContext) -> OpenApiRouter<AppContext> {
    OpenApiRouter::new()
        .routes(routes![list])
        .routes(routes![create])
        .routes(routes![get])
        .routes(routes![update_username])
        .routes(routes![reset_password])
        .routes(routes![delete])
        .with_tags(["users"])
        .route_layer(auth(ctx, JwtOrSession))
}
