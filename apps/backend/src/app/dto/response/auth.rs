use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::model::Perm;

use super::UserResponse;

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub state: AuthStateResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthStateResponse {
    pub user: UserResponse,
    pub permissions: Vec<Perm>,
}
