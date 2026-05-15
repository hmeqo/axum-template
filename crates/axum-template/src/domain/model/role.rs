use strum::{Display, EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};
use toasty::Model;

use super::{Perm, UserRole};

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

    pub fn default_permissions(&self) -> &[Perm] {
        DEFAULT_ROLE_PERMISSIONS
            .iter()
            .find(|(r, _)| r.name() == self.name())
            .map(|(_, perms)| *perms)
            .unwrap_or(&[])
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

    pub permissions: String,

    #[auto]
    pub created_at: jiff::Timestamp,

    #[auto]
    pub updated_at: jiff::Timestamp,

    #[has_many]
    pub user_roles: toasty::HasMany<UserRole>,
}

impl Role {
    pub fn is_admin(&self) -> bool {
        self.name == DefaultRole::Superuser.name() || self.name == DefaultRole::Admin.name()
    }

    pub fn perm_codes(&self) -> Vec<String> {
        serde_json::from_str(&self.permissions).unwrap_or_default()
    }

    pub fn set_perms(&mut self, perms: &[Perm]) {
        self.permissions = serde_json::to_string(perms).unwrap_or_default();
    }

    pub fn parse_perms(&self) -> Vec<Perm> {
        self.perm_codes()
            .iter()
            .filter_map(|c| Perm::from_code(c))
            .collect()
    }
}
