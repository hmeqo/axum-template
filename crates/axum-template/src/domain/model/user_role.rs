use toasty::Model;

use super::{Role, User};
use crate::domain::vo::{RoleId, UserId, UserRoleId};

#[derive(Debug, Clone, Model)]
#[unique(user_id, role_id)]
pub struct UserRole {
    #[key]
    #[auto]
    pub id: UserRoleId,

    #[index]
    pub user_id: UserId,

    #[belongs_to(key = user_id, references = id)]
    pub user: toasty::Deferred<User>,

    #[index]
    pub role_id: RoleId,

    #[belongs_to(key = role_id, references = id)]
    pub role: toasty::Deferred<Role>,

    #[auto]
    pub created_at: jiff::Timestamp,
}
