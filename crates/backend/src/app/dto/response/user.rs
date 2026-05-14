use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::{db::Pk, model::User};

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResp {
    pub id: Pk,
    pub username: String,
    #[schema(value_type = String)]
    pub created_at: jiff::Timestamp,
    #[schema(value_type = String)]
    pub updated_at: jiff::Timestamp,
}

impl From<User> for UserResp {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserListResp {
    pub users: Vec<UserResp>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResp {
    pub message: String,
}
