use axum::{Json, extract::State, response::IntoResponse};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    app::{
        AppState,
        dto::{request::*, response::*},
        helper::extractor::{AppJson, AppPath, AppQuery, RequirePermission},
        response::ErrorResponse,
    },
    domain::{db::Pk, model::Permission},
    error::{AppError, ErrorKind},
    ext::{EndpointRouter, EndpointRouterT, OpenApiRouterExt},
};

#[utoipa::path(get, path="/", params(
    ("page" = Option<u64>, Query, description = "Page number"),
    ("per_page" = Option<u64>, Query, description = "Items per page")
), responses(
    (status = 200, body = UserListResponse),
    (status = 400, body = ErrorResponse),
))]
pub async fn list(
    State(state): State<AppState>,
    AppQuery(pagination): AppQuery<PaginationQuery>,
) -> Result<impl IntoResponse, AppError> {
    let page = pagination.page;
    let per_page = pagination.per_page;

    let users = state.services().user.list(page, per_page).await?;
    let total = state.services().user.count().await?;

    let user_responses = users.into_iter().map(UserResponse::from).collect();

    let response = UserListResponse {
        users: user_responses,
        total,
        page,
        per_page,
    };

    Ok(Json(response))
}

#[utoipa::path(post, path="/", request_body = CreateUserRequest, responses(
    (status = 200, body = UserResponse),
    (status = 400, body = ErrorResponse),
))]
pub async fn create(
    RequirePermission(user): RequirePermission,
    State(state): State<AppState>,
    AppJson(payload): AppJson<CreateUserRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !user.has_permission(Permission::UserCreate) {
        return Err(ErrorKind::PermissionDenied.with_message("Insufficient permissions"));
    }

    let user = state
        .services()
        .user
        .create(payload.username, payload.password)
        .await?;
    let response = UserResponse::from(user);
    Ok(Json(response))
}

#[utoipa::path(get, path="/{id}", params(
    ("id" = Pk, Path)
), responses(
    (status = 200, body = UserResponse),
    (status = 400, body = ErrorResponse),
))]
pub async fn get(
    State(state): State<AppState>,
    AppPath(PkPath { id }): AppPath<PkPath>,
) -> Result<impl IntoResponse, AppError> {
    let user = state.services().user.get_by_id(id).await?;
    let response = UserResponse::from(user);
    Ok(Json(response))
}

#[utoipa::path(put, path="/{id}/username", params(
    ("id" = Pk, Path)
), request_body = UpdateUsernameRequest, responses(
    (status = 200, body = UserResponse),
    (status = 400, body = ErrorResponse),
))]
pub async fn update_username(
    State(state): State<AppState>,
    AppPath(PkPath { id }): AppPath<PkPath>,
    Json(payload): Json<UpdateUsernameRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = state
        .services()
        .user
        .update_username(id, payload.username)
        .await?;
    let response = UserResponse::from(user);
    Ok(Json(response))
}

#[utoipa::path(put, path="/{id}/password", params(
    ("id" = Pk, Path)
), request_body = ChangePasswordRequest, responses(
    (status = 200, body = MessageResponse),
    (status = 400, body = ErrorResponse),
))]
pub async fn change_password(
    State(state): State<AppState>,
    AppPath(PkPath { id }): AppPath<PkPath>,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    state
        .services()
        .user
        .change_password(id, &payload.old_password, &payload.new_password)
        .await?;
    let response = MessageResponse {
        message: "Password changed successfully".to_string(),
    };
    Ok(Json(response))
}

#[utoipa::path(delete, path="/{id}", params(
    ("id" = Pk, Path)
), responses(
    (status = 200, body = MessageResponse),
    (status = 400, body = ErrorResponse),
))]
pub async fn delete(
    State(state): State<AppState>,
    AppPath(PkPath { id }): AppPath<PkPath>,
) -> Result<impl IntoResponse, AppError> {
    state.services().user.delete(id).await?;
    let response = MessageResponse {
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
