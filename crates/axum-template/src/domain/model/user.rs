use toasty::Model;

use super::{File, RefreshToken, Session, UserRole};
use crate::domain::vo::{PasswordHash, UserId};

#[derive(Debug, Clone, Model)]
pub struct User {
    #[key]
    #[auto]
    pub id: UserId,

    #[unique]
    pub username: String,

    pub password: PasswordHash,

    #[auto]
    pub created_at: jiff::Timestamp,

    #[auto]
    pub updated_at: jiff::Timestamp,

    #[has_many]
    pub user_roles: toasty::Deferred<Vec<UserRole>>,

    #[has_many]
    pub sessions: toasty::Deferred<Vec<Session>>,

    #[has_many]
    pub refresh_tokens: toasty::Deferred<Vec<RefreshToken>>,

    #[has_many]
    pub files: toasty::Deferred<Vec<File>>,
}
