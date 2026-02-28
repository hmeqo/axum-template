use std::sync::Arc;

use crate::{
    domain::{
        db::Pk,
        model::{Perm, PermissionActiveModelExt},
        repository::PermissionRepository,
    },
    error::{ErrorKind, Result},
};
use entity::permission;

/// Permission CRUD service
#[derive(Debug)]
pub struct PermissionService {
    pub repo: Arc<PermissionRepository>,
}

impl PermissionService {
    /// Create a new permission
    pub async fn create(&self, perm: Perm) -> Result<permission::Model> {
        let code = perm.code();

        if self.repo.exists_by_code(code).await? {
            return Err(ErrorKind::AlreadyExists.with_message("Permission already exists"));
        }

        let active = permission::ActiveModel::new_permission(
            code.to_owned(),
            Some(perm.description().to_owned()),
        );
        self.repo.insert(active).await
    }

    /// Find permission by ID
    pub async fn find_by_id(&self, id: Pk) -> Result<Option<permission::Model>> {
        self.repo.find_by_id(id).await
    }

    /// Get permission by ID or return not found
    pub async fn get_by_id(&self, id: Pk) -> Result<permission::Model> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| ErrorKind::NotFound.with_message("Permission not found"))
    }

    /// Find permission
    pub async fn find(&self, perm: Perm) -> Result<Option<permission::Model>> {
        self.repo.find_by_code(perm.code()).await
    }

    /// Check if permission exists
    pub async fn exists(&self, perm: Perm) -> Result<bool> {
        self.repo.exists_by_code(perm.code()).await
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
        id: Pk,
        description: Option<String>,
    ) -> Result<permission::Model> {
        let perm = self.get_by_id(id).await?;

        let mut active: permission::ActiveModel = perm.into();
        active.set_description(description);

        self.repo.update(active).await
    }

    /// Delete permission by ID
    pub async fn delete(&self, id: Pk) -> Result<()> {
        let _ = self.get_by_id(id).await?;
        self.repo.delete_by_id(id).await
    }
}
