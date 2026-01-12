use axum_login::AuthUser;
use entity::{permission, role, user};
use serde::{Deserialize, Serialize};

use crate::domain::db::Pk;

use super::{Permission, PermissionExt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPrincipal {
    pub user: user::Model,
    pub roles: Vec<role::Model>,
    pub permissions: Vec<permission::Model>,
}

impl AuthUser for UserPrincipal {
    type Id = Pk;

    fn id(&self) -> Self::Id {
        self.user.id
    }

    /// 返回 session 认证哈希
    ///
    /// 使用密码哈希作为 session 验证依据，
    /// 当密码改变时，所有现有的 session 将失效
    fn session_auth_hash(&self) -> &[u8] {
        self.user.password.as_bytes()
    }
}

impl UserPrincipal {
    pub fn new(user: user::Model) -> Self {
        Self {
            user,
            roles: Vec::new(),
            permissions: Vec::new(),
        }
    }

    pub fn with_roles(self, roles: Vec<role::Model>) -> Self {
        Self {
            user: self.user,
            roles,
            permissions: self.permissions,
        }
    }

    pub fn with_permissions(self, permissions: Vec<permission::Model>) -> Self {
        Self {
            user: self.user,
            roles: self.roles,
            permissions,
        }
    }

    pub fn has_role(&self, role_name: &str) -> bool {
        self.roles.iter().any(|r| r.name == role_name)
    }

    pub fn has_permission(&self, perm: Permission) -> bool {
        self.permissions.iter().any(|p| p.matches(perm))
    }

    pub fn get_role_names(&self) -> Vec<&str> {
        self.roles.iter().map(|r| r.name.as_str()).collect()
    }

    pub fn get_permission_names(&self) -> Vec<String> {
        self.permissions.iter().map(|p| p.full_name()).collect()
    }

    pub fn add_role(&mut self, role: role::Model) {
        if !self.has_role(&role.name) {
            self.roles.push(role);
        }
    }
}
