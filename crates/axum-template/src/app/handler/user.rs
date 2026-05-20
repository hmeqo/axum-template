use axum::{Json, extract::State, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppState,
        dto::{request::*, response::*},
        error::ErrorResp,
        extractor::{AppJson, AppPath, AppQuery, SessionCtx},
    },
    bail,
    domain::{db::Pk, model::Perm},
    error::{AppError, ErrorKind},
    ext::{EndpointRouter, EndpointRouterT, OpenApiRouterExt},
};

#[utoipa::path(get, path="/", params(
    ("page" = Option<u64>, Query, description = "Page number"),
    ("per_page" = Option<u64>, Query, description = "Items per page")
), responses(
    (status = 200, body = UserListResp),
    (status = 400, body = ErrorResp),
))]
pub async fn list(
    ctx: SessionCtx,
    State(state): State<AppState>,
    AppQuery(pagination): AppQuery<PaginationReq>,
) -> Result<impl IntoResponse, AppError> {
    state
        .srv()
        .auth
        .require_permission(ctx.user_id, Perm::UserRead)
        .await?;

    let page = pagination.page;
    let per_page = pagination.per_page;

    let users = state.srv().user.list(page, per_page).await?;
    let total = state.srv().user.count().await?;

    let user_responses = users.into_iter().map(UserResp::from).collect();

    let response = UserListResp {
        users: user_responses,
        total,
        page,
        per_page,
    };

    Ok(Json(response))
}

#[utoipa::path(post, path="/", request_body = CreateUserReq, responses(
    (status = 200, body = UserResp),
    (status = 400, body = ErrorResp),
))]
pub async fn create(
    ctx: SessionCtx,
    State(state): State<AppState>,
    AppJson(payload): AppJson<CreateUserReq>,
) -> Result<impl IntoResponse, AppError> {
    state
        .srv()
        .auth
        .require_permission(ctx.user_id, Perm::UserWrite)
        .await?;

    let user = state
        .srv()
        .user
        .create(payload.username, payload.password)
        .await?;
    let response = UserResp::from(user);
    Ok(Json(response))
}

#[utoipa::path(get, path="/{id}", params(
    ("id" = Pk, Path)
), responses(
    (status = 200, body = UserResp),
    (status = 400, body = ErrorResp),
))]
pub async fn get(
    ctx: SessionCtx,
    State(state): State<AppState>,
    AppPath(PkPath { id }): AppPath<PkPath>,
) -> Result<impl IntoResponse, AppError> {
    state
        .srv()
        .auth
        .require_permission(ctx.user_id, Perm::UserRead)
        .await?;

    let user = state.srv().user.get_by_id(id).await?;
    let response = UserResp::from(user);
    Ok(Json(response))
}

#[utoipa::path(put, path="/{id}/username", params(
    ("id" = Pk, Path)
), request_body = UpdateUsernameReq, responses(
    (status = 200, body = UserResp),
    (status = 400, body = ErrorResp),
))]
pub async fn update_username(
    ctx: SessionCtx,
    State(state): State<AppState>,
    AppPath(PkPath { id }): AppPath<PkPath>,
    Json(payload): Json<UpdateUsernameReq>,
) -> Result<impl IntoResponse, AppError> {
    state
        .srv()
        .auth
        .require_permission(ctx.user_id, Perm::UserWrite)
        .await?;

    let user = state
        .srv()
        .user
        .update_username(id, payload.username)
        .await?;
    let response = UserResp::from(user);
    Ok(Json(response))
}

#[utoipa::path(put, path="/{id}/password", params(
    ("id" = Pk, Path)
), request_body = ChangePasswordReq, responses(
    (status = 200, body = MessageResp),
    (status = 400, body = ErrorResp),
))]
pub async fn change_password(
    ctx: SessionCtx,
    State(state): State<AppState>,
    AppPath(PkPath { id }): AppPath<PkPath>,
    Json(payload): Json<ChangePasswordReq>,
) -> Result<impl IntoResponse, AppError> {
    if ctx.user_id != id {
        bail!(
            ErrorKind::PermissionDenied,
            "Cannot change another user's password"
        );
    }

    state
        .srv()
        .user
        .change_password(id, &payload.old_password, &payload.new_password)
        .await?;
    let response = MessageResp {
        message: "Password changed successfully".to_string(),
    };
    Ok(Json(response))
}

#[utoipa::path(delete, path="/{id}", params(
    ("id" = Pk, Path)
), responses(
    (status = 200, body = MessageResp),
    (status = 400, body = ErrorResp),
))]
pub async fn delete(
    ctx: SessionCtx,
    State(state): State<AppState>,
    AppPath(PkPath { id }): AppPath<PkPath>,
) -> Result<impl IntoResponse, AppError> {
    state
        .srv()
        .auth
        .require_permission(ctx.user_id, Perm::UserDelete)
        .await?;

    state.srv().user.delete(id).await?;
    let response = MessageResp {
        message: "User deleted successfully".to_string(),
    };
    Ok(Json(response))
}

pub fn router() -> EndpointRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes![list])
        .routes(routes![create])
        .routes(routes![get])
        .routes(routes![delete])
        .routes(routes![update_username])
        .routes(routes![change_password])
        .with_tags(["user"])
        .endpoint("/users")
}
