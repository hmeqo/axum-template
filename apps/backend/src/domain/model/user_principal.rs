use std::sync::Arc;

use arc_swap::ArcSwap;
use axum_login::AuthUser;
use entity::{permission, role, user};

use crate::domain::{
    db::Pk,
    model::{DefaultRole, PermissionExt},
    service::{RoleService, UserService},
};

use super::Perm;

#[derive(Debug, Clone)]
pub struct UserPrincipal {
    pub user_srv: Arc<UserService>,
    pub role_srv: Arc<RoleService>,
    pub user: user::Model,
    roles: Arc<ArcSwap<Option<Vec<role::Model>>>>,
    permissions: Arc<ArcSwap<Option<Vec<permission::Model>>>>,
}

impl AuthUser for UserPrincipal {
    type Id = Pk;

    fn id(&self) -> Self::Id {
        self.user.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.user.password.as_bytes()
    }
}

impl UserPrincipal {
    pub fn new(
        user_srv: &Arc<UserService>,
        role_srv: &Arc<RoleService>,
        user: user::Model,
    ) -> Self {
        Self {
            user_srv: Arc::clone(user_srv),
            role_srv: Arc::clone(role_srv),
            user,
            roles: Arc::new(ArcSwap::new(Arc::new(None))),
            permissions: Arc::new(ArcSwap::new(Arc::new(None))),
        }
    }

    pub async fn roles(&self) -> Arc<Vec<role::Model>> {
        let guard = self.roles.load();
        if let Some(roles) = guard.as_ref().as_ref() {
            return Arc::new(roles.clone());
        }
        drop(guard);

        let roles = match self.role_srv.get_user_roles(self.user.id).await {
            Ok(roles) => roles,
            Err(_) => Vec::new(),
        };

        let roles_arc = Arc::new(roles);
        self.roles.store(Arc::new(Some((*roles_arc).clone())));
        roles_arc
    }

    pub async fn permissions(&self) -> Arc<Vec<permission::Model>> {
        let guard = self.permissions.load();
        if let Some(perms) = guard.as_ref().as_ref() {
            return Arc::new(perms.clone());
        }
        drop(guard);

        let perms = match self.role_srv.get_user_permissions(self.user.id).await {
            Ok(perms) => perms,
            Err(_) => Vec::new(),
        };

        let perms_arc = Arc::new(perms);
        self.permissions.store(Arc::new(Some((*perms_arc).clone())));
        perms_arc
    }

    pub async fn has_role(&self, role_name: &str) -> bool {
        self.roles().await.iter().any(|r| r.name == role_name)
    }

    pub async fn has_permission_by_code(&self, code: &str) -> bool {
        self.permissions()
            .await
            .iter()
            .any(|p| p.matches_code(code))
    }

    pub async fn has_permission(&self, perm: Perm) -> bool {
        self.has_permission_by_code(perm.code()).await
    }

    pub async fn role_ids(&self) -> std::collections::HashSet<i64> {
        self.roles().await.iter().map(|r| r.id).collect()
    }

    pub async fn is_superuser(&self) -> bool {
        self.has_role(DefaultRole::Superuser.name()).await
    }

    pub async fn is_admin(&self) -> bool {
        self.has_role(DefaultRole::Superuser.name()).await
            || self.has_role(DefaultRole::Admin.name()).await
    }

    pub async fn permissions_as_enum(&self) -> Vec<Perm> {
        self.permissions()
            .await
            .iter()
            .filter_map(|p| Perm::try_from(p).ok())
            .collect()
    }
}
