use jsonwebtoken::{DecodingKey, EncodingKey, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use toasty::Db;
use uuid::Uuid;

use crate::{
    bail,
    domain::{db::Pk, model::{RefreshToken, User}},
    error::{ErrorKind, Result, ResultExt},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Pk,
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

#[derive(Debug, Clone)]
pub struct TokenService {
    db: Db,
    jwt_secret: String,
}

impl TokenService {
    pub fn new(db: Db, jwt_secret: String) -> Self {
        Self { db, jwt_secret }
    }

    pub fn encode_access_token(&self, user: &User) -> Result<String> {
        let now = jiff::Timestamp::now().as_second() as usize;
        let claims = Claims {
            sub: user.id,
            username: user.username.clone(),
            exp: now + 3600,
            iat: now,
        };
        encode(
            &Default::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        )
        .err_kind_msg(ErrorKind::Internal, "Token generation failed")
    }

    pub fn decode_access_token(&self, token: &str) -> Result<Claims> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_ref()),
            &Validation::default(),
        )
        .map(|d| d.claims)
        .err_kind_msg(ErrorKind::Unauthorized, "Invalid token")
    }

    pub async fn generate_refresh_token(&self, user_id: Pk) -> Result<String> {
        let mut db = self.db.clone();
        let token = Uuid::new_v4().to_string();
        let expires_at = jiff::Timestamp::now() + jiff::Span::new().days(30);
        toasty::create!(RefreshToken {
            user_id,
            token: token.clone(),
            expires_at,
        })
        .exec(&mut db)
        .await?;
        Ok(token)
    }

    pub async fn rotate_refresh_token(&self, refresh_token_str: &str) -> Result<RotatedTokens> {
        let mut db = self.db.clone();

        let stored = RefreshToken::filter_by_token(refresh_token_str)
            .get(&mut db)
            .await
            .err_kind_msg(ErrorKind::Unauthorized, "Invalid refresh token")?;

        RefreshToken::filter_by_id(stored.id)
            .delete()
            .exec(&mut db)
            .await?;

        let now = jiff::Timestamp::now();
        if stored.expires_at < now {
            bail!(ErrorKind::Unauthorized, "Refresh token expired");
        }

        let user = User::get_by_id(&mut db, &stored.user_id).await?;
        let access_token = self.encode_access_token(&user)?;
        let refresh_token = self.generate_refresh_token(user.id).await?;

        Ok(RotatedTokens {
            access_token,
            refresh_token,
            user,
        })
    }
}
