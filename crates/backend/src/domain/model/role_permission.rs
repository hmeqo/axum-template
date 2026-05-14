use toasty::Model;

use super::{Permission, Role};

#[derive(Debug, Clone, Model)]
pub struct RolePermission {
    #[key]
    #[auto]
    pub id: i64,

    #[index]
    pub role_id: i64,

    #[belongs_to(key = role_id, references = id)]
    pub role: toasty::BelongsTo<Role>,

    #[index]
    pub permission_id: i64,

    #[belongs_to(key = permission_id, references = id)]
    pub permission: toasty::BelongsTo<Permission>,

    #[auto]
    pub created_at: jiff::Timestamp,
}
