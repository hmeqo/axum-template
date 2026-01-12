use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::db::Pk;

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub id: Pk,
    pub username: String,
    #[schema(value_type = String)]
    pub created_at: DateTime<FixedOffset>,
    #[schema(value_type = String)]
    pub updated_at: DateTime<FixedOffset>,
}

impl From<entity::user::Model> for UserResponse {
    fn from(user: entity::user::Model) -> Self {
        Self {
            id: user.id,
            username: user.username,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserListResponse {
    pub users: Vec<UserResponse>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
