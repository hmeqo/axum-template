use chrono::Utc;
use entity::role;
use sea_orm::ActiveValue::Set;
use strum::{Display, EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};

use crate::domain::model::Perm;

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

/// Role 领域模型行为扩展
pub trait RoleExt {
    /// 判断是否为管理员角色
    fn is_admin(&self) -> bool;
}

/// Role ActiveModel 创建方法
pub trait RoleActiveModelExt {
    /// 创建新角色的 ActiveModel
    fn new_role(name: String, description: Option<String>) -> role::ActiveModel;

    /// 更新角色名
    fn set_name(&mut self, name: String);

    /// 更新角色描述
    fn set_description(&mut self, description: Option<String>);
}

impl RoleActiveModelExt for role::ActiveModel {
    fn new_role(name: String, description: Option<String>) -> role::ActiveModel {
        let now = Utc::now().into();
        role::ActiveModel {
            name: Set(name),
            description: Set(description),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
    }

    fn set_name(&mut self, name: String) {
        self.name = Set(name);
        self.updated_at = Set(Utc::now().into());
    }

    fn set_description(&mut self, description: Option<String>) {
        self.description = Set(description);
        self.updated_at = Set(Utc::now().into());
    }
}
