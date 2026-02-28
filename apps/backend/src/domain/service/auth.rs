use std::sync::Arc;

use crate::{
    domain::{
        db::Pk,
        model::UserPrincipal,
        service::{RoleService, UserService},
    },
    error::Result,
    util::password,
};

#[derive(Debug)]
pub struct AuthService {
    pub user_service: Arc<UserService>,
    pub role_service: Arc<RoleService>,
}

impl AuthService {
    /// Authenticate user with username and password
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<UserPrincipal>> {
        let user = self.user_service.find_by_username(username).await?;

        match user {
            Some(user) if password::verify(password, &user.password)? => Ok(Some(
                UserPrincipal::new(&self.user_service, &self.role_service, user),
            )),
            _ => Ok(None),
        }
    }

    pub async fn get_user(&self, user_id: Pk) -> Result<Option<UserPrincipal>> {
        self.user_service
            .find_by_id(user_id)
            .await
            .map(|uon| uon.map(|u| UserPrincipal::new(&self.user_service, &self.role_service, u)))
    }
}
