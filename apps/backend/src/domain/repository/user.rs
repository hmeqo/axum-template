use sea_orm::{DatabaseConnection, QueryFilter, entity::prelude::*};
use std::sync::Arc;

use crate::{domain::db::Pk, error::Result};
use entity::{prelude::*, user};

#[derive(Debug)]
pub struct UserRepository {
    pub db: Arc<DatabaseConnection>,
}

impl UserRepository {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Insert a new user record
    pub async fn insert(&self, active_model: user::ActiveModel) -> Result<user::Model> {
        active_model.insert(self.db()).await.map_err(Into::into)
    }

    /// Update an existing user record
    pub async fn update(&self, active_model: user::ActiveModel) -> Result<user::Model> {
        active_model.update(self.db()).await.map_err(Into::into)
    }

    /// Delete a user by ID
    pub async fn delete_by_id(&self, id: Pk) -> Result<()> {
        User::delete_by_id(id).exec(self.db()).await?;
        Ok(())
    }

    /// Find a user by username
    pub async fn find_by_username(&self, username: &str) -> Result<Option<user::Model>> {
        User::find()
            .filter(user::Column::Username.eq(username))
            .one(self.db())
            .await
            .map_err(Into::into)
    }

    /// Find a user by ID
    pub async fn find_by_id(&self, id: Pk) -> Result<Option<user::Model>> {
        User::find_by_id(id)
            .one(self.db())
            .await
            .map_err(Into::into)
    }

    /// List users with pagination
    pub async fn list(&self, page: u64, per_page: u64) -> Result<Vec<user::Model>> {
        User::find()
            .paginate(self.db(), per_page)
            .fetch_page(page)
            .await
            .map_err(Into::into)
    }

    /// Count the total number of users
    pub async fn count(&self) -> Result<u64> {
        User::find().count(self.db()).await.map_err(Into::into)
    }

    /// Check whether a username already exists
    pub async fn exists_by_username(&self, username: &str) -> Result<bool> {
        Ok(self.find_by_username(username).await?.is_some())
    }
}
