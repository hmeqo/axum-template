use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::model::Perm;

use super::UserResp;

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResp {
    pub state: AuthStateResp,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthStateResp {
    pub user: UserResp,
    pub permissions: Vec<Perm>,
}
