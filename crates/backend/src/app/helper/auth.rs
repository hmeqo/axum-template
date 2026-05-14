use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use jsonwebtoken::{DecodingKey, EncodingKey, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    app::AppState,
    domain::{Services, db::Pk, model::User},
    error::{AppError, ErrorKind},
};

// --- JWT Claims & Token ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Pk,
    pub username: String,
    pub exp: usize,
    pub iat: usize,
}

pub fn encode_access_token(user_id: Pk, username: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = jiff::Timestamp::now().as_second() as usize;
    let claims = Claims {
        sub: user_id,
        username: username.to_owned(),
        exp: now + 3600, // 1 hour
        iat: now,
    };
    encode(&Default::default(), &claims, &EncodingKey::from_secret(secret.as_ref()))
}

pub fn decode_access_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(token, &DecodingKey::from_secret(secret.as_ref()), &Validation::default())
        .map(|d| d.claims)
}

pub fn generate_refresh_token() -> String {
    Uuid::new_v4().to_string()
}

// --- Auth Context Extractor ---

/// Authentication context extracted from JWT.
/// Holds user_id and username; lazy-loads full User via `.user(services)`.
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

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ErrorKind::Unauthorized)?;

        let secret = &state.config.load().auth.jwt_secret;
        let claims = decode_access_token(token, secret)
            .map_err(|_| ErrorKind::Unauthorized.msg("Invalid token"))?;

        Ok(AuthCtx {
            user_id: claims.sub,
            username: claims.username,
        })
    }
}
