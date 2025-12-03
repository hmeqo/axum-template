use std::sync::Arc;

use crate::{
    domain::{model::UserActiveModelExt, repository::UserRepository},
    error::{ErrorKind, Result},
    util::password,
};
use entity::user;

/// User CRUD service
pub struct UserService {
    pub repo: Arc<UserRepository>,
}

impl UserService {
    /// Create a new user
    pub async fn create(&self, username: String, password: String) -> Result<user::Model> {
        if self.repo.exists_by_username(&username).await? {
            return Err(ErrorKind::Exists
                .with_detail("Username already exists")
                .into());
        }

        let hashed = password::hash(&password)?;
        let user = user::ActiveModel::new_user(username, hashed);

        self.repo.insert(user).await
    }

    /// Find user by ID
    pub async fn find_by_id(&self, id: i32) -> Result<Option<user::Model>> {
        self.repo.find_by_id(id).await
    }

    /// Get user by ID or return not found
    pub async fn get_by_id(&self, id: i32) -> Result<user::Model> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| ErrorKind::NotFound.with_detail("User not found").into())
    }

    /// Find user by username
    pub async fn find_by_username(&self, username: &str) -> Result<Option<user::Model>> {
        self.repo.find_by_username(username).await
    }

    /// Check if user exists by ID
    pub async fn exists(&self, id: i32) -> Result<bool> {
        Ok(self.find_by_id(id).await?.is_some())
    }

    /// Check if user exists by username
    pub async fn exists_by_username(&self, username: &str) -> Result<bool> {
        self.repo.exists_by_username(username).await
    }

    /// List users with pagination
    pub async fn list(&self, page: u64, per_page: u64) -> Result<Vec<user::Model>> {
        self.repo.list(page, per_page).await
    }

    /// Count total users
    pub async fn count(&self) -> Result<u64> {
        self.repo.count().await
    }

    /// Update username
    pub async fn update_username(&self, id: i32, new_username: String) -> Result<user::Model> {
        let user = self.get_by_id(id).await?;

        if new_username != user.username && self.exists_by_username(&new_username).await? {
            return Err(ErrorKind::Conflict
                .with_detail("Username already exists")
                .into());
        }

        let mut active: user::ActiveModel = user.into();
        active.set_username(new_username);

        self.repo.update(active).await
    }

    /// Change user password requiring current password match
    pub async fn change_password(
        &self,
        id: i32,
        old_password: &str,
        new_password: &str,
    ) -> Result<()> {
        let user = self.get_by_id(id).await?;

        if !password::verify(old_password, &user.password)? {
            return Err(ErrorKind::Unauthenticated
                .with_detail("Invalid old password")
                .into());
        }

        let hashed = password::hash(new_password)?;
        let mut active: user::ActiveModel = user.into();
        active.set_password(hashed);

        self.repo.update(active).await.map(|_| ())
    }

    /// Reset password without verifying current password
    pub async fn reset_password(&self, id: i32, new_password: &str) -> Result<()> {
        let user = self.get_by_id(id).await?;

        let hashed = password::hash(new_password)?;
        let mut active: user::ActiveModel = user.into();
        active.set_password(hashed);

        self.repo.update(active).await.map(|_| ())
    }

    /// Delete user by ID
    pub async fn delete(&self, id: i32) -> Result<()> {
        self.repo.delete_by_id(id).await
    }
}
