use serde::Serialize;
use utoipa::ToSchema;

use super::UserResp;
use crate::domain::model::Perm;

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResp {
    pub access_token: String,
    pub refresh_token: String,
    pub state: AuthStateResp,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RefreshResp {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthStateResp {
    pub user: UserResp,
    pub permissions: Vec<Perm>,
}
