use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub id: i32,
    pub username: String,
    #[schema(value_type = String)]
    #[serde(with = "time::serde::iso8601")]
    pub created_at: OffsetDateTime,
    #[schema(value_type = String)]
    #[serde(with = "time::serde::iso8601")]
    pub updated_at: OffsetDateTime,
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
