//! 用户名与新设密码的构造约束。

use crate::{
    bail,
    error::{AppError, ErrorCode, Result},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Username(String);

impl Username {
    pub const MIN_CHARS: usize = 3;
    pub const MAX_CHARS: usize = 20;

    pub fn new(raw: impl Into<String>) -> Result<Self> {
        let raw = raw.into();
        check_len(&raw, Self::MIN_CHARS, Self::MAX_CHARS, "Username")?;
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Password(String);

impl Password {
    pub const MIN_CHARS: usize = 8;
    pub const MAX_CHARS: usize = 32;

    pub fn new(raw: impl Into<String>) -> Result<Self> {
        let raw = raw.into();
        check_len(&raw, Self::MIN_CHARS, Self::MAX_CHARS, "Password")?;
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn check_len(raw: &str, min: usize, max: usize, label: &str) -> Result<()> {
    if (min..=max).contains(&raw.chars().count()) {
        return Ok(());
    }

    bail!(AppError::business_msg(
        ErrorCode::InputInvalidFields,
        format!("{label} must be between {min} and {max} characters"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_bounds_are_inclusive() {
        assert_eq!(Username::new("abc").unwrap().as_str(), "abc");
        assert_eq!(Username::new("a".repeat(20)).unwrap().as_str().len(), 20);
        assert!(Username::new("ab").is_err());
        assert!(Username::new("a".repeat(21)).is_err());
    }

    #[test]
    fn password_bounds_are_inclusive() {
        assert!(Password::new("12345678").is_ok());
        assert_eq!(Password::new("a".repeat(32)).unwrap().as_str().len(), 32);
        assert!(Password::new("1234567").is_err());
        assert!(Password::new("a".repeat(33)).is_err());
    }

    #[test]
    fn bounds_count_characters_not_bytes() {
        assert!(Username::new("汉".repeat(20)).is_ok());
        assert!(Password::new("密".repeat(11)).is_ok());
    }

    #[test]
    fn violations_are_client_visible_input_errors() {
        let err = Username::new("ab").unwrap_err();
        assert_eq!(err.code(), Some(ErrorCode::InputInvalidFields));
    }
}
