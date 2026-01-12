use std::sync::Arc;

use crate::{
    domain::{
        db::Pk,
        model::{Permission, PermissionExt, RoleActiveModelExt},
        repository::{
            PermissionRepository, RolePermissionRepository, RoleRepository, UserRoleRepository,
        },
    },
    error::{ErrorKind, Result},
};
use entity::{permission, role};

/// Role CRUD service
#[derive(Debug)]
pub struct RoleService {
    pub role_repo: Arc<RoleRepository>,
    pub permission_repo: Arc<PermissionRepository>,
    pub user_role_repo: Arc<UserRoleRepository>,
    pub role_permission_repo: Arc<RolePermissionRepository>,
}

impl RoleService {
    /// Create a new role
    pub async fn create(&self, name: String, description: Option<String>) -> Result<role::Model> {
        if self.role_repo.exists_by_name(&name).await? {
            return Err(ErrorKind::AlreadyExists.with_message("Role already exists"));
        }

        let active = role::ActiveModel::new_role(name, description);
        self.role_repo.insert(active).await
    }

    /// Find role by ID
    pub async fn find_by_id(&self, id: Pk) -> Result<Option<role::Model>> {
        self.role_repo.find_by_id(id).await
    }

    /// Get role by ID or return not found
    pub async fn get_by_id(&self, id: Pk) -> Result<role::Model> {
        self.role_repo
            .find_by_id(id)
            .await?
            .ok_or(ErrorKind::NotFound.with_message("Role not found"))
    }

    /// Find role by name
    pub async fn find_by_name(&self, name: &str) -> Result<Option<role::Model>> {
        self.role_repo.find_by_name(name).await
    }

    /// Check if role exists by name
    pub async fn exists_by_name(&self, name: &str) -> Result<bool> {
        self.role_repo.exists_by_name(name).await
    }

    /// List roles with pagination
    pub async fn list(&self, page: u64, per_page: u64) -> Result<Vec<role::Model>> {
        self.role_repo.list(page, per_page).await
    }

    /// List all roles
    pub async fn list_all(&self) -> Result<Vec<role::Model>> {
        self.role_repo.list_all().await
    }

    /// Count total roles
    pub async fn count(&self) -> Result<u64> {
        self.role_repo.count().await
    }

    /// Update role name
    pub async fn update_name(&self, id: Pk, new_name: String) -> Result<role::Model> {
        let current = self.get_by_id(id).await?;

        if new_name != current.name && self.exists_by_name(&new_name).await? {
            return Err(ErrorKind::AlreadyExists.with_message("Role name already exists"));
        }

        let mut active: role::ActiveModel = current.into();
        active.set_name(new_name);

        self.role_repo.update(active).await
    }

    /// Update role description
    pub async fn update_description(
        &self,
        id: Pk,
        description: Option<String>,
    ) -> Result<role::Model> {
        let current = self.get_by_id(id).await?;

        let mut active: role::ActiveModel = current.into();
        active.set_description(description);

        self.role_repo.update(active).await
    }

    /// Delete role by ID
    pub async fn delete(&self, id: Pk) -> Result<()> {
        let _ = self.get_by_id(id).await?;
        self.role_repo.delete_by_id(id).await
    }

    /// Add permission to role
    pub async fn add_permission(&self, role_id: Pk, perm_id: Pk) -> Result<()> {
        let _ = self.get_by_id(role_id).await?;

        self.permission_repo
            .find_by_id(perm_id)
            .await?
            .ok_or_else(|| ErrorKind::NotFound.with_message("Permission not found"))?;

        if self.role_permission_repo.exists(role_id, perm_id).await? {
            return Err(
                ErrorKind::AlreadyExists.with_message("Permission already assigned to role")
            );
        }

        self.role_permission_repo.assign(role_id, perm_id).await
    }

    /// Remove permission from role
    pub async fn remove_permission(&self, role_id: Pk, perm_id: Pk) -> Result<()> {
        self.role_permission_repo.remove(role_id, perm_id).await
    }

    /// Get all permissions for a role
    pub async fn get_permissions(&self, role_id: Pk) -> Result<Vec<permission::Model>> {
        let ids = self
            .role_permission_repo
            .permission_ids_for_role(role_id)
            .await?;
        self.permission_repo.list_by_ids(ids).await
    }

    /// Check if role has a specific permission
    pub async fn has_permission(&self, role_id: Pk, perm: Permission) -> Result<bool> {
        Ok(self
            .get_permissions(role_id)
            .await?
            .iter()
            .any(|p| p.matches(perm)))
    }

    /// Assign role to user
    pub async fn assign_to_user(&self, user_id: Pk, role_id: Pk) -> Result<()> {
        let _ = self.get_by_id(role_id).await?;
        if self.user_role_repo.exists(user_id, role_id).await? {
            return Err(ErrorKind::AlreadyExists.with_message("Role already assigned to user"));
        }
        self.user_role_repo.assign(user_id, role_id).await
    }

    /// Remove role from user
    pub async fn remove_from_user(&self, user_id: Pk, role_id: Pk) -> Result<()> {
        self.user_role_repo.remove(user_id, role_id).await
    }

    /// Get all roles for a user
    pub async fn get_user_roles(&self, user_id: Pk) -> Result<Vec<role::Model>> {
        let role_ids = self.user_role_repo.role_ids_for_user(user_id).await?;
        self.role_repo.find_by_ids(role_ids).await
    }

    /// Get all permissions for a user (through their roles)
    pub async fn get_user_permissions(&self, user_id: Pk) -> Result<Vec<permission::Model>> {
        let roles = self.get_user_roles(user_id).await?;

        let mut all_permissions = Vec::new();
        for role in roles {
            let perms = self.get_permissions(role.id).await?;
            all_permissions.extend(perms);
        }

        all_permissions.sort_by_key(|p| p.id);
        all_permissions.dedup_by_key(|p| p.id);
        Ok(all_permissions)
    }

    /// Check if user has a specific permission
    pub async fn user_has_permission(
        &self,
        user_id: Pk,
        resource: &str,
        action: &str,
    ) -> Result<bool> {
        let permissions = self.get_user_permissions(user_id).await?;
        Ok(permissions
            .iter()
            .any(|p| p.resource == resource && p.action == action))
    }
}
