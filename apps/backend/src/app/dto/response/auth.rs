use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub user: super::user::UserResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CurrentUserResponse {
    pub user: super::user::UserResponse,
}
