use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};
use utoipa::ToSchema;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
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

pub fn perms_match(self_code: &str, target_code: &str) -> bool {
    if self_code == "*" {
        return true;
    }
    if self_code == target_code {
        return true;
    }
    if let Some(prefix) = self_code.strip_suffix(":*")
        && let Some(target_prefix) = target_code.split(':').next()
    {
        return prefix == target_prefix;
    }
    false
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
        perms_match(self.code(), target_code)
    }

    pub fn all() -> Vec<Perm> {
        Self::iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;

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
