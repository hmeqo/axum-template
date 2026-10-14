use argon2::{
    Argon2, PasswordHasher, PasswordVerifier, password_hash::phc::PasswordHash as PhcHash,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::vo::Password,
    error::{AppError, Result},
};

/// 密码列的列类型。
#[derive(Clone, PartialEq, Serialize, Deserialize, toasty::Embed, ToSchema)]
#[serde(transparent)]
pub struct PasswordHash(String);

impl std::fmt::Debug for PasswordHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PasswordHash(<redacted>)")
    }
}

impl PasswordHash {
    pub fn hash(plain: &Password) -> Result<Self> {
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(plain.as_str().as_bytes())
            .map_err(|e| AppError::internal(format!("Password hashing failed: {e}")))?;
        Ok(Self(hash.to_string()))
    }

    /// 校验既有密码：接受任意明文，登录时不套长度策略。
    pub fn verify(&self, plain: &str) -> Result<bool> {
        let parsed = PhcHash::new(&self.0)
            .map_err(|e| AppError::internal(format!("Invalid password hash: {e}")))?;
        Ok(Argon2::default()
            .verify_password(plain.as_bytes(), &parsed)
            .is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_roundtrip() {
        let hash = PasswordHash::hash(&Password::new("test_password_123").unwrap()).unwrap();
        assert!(hash.verify("test_password_123").unwrap());
        assert!(!hash.verify("wrong_password").unwrap());
    }
}
