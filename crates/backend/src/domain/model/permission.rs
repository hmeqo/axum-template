use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};
use toasty::Model;
use utoipa::ToSchema;

use super::RolePermission;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    EnumString,
    EnumIter,
    IntoStaticStr,
    Serialize,
    Deserialize,
    Display,
    ToSchema,
)]
pub enum Perm {
    #[serde(rename = "*")]
    #[strum(serialize = "*")]
    All,

    #[serde(rename = "user:read")]
    #[strum(serialize = "user:read")]
    UserRead,
    #[serde(rename = "user:write")]
    #[strum(serialize = "user:write")]
    UserWrite,
    #[serde(rename = "user:delete")]
    #[strum(serialize = "user:delete")]
    UserDelete,
    #[serde(rename = "user:*")]
    #[strum(serialize = "user:*")]
    UserAll,

    #[serde(rename = "role:read")]
    #[strum(serialize = "role:read")]
    RoleRead,
    #[serde(rename = "role:write")]
    #[strum(serialize = "role:write")]
    RoleWrite,
    #[serde(rename = "role:delete")]
    #[strum(serialize = "role:delete")]
    RoleDelete,
    #[serde(rename = "role:*")]
    #[strum(serialize = "role:*")]
    RoleAll,
}

impl Perm {
    pub fn from_code(code: &str) -> Option<Self> {
        code.parse().ok()
    }

    pub fn code(&self) -> &'static str {
        self.into()
    }

    pub const fn description(&self) -> &'static str {
        match self {
            Self::All => "超级用户",
            Self::UserRead => "查看用户信息",
            Self::UserWrite => "创建/修改用户",
            Self::UserDelete => "删除用户",
            Self::UserAll => "用户管理所有权限",
            Self::RoleRead => "查看角色信息",
            Self::RoleWrite => "创建/修改角色",
            Self::RoleDelete => "删除角色",
            Self::RoleAll => "角色管理所有权限",
        }
    }

    pub fn matches(&self, target_code: &str) -> bool {
        let self_code = self.code();

        if self_code == Perm::All.code() {
            return true;
        }

        if self_code == target_code {
            return true;
        }

        if let Some(prefix) = self_code.strip_suffix(":*") {
            if let Some(target_prefix) = target_code.split(':').next() {
                return prefix == target_prefix;
            }
        }

        false
    }

    pub fn all() -> Vec<Perm> {
        Self::iter().collect()
    }
}

#[derive(Debug, Clone, Model)]
pub struct Permission {
    #[key]
    #[auto]
    pub id: i64,

    #[unique]
    pub code: String,

    pub description: Option<String>,

    #[auto]
    pub created_at: jiff::Timestamp,

    #[has_many]
    pub role_permissions: toasty::HasMany<RolePermission>,
}

impl Permission {
    pub fn matches_code(&self, code: &str) -> bool {
        if self.code == "*" {
            return true;
        }

        if self.code == code {
            return true;
        }

        if let Some(prefix) = self.code.strip_suffix(":*") {
            if let Some(target_prefix) = code.split(':').next() {
                return prefix == target_prefix;
            }
        }

        false
    }

    pub fn permission_code(&self) -> String {
        self.code.clone()
    }
}

impl TryFrom<&Permission> for Perm {
    type Error = String;

    fn try_from(model: &Permission) -> Result<Self, Self::Error> {
        Self::from_code(&model.code)
            .ok_or_else(|| format!("Unknown permission code: {}", model.code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_permissions_consistency() {
        for perm in Perm::iter() {
            let serde_json_str = serde_json::to_string(&perm).expect("Serialize failed");
            let serde_val = serde_json_str.trim_matches('"');

            let strum_val: &'static str = perm.into();

            assert_eq!(
                serde_val, strum_val,
                "JSON and Strum values for {} are inconsistent: {} != {}",
                perm, serde_val, strum_val
            );
        }
    }
}
