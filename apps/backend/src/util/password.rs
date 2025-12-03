//! Password hashing utilities using Argon2

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};

use crate::error::{AppError, Result};

/// Hash a password using Argon2
pub fn hash(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::internal(format!("Password hashing failed: {}", e)))?;
    Ok(hash.to_string())
}

/// Verify a password against a hash
pub fn verify(password: &str, hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AppError::internal(format!("Invalid password hash: {}", e)))?;

    let argon2 = Argon2::default();
    Ok(argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let password = "test_password_123";

        let hash = hash(password).unwrap();
        assert!(verify(password, &hash).unwrap());
        assert!(!verify("wrong_password", &hash).unwrap());
    }
}
