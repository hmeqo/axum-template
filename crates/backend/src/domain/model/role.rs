use strum::{Display, EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};
use toasty::Model;

use super::{Perm, RolePermission, UserRole};

pub const DEFAULT_ROLE_PERMISSIONS: &[(DefaultRole, &[Perm])] = &[
    (DefaultRole::Superuser, &[Perm::All]),
    (DefaultRole::Admin, &[Perm::UserAll, Perm::RoleAll]),
    (DefaultRole::User, &[Perm::UserRead]),
];

#[derive(Debug, Clone, Copy, EnumString, EnumIter, IntoStaticStr, Display)]
#[strum(serialize_all = "snake_case")]
pub enum DefaultRole {
    Superuser,
    Admin,
    User,
}

impl DefaultRole {
    pub fn name(&self) -> &'static str {
        self.into()
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Superuser => "超级用户",
            Self::Admin => "管理员",
            Self::User => "普通用户",
        }
    }

    pub fn all() -> Vec<DefaultRole> {
        Self::iter().collect()
    }
}

#[derive(Debug, Clone, Model)]
pub struct Role {
    #[key]
    #[auto]
    pub id: i64,

    #[unique]
    pub name: String,

    pub description: Option<String>,

    #[auto]
    pub created_at: jiff::Timestamp,

    #[auto]
    pub updated_at: jiff::Timestamp,

    #[has_many]
    pub user_roles: toasty::HasMany<UserRole>,

    #[has_many]
    pub role_permissions: toasty::HasMany<RolePermission>,
}

impl Role {
    pub fn is_admin(&self) -> bool {
        self.name == DefaultRole::Superuser.name() || self.name == DefaultRole::Admin.name()
    }
}
