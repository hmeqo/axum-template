use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use super::UserResp;
use crate::domain::{
    model::Session,
    service::AuthUser,
    vo::{Perm, SessionId},
};

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RefreshReq {
    pub refresh_token: String,
}

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

impl From<AuthUser> for AuthStateResp {
    fn from(a: AuthUser) -> Self {
        Self {
            user: UserResp::from(a.user),
            permissions: a.permissions,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionResp {
    pub id: SessionId,

    pub ip: Option<String>,

    pub user_agent: Option<String>,

    #[schema(value_type = String, format = "date-time")]
    pub expires_at: jiff::Timestamp,

    #[schema(value_type = String, format = "date-time")]
    pub created_at: jiff::Timestamp,

    /// Whether this is the session that made the request
    pub current: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionListResp {
    pub sessions: Vec<SessionResp>,
}

impl SessionResp {
    pub fn new(session: &Session, current_id: SessionId) -> Self {
        Self {
            id: session.id,
            ip: session.ip.clone(),
            user_agent: session.user_agent.clone(),
            expires_at: session.expires_at,
            created_at: session.created_at,
            current: session.id == current_id,
        }
    }
}
