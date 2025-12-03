use sea_orm::{DatabaseConnection, QueryFilter, entity::prelude::*};
use std::sync::Arc;

use crate::error::Result;
use entity::{prelude::*, role};

/// 角色 CRUD 操作
pub struct RoleRepository {
    pub db: Arc<DatabaseConnection>,
}

impl RoleRepository {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn insert(&self, active_model: role::ActiveModel) -> Result<role::Model> {
        active_model.insert(self.db()).await.map_err(Into::into)
    }

    pub async fn update(&self, active_model: role::ActiveModel) -> Result<role::Model> {
        active_model.update(self.db()).await.map_err(Into::into)
    }

    pub async fn delete_by_id(&self, id: i32) -> Result<()> {
        Role::delete_by_id(id).exec(self.db()).await?;
        Ok(())
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<role::Model>> {
        Role::find_by_id(id)
            .one(self.db())
            .await
            .map_err(Into::into)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<role::Model>> {
        Role::find()
            .filter(role::Column::Name.eq(name))
            .one(self.db())
            .await
            .map_err(Into::into)
    }

    pub async fn exists_by_name(&self, name: &str) -> Result<bool> {
        Ok(self.find_by_name(name).await?.is_some())
    }

    pub async fn list(&self, page: u64, per_page: u64) -> Result<Vec<role::Model>> {
        Role::find()
            .paginate(self.db(), per_page)
            .fetch_page(page)
            .await
            .map_err(Into::into)
    }

    pub async fn list_all(&self) -> Result<Vec<role::Model>> {
        Role::find().all(self.db()).await.map_err(Into::into)
    }

    pub async fn count(&self) -> Result<u64> {
        Role::find().count(self.db()).await.map_err(Into::into)
    }

    pub async fn find_by_ids(&self, ids: Vec<i32>) -> Result<Vec<role::Model>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        Role::find()
            .filter(role::Column::Id.is_in(ids))
            .all(self.db())
            .await
            .map_err(Into::into)
    }
}
