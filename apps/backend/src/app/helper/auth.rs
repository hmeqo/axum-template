use std::sync::Arc;

use axum_login::{AuthnBackend, UserId};

use crate::{
    domain::{model::UserPrincipal, service::AuthService},
    error::AppError,
};

/// Credentials for user authentication
#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// Authentication backend for axum-login
#[derive(Debug, Clone)]
pub struct Backend {
    pub auth_service: Arc<AuthService>,
}

impl AuthnBackend for Backend {
    type User = UserPrincipal;
    type Credentials = Credentials;
    type Error = AppError;

    /// Authenticate user with credentials (username/password)
    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        self.auth_service
            .authenticate(&creds.username, &creds.password)
            .await
    }

    /// Get user by ID (used for session validation)
    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        self.auth_service.get_user(*user_id).await
    }
}

/// Type alias for the AuthSession with our Backend
pub type AuthSession = axum_login::AuthSession<Backend>;
