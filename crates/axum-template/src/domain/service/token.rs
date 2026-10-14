use std::fmt;

use jsonwebtoken::{DecodingKey, EncodingKey, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use toasty::Db;
use uuid::Uuid;

use crate::{
    domain::{
        db::token as token_db,
        model::{RefreshToken, User},
        service::UserService,
        vo::{ClientInfo, UserId},
    },
    error::{AppError, ErrorCode, Result},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: UserId,
    pub username: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone)]
pub struct RotatedTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub user: User,
}

#[derive(Clone)]
pub struct TokenService {
    db: Db,
    user: UserService,
    encoding: EncodingKey,
    decoding: DecodingKey,
    expires_in_seconds: u64,
}

impl fmt::Debug for TokenService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenService")
            .field("db", &self.db)
            .field("expires_in_seconds", &self.expires_in_seconds)
            .finish()
    }
}

impl TokenService {
    pub fn new(db: Db, user: UserService, jwt_secret: String, expires_in_seconds: u64) -> Self {
        Self {
            db,
            user,
            encoding: EncodingKey::from_secret(jwt_secret.as_ref()),
            decoding: DecodingKey::from_secret(jwt_secret.as_ref()),
            expires_in_seconds,
        }
    }

    pub fn encode_access_token(&self, user: &User) -> Result<String> {
        let now = jiff::Timestamp::now().as_second() as usize;
        let claims = Claims {
            sub: user.id,
            username: user.username.clone(),
            exp: now + self.expires_in_seconds as usize,
            iat: now,
        };
        encode(&Default::default(), &claims, &self.encoding)
            .map_err(|e| AppError::internal(format!("Token generation failed: {e}")))
    }

    pub fn decode_access_token(&self, token: &str) -> Result<Claims> {
        decode::<Claims>(token, &self.decoding, &Validation::default())
            .map(|d| d.claims)
            .map_err(|_| AppError::unauthenticated("Invalid token"))
    }

    pub async fn generate_refresh_token(
        &self,
        user_id: UserId,
        client: &ClientInfo,
    ) -> Result<String> {
        let mut db = self.db.clone();
        let token = Uuid::new_v4().to_string();
        let expires_at = RefreshToken::new_expiry(jiff::Timestamp::now());
        toasty::create!(RefreshToken {
            user_id,
            token: token.clone(),
            expires_at,
            ip: client.ip.map(|ip| ip.to_string()),
            user_agent: client.user_agent.clone(),
        })
        .exec(&mut db)
        .await?;
        Ok(token)
    }

    pub async fn delete_all_refresh_tokens(&self, user_id: UserId) -> Result<()> {
        let mut db = self.db.clone();
        token_db::delete_refresh_tokens_by_user(&mut db, user_id).await
    }

    pub async fn rotate_refresh_token(
        &self,
        refresh_token_str: &str,
        client: &ClientInfo,
    ) -> Result<RotatedTokens> {
        let mut db = self.db.clone();

        let stored = token_db::take_refresh_token(&mut db, refresh_token_str).await?;

        token_db::consume_refresh_token(&mut db, stored.id).await?;

        let user = self
            .user
            .find_by_id(stored.user_id)
            .await?
            .ok_or_else(|| AppError::business(ErrorCode::AuthRefreshTokenInvalid))?;
        let access_token = self.encode_access_token(&user)?;
        let refresh_token = self.generate_refresh_token(user.id, client).await?;

        Ok(RotatedTokens {
            access_token,
            refresh_token,
            user,
        })
    }
}
