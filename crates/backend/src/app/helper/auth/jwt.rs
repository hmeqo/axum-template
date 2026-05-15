use axum::{extract::FromRequestParts, http::request::Parts};

use crate::{
    app::AppState,
    domain::{Services, db::Pk, model::User},
    error::{AppError, ErrorKind},
};

/// Authentication context extracted from the request.
///
/// Currently supports JWT Bearer tokens; future iterations will
/// also support session cookie auth, trying JWT first then falling
/// back to session.
#[derive(Debug)]
pub struct AuthCtx {
    pub user_id: Pk,
    username: String,
}

impl AuthCtx {
    pub fn username(&self) -> &str {
        &self.username
    }

    pub async fn user(&self, services: &Services) -> Result<User, AppError> {
        services.user.get_by_id(self.user_id).await
    }
}

impl FromRequestParts<AppState> for AuthCtx {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ErrorKind::Unauthorized)?;

        let claims = state
            .services()
            .token
            .decode_access_token(token)?;

        Ok(AuthCtx {
            user_id: claims.sub,
            username: claims.username,
        })
    }
}
