use chrono::Utc;
use entity::permission;
use sea_orm::ActiveValue::Set;
use strum::{AsRefStr, EnumIter, EnumString, IntoStaticStr};

/// 系统资源类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, EnumIter, AsRefStr, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum Resource {
    User,
    Role,
    Permission,
    Admin,
}

/// 操作类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, EnumIter, AsRefStr, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum Action {
    Create,
    Read,
    Update,
    Delete,
    List,
    Assign,
    #[strum(serialize = "*")]
    All,
}

/// 权限枚举 - 每个变体代表一个具体权限
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
pub enum Permission {
    // User management
    UserCreate,
    UserRead,
    UserUpdate,
    UserDelete,
    UserList,
    // Role management
    RoleCreate,
    RoleRead,
    RoleUpdate,
    RoleDelete,
    RoleList,
    RoleAssign,
    // Permission management
    PermissionRead,
    PermissionList,
    // Admin
    AdminAll,
}

impl Permission {
    /// 从资源和操作创建权限枚举
    pub fn from_resource_action(resource: &str, action: &str) -> Option<Self> {
        match (resource, action) {
            ("user", "create") => Some(Self::UserCreate),
            ("user", "read") => Some(Self::UserRead),
            ("user", "update") => Some(Self::UserUpdate),
            ("user", "delete") => Some(Self::UserDelete),
            ("user", "list") => Some(Self::UserList),
            ("role", "create") => Some(Self::RoleCreate),
            ("role", "read") => Some(Self::RoleRead),
            ("role", "update") => Some(Self::RoleUpdate),
            ("role", "delete") => Some(Self::RoleDelete),
            ("role", "list") => Some(Self::RoleList),
            ("role", "assign") => Some(Self::RoleAssign),
            ("permission", "read") => Some(Self::PermissionRead),
            ("permission", "list") => Some(Self::PermissionList),
            ("admin", "*") => Some(Self::AdminAll),
            _ => None,
        }
    }

    /// 获取资源类型和操作类型
    pub const fn resource_action(&self) -> (Resource, Action) {
        match self {
            Self::UserCreate => (Resource::User, Action::Create),
            Self::UserRead => (Resource::User, Action::Read),
            Self::UserUpdate => (Resource::User, Action::Update),
            Self::UserDelete => (Resource::User, Action::Delete),
            Self::UserList => (Resource::User, Action::List),
            Self::RoleCreate => (Resource::Role, Action::Create),
            Self::RoleRead => (Resource::Role, Action::Read),
            Self::RoleUpdate => (Resource::Role, Action::Update),
            Self::RoleDelete => (Resource::Role, Action::Delete),
            Self::RoleList => (Resource::Role, Action::List),
            Self::RoleAssign => (Resource::Role, Action::Assign),
            Self::PermissionRead => (Resource::Permission, Action::Read),
            Self::PermissionList => (Resource::Permission, Action::List),
            Self::AdminAll => (Resource::Admin, Action::All),
        }
    }

    /// 获取资源类型
    pub const fn resource(&self) -> Resource {
        self.resource_action().0
    }

    /// 获取操作类型
    pub const fn action(&self) -> Action {
        self.resource_action().1
    }

    /// 获取描述
    pub const fn description(&self) -> &'static str {
        match self {
            Self::UserCreate => "Create new users",
            Self::UserRead => "View user information",
            Self::UserUpdate => "Update user information",
            Self::UserDelete => "Delete users",
            Self::UserList => "List all users",
            Self::RoleCreate => "Create new roles",
            Self::RoleRead => "View role information",
            Self::RoleUpdate => "Update role information",
            Self::RoleDelete => "Delete roles",
            Self::RoleList => "List all roles",
            Self::RoleAssign => "Assign roles to users",
            Self::PermissionRead => "View permission information",
            Self::PermissionList => "List all permissions",
            Self::AdminAll => "All admin permissions",
        }
    }

    pub fn matches(&self, resource: &str, action: &str) -> bool {
        (self.resource_str() == resource || self.resource_str() == Resource::Admin.as_ref())
            && (self.action_str() == action || self.action_str() == Action::All.as_ref())
    }

    /// 获取完整权限名 (resource:action)
    pub fn full_name(&self) -> String {
        format!("{}:{}", self.resource().as_ref(), self.action().as_ref())
    }

    /// 获取资源字符串
    pub fn resource_str(&self) -> &'static str {
        self.resource().into()
    }

    /// 获取操作字符串
    pub fn action_str(&self) -> &'static str {
        self.action().into()
    }
}

// 从数据库模型转换为权限枚举
impl TryFrom<&permission::Model> for Permission {
    type Error = String;

    fn try_from(model: &permission::Model) -> Result<Self, Self::Error> {
        Self::from_resource_action(&model.resource, &model.action)
            .ok_or_else(|| format!("Unknown permission: {}:{}", model.resource, model.action))
    }
}

// 从权限枚举转换为数据库模型
impl From<Permission> for permission::ActiveModel {
    fn from(perm: Permission) -> Self {
        permission::ActiveModel {
            resource: Set(perm.resource_str().to_string()),
            action: Set(perm.action_str().to_string()),
            description: Set(Some(perm.description().to_string())),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        }
    }
}

pub trait PermissionExt {
    /// 判断是否匹配指定的资源和操作
    fn matches(&self, perm: Permission) -> bool;

    /// 获取权限全名 (resource:action)
    fn full_name(&self) -> String;
}

/// Permission ActiveModel 创建方法
pub trait PermissionActiveModelExt {
    /// 创建新权限的 ActiveModel
    fn new_permission(
        resource: String,
        action: String,
        description: Option<String>,
    ) -> permission::ActiveModel;

    /// 更新描述
    fn set_description(&mut self, description: Option<String>);
}

impl PermissionExt for permission::Model {
    fn matches(&self, perm: Permission) -> bool {
        perm.matches(&self.resource, &self.action)
    }

    fn full_name(&self) -> String {
        format!("{}:{}", self.resource, self.action)
    }
}

impl PermissionActiveModelExt for permission::ActiveModel {
    fn new_permission(
        resource: String,
        action: String,
        description: Option<String>,
    ) -> permission::ActiveModel {
        permission::ActiveModel {
            resource: Set(resource),
            action: Set(action),
            description: Set(description),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        }
    }

    fn set_description(&mut self, description: Option<String>) {
        self.description = Set(description);
    }
}
