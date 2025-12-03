use std::sync::Arc;

use crate::{
    domain::{model::PermissionActiveModelExt, repository::PermissionRepository},
    error::{ErrorKind, Result},
};
use entity::permission;

/// Permission CRUD service
pub struct PermissionService {
    pub repo: Arc<PermissionRepository>,
}

impl PermissionService {
    /// Create a new permission
    pub async fn create(
        &self,
        resource: String,
        action: String,
        description: Option<String>,
    ) -> Result<permission::Model> {
        if self
            .repo
            .exists_by_resource_action(&resource, &action)
            .await?
        {
            return Err(ErrorKind::Exists
                .with_detail("Permission already exists")
                .into());
        }

        let active = permission::ActiveModel::new_permission(resource, action, description);
        self.repo.insert(active).await
    }

    /// Find permission by ID
    pub async fn find_by_id(&self, id: i32) -> Result<Option<permission::Model>> {
        self.repo.find_by_id(id).await
    }

    /// Get permission by ID or return not found
    pub async fn get_by_id(&self, id: i32) -> Result<permission::Model> {
        self.repo.find_by_id(id).await?.ok_or_else(|| {
            ErrorKind::NotFound
                .with_detail("Permission not found")
                .into()
        })
    }

    /// Find permission by resource and action
    pub async fn find_by_resource_action(
        &self,
        resource: &str,
        action: &str,
    ) -> Result<Option<permission::Model>> {
        self.repo.find_by_resource_action(resource, action).await
    }

    /// Check if permission exists by resource/action
    pub async fn exists_by_resource_action(&self, resource: &str, action: &str) -> Result<bool> {
        self.repo.exists_by_resource_action(resource, action).await
    }

    /// List permissions with pagination
    pub async fn list(&self, page: u64, per_page: u64) -> Result<Vec<permission::Model>> {
        self.repo.list(page, per_page).await
    }

    /// List all permissions
    pub async fn list_all(&self) -> Result<Vec<permission::Model>> {
        self.repo.list_all().await
    }

    /// Count total permissions
    pub async fn count(&self) -> Result<u64> {
        self.repo.count().await
    }

    /// Update permission description
    pub async fn update_description(
        &self,
        id: i32,
        description: Option<String>,
    ) -> Result<permission::Model> {
        let perm = self.get_by_id(id).await?;

        let mut active: permission::ActiveModel = perm.into();
        active.set_description(description);

        self.repo.update(active).await
    }

    /// Delete permission by ID
    pub async fn delete(&self, id: i32) -> Result<()> {
        let _ = self.get_by_id(id).await?;
        self.repo.delete_by_id(id).await
    }
}
