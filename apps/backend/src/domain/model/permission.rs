use chrono::Utc;
use entity::permission;
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};
use utoipa::ToSchema;

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

        // 精确匹配
        if self_code == target_code {
            return true;
        }

        // 通配符匹配，如 "user:*" 匹配 "user:read"
        if let Some(prefix) = self_code.strip_suffix(":*") {
            if let Some(target_prefix) = target_code.split(':').next() {
                return prefix == target_prefix;
            }
        }

        false
    }

    /// 检查权限列表中是否包含目标权限
    pub fn check_permissions(permissions: &[String], target_code: &str) -> bool {
        permissions.iter().any(|perm| {
            // 超级管理员权限
            if perm == "*" {
                return true;
            }

            // 精确匹配
            if perm == target_code {
                return true;
            }

            // 通配符匹配
            if let Some(prefix) = perm.strip_suffix(":*") {
                if let Some(target_prefix) = target_code.split(':').next() {
                    return prefix == target_prefix;
                }
            }

            false
        })
    }

    /// 获取所有权限列表（用于初始化数据库）
    pub fn all() -> Vec<Perm> {
        Self::iter().collect()
    }
}

// 从数据库模型转换为权限枚举
impl TryFrom<&permission::Model> for Perm {
    type Error = String;

    fn try_from(model: &permission::Model) -> Result<Self, Self::Error> {
        Self::from_code(&model.code)
            .ok_or_else(|| format!("Unknown permission code: {}", model.code))
    }
}

// 从权限枚举转换为数据库模型
impl From<Perm> for permission::ActiveModel {
    fn from(perm: Perm) -> Self {
        permission::ActiveModel {
            code: Set(perm.code().to_string()),
            description: Set(Some(perm.description().to_string())),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        }
    }
}

/// 权限扩展 trait
pub trait PermissionExt {
    /// 判断是否匹配指定的权限代码
    fn matches_code(&self, code: &str) -> bool;

    /// 获取权限代码
    fn permission_code(&self) -> String;
}

impl PermissionExt for permission::Model {
    fn matches_code(&self, code: &str) -> bool {
        // 超级管理员权限匹配所有
        if self.code == "*" {
            return true;
        }

        // 精确匹配
        if self.code == code {
            return true;
        }

        // 通配符匹配
        if let Some(prefix) = self.code.strip_suffix(":*") {
            if let Some(target_prefix) = code.split(':').next() {
                return prefix == target_prefix;
            }
        }

        false
    }

    fn permission_code(&self) -> String {
        self.code.clone()
    }
}

/// Permission ActiveModel 创建方法
pub trait PermissionActiveModelExt {
    /// 创建新权限的 ActiveModel
    fn new_permission(code: String, description: Option<String>) -> permission::ActiveModel;

    /// 更新描述
    fn set_description(&mut self, description: Option<String>);
}

impl PermissionActiveModelExt for permission::ActiveModel {
    fn new_permission(code: String, description: Option<String>) -> permission::ActiveModel {
        permission::ActiveModel {
            code: Set(code),
            description: Set(description),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        }
    }

    fn set_description(&mut self, description: Option<String>) {
        self.description = Set(description);
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
