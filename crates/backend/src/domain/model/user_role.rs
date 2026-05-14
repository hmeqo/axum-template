use toasty::Model;

use super::{Role, User};

#[derive(Debug, Clone, Model)]
pub struct UserRole {
    #[key]
    #[auto]
    pub id: i64,

    #[index]
    pub user_id: i64,

    #[belongs_to(key = user_id, references = id)]
    pub user: toasty::BelongsTo<User>,

    #[index]
    pub role_id: i64,

    #[belongs_to(key = role_id, references = id)]
    pub role: toasty::BelongsTo<Role>,

    #[auto]
    pub created_at: jiff::Timestamp,
}
