use toasty::Model;

use super::UserRole;

#[derive(Debug, Clone, Model)]
pub struct User {
    #[key]
    #[auto]
    pub id: i64,

    #[unique]
    pub username: String,

    pub password: String,

    #[auto]
    pub created_at: jiff::Timestamp,

    #[auto]
    pub updated_at: jiff::Timestamp,

    #[has_many]
    pub user_roles: toasty::HasMany<UserRole>,
}
