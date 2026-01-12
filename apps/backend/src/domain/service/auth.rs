use std::sync::Arc;

use crate::{
    domain::{db::Pk, model::UserPrincipal, service::UserService},
    error::Result,
    util::password,
};
use entity::user;

#[derive(Debug)]
pub struct AuthService {
    pub user_service: Arc<UserService>,
}

impl AuthService {
    /// Authenticate user with username and password
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<user::Model>> {
        let user = self.user_service.find_by_username(username).await?;

        match user {
            Some(user) if password::verify(password, &user.password)? => Ok(Some(user)),
            _ => Ok(None),
        }
    }

    pub async fn get_user(&self, user_id: Pk) -> Result<Option<UserPrincipal>> {
        self.user_service
            .find_by_id(user_id)
            .await
            .map(|r| r.map(UserPrincipal::new))
    }
}
