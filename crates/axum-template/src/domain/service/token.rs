use std::fmt;

use jsonwebtoken::{DecodingKey, EncodingKey, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use toasty::Db;
use uuid::Uuid;

use crate::{
    bail,
    domain::{
        db::Pk,
        model::{RefreshToken, User},
    },
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

#[derive(Clone)]
pub struct TokenService {
    db: Db,
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
    fn db(&self) -> Db {
        self.db.clone()
    }

    pub fn new(db: Db, jwt_secret: String, expires_in_seconds: u64) -> Self {
        Self {
            db,
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
            .err_kind_msg(ErrorKind::Internal, "Token generation failed")
    }

    pub fn decode_access_token(&self, token: &str) -> Result<Claims> {
        decode::<Claims>(token, &self.decoding, &Validation::default())
            .map(|d| d.claims)
            .err_kind_msg(ErrorKind::Unauthorized, "Invalid token")
    }

    pub async fn generate_refresh_token(&self, user_id: Pk) -> Result<String> {
        let mut db = self.db();
        let token = Uuid::new_v4().to_string();
        let expires_at = jiff::Timestamp::now() + jiff::Span::new().hours(30 * 24);
        toasty::create!(RefreshToken {
            user_id,
            token: token.clone(),
            expires_at,
        })
        .exec(&mut db)
        .await?;
        Ok(token)
    }

    pub async fn delete_all_refresh_tokens(&self, user_id: Pk) -> Result<()> {
        let mut db = self.db();
        let tokens = RefreshToken::all()
            .filter(RefreshToken::fields().user_id().eq(user_id))
            .exec(&mut db)
            .await?;
        for t in &tokens {
            RefreshToken::filter_by_id(t.id)
                .delete()
                .exec(&mut db)
                .await?;
        }
        Ok(())
    }

    pub async fn rotate_refresh_token(&self, refresh_token_str: &str) -> Result<RotatedTokens> {
        let mut db = self.db();

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
