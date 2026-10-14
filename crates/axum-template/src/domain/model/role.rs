use strum::{Display, EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};
use toasty::Model;

use super::UserRole;
use crate::domain::vo::{Perm, RoleId};

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
            Self::Superuser => "Superuser",
            Self::Admin => "Administrator",
            Self::User => "Regular user",
        }
    }

    pub fn all() -> Vec<DefaultRole> {
        Self::iter().collect()
    }

    pub fn default_permissions(&self) -> &'static [Perm] {
        match self {
            Self::Superuser => &[Perm::All],
            Self::Admin => &[Perm::UserAll, Perm::RoleAll],
            Self::User => &[Perm::UserRead],
        }
    }
}

#[derive(Debug, Clone, Model)]
pub struct Role {
    #[key]
    #[auto]
    pub id: RoleId,

    #[unique]
    pub name: String,

    pub description: Option<String>,

    #[column(type = jsonb)]
    pub permissions: toasty::Json<Vec<Perm>>,

    #[auto]
    pub created_at: jiff::Timestamp,

    #[auto]
    pub updated_at: jiff::Timestamp,

    #[has_many]
    pub user_roles: toasty::Deferred<Vec<UserRole>>,
}

impl Role {
    pub fn is_admin(&self) -> bool {
        self.name == DefaultRole::Superuser.name() || self.name == DefaultRole::Admin.name()
    }

    pub fn perm_codes(&self) -> Vec<String> {
        self.permissions
            .iter()
            .map(|p| p.code().to_string())
            .collect()
    }

    pub fn permissions(&self) -> Vec<Perm> {
        self.permissions.to_vec()
    }
}
