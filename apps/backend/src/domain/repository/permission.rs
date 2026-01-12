use sea_orm::{DatabaseConnection, QueryFilter, entity::prelude::*};
use std::sync::Arc;

use crate::{domain::db::Pk, error::Result};
use entity::{permission, prelude::*};

#[derive(Debug)]
pub struct PermissionRepository {
    pub db: Arc<DatabaseConnection>,
}

impl PermissionRepository {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn insert(&self, active_model: permission::ActiveModel) -> Result<permission::Model> {
        active_model.insert(self.db()).await.map_err(Into::into)
    }

    pub async fn update(&self, active_model: permission::ActiveModel) -> Result<permission::Model> {
        active_model.update(self.db()).await.map_err(Into::into)
    }

    pub async fn delete_by_id(&self, id: Pk) -> Result<()> {
        Permission::delete_by_id(id).exec(self.db()).await?;
        Ok(())
    }

    pub async fn find_by_id(&self, id: Pk) -> Result<Option<permission::Model>> {
        Permission::find_by_id(id)
            .one(self.db())
            .await
            .map_err(Into::into)
    }

    pub async fn find_by_resource_action(
        &self,
        resource: &str,
        action: &str,
    ) -> Result<Option<permission::Model>> {
        Permission::find()
            .filter(permission::Column::Resource.eq(resource))
            .filter(permission::Column::Action.eq(action))
            .one(self.db())
            .await
            .map_err(Into::into)
    }

    pub async fn exists_by_resource_action(&self, resource: &str, action: &str) -> Result<bool> {
        Ok(self
            .find_by_resource_action(resource, action)
            .await?
            .is_some())
    }

    pub async fn list(&self, page: u64, per_page: u64) -> Result<Vec<permission::Model>> {
        Permission::find()
            .paginate(self.db(), per_page)
            .fetch_page(page)
            .await
            .map_err(Into::into)
    }

    pub async fn list_all(&self) -> Result<Vec<permission::Model>> {
        Permission::find().all(self.db()).await.map_err(Into::into)
    }

    pub async fn list_by_ids(&self, ids: Vec<Pk>) -> Result<Vec<permission::Model>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        Permission::find()
            .filter(permission::Column::Id.is_in(ids))
            .all(self.db())
            .await
            .map_err(Into::into)
    }

    pub async fn count(&self) -> Result<u64> {
        Permission::find()
            .count(self.db())
            .await
            .map_err(Into::into)
    }
}
