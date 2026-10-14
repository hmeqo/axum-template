//! 请求级身份：组认证后端解析一次写入请求扩展，提取器只读它。

pub mod backend;
pub mod jwt;
pub mod session;

use axum::{
    extract::{FromRequestParts, OptionalFromRequestParts},
    http::{Extensions, request::Parts},
};
pub use backend::{AuthBackend, JwtAuth, JwtOrSession, SessionAuth};
pub use session::{SessionIdentity, remove_session_cookie, session_token, set_session_cookie};

use crate::{
    app::AppContext,
    domain::vo::{SessionId, UserId},
    error::{AppError, ErrorCode},
};

/// 没挂认证层或凭证缺席时的 401 文案。
pub(crate) const AUTHENTICATION_REQUIRED: &str = "Authentication required";

/// 会话端点缺凭证时的 401 文案。
pub(crate) const SESSION_COOKIE_NOT_FOUND: &str = "Session cookie not found";

/// 一种凭证的对外说法：401 文案里的名字，以及被拒时广告的 `WWW-Authenticate`。
///
/// crate 不枚举凭证：再加一种来源，自己给一份即可。
#[derive(Debug, Clone, Copy)]
pub struct CredentialSpec {
    pub name: &'static str,
    /// 被拒时广告的挑战（RFC 6750 §3 的 `error="invalid_token"`）。
    pub rejected_challenge: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrentUser {
    pub user_id: UserId,
}

impl CurrentUser {
    /// 这条解析结果里的主体；缺席或无效时 401。
    pub(crate) fn of(resolved: &Resolved) -> Result<Self, AppError> {
        match resolved {
            Resolved::Authenticated { user_id, .. } => Ok(Self { user_id: *user_id }),
            _ => Err(resolved.rejection()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Resolved {
    Authenticated {
        user_id: UserId,
        /// 只有会话 cookie 那条来源才有。
        session: Option<SessionId>,
        credential: CredentialSpec,
    },
    /// 见到了凭证但校验不过；`code` 保留原始错误的业务码。
    Invalid {
        credential: CredentialSpec,
        code: Option<ErrorCode>,
    },
    Absent,
}

impl Resolved {
    pub(crate) fn cached(extensions: &Extensions) -> Option<Self> {
        extensions.get::<Self>().copied()
    }

    /// 缺席或无效时的 401：`Invalid` 复用原始业务码，其余是统一文案。
    pub(crate) fn rejection(&self) -> AppError {
        match self {
            Self::Invalid { credential, code } => match code {
                Some(code) => AppError::business(*code),
                None => AppError::unauthenticated(format!("Invalid {}", credential.name)),
            },
            _ => AppError::unauthenticated(AUTHENTICATION_REQUIRED),
        }
    }

    pub(crate) fn rejected_challenge(&self) -> Option<&'static str> {
        match self {
            Self::Invalid { credential, .. } => credential.rejected_challenge,
            _ => None,
        }
    }
}

/// 本组后端解析出的身份；组没挂层、身份类型不符或凭证无效时 401。
#[derive(Debug)]
pub struct Auth<B: AuthBackend>(pub B::Identity);

impl<B: AuthBackend> FromRequestParts<AppContext> for Auth<B> {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppContext,
    ) -> Result<Self, Self::Rejection> {
        let resolved = Resolved::cached(&parts.extensions).unwrap_or(Resolved::Absent);
        B::identity(&resolved).map(Self)
    }
}

impl<B: AuthBackend> OptionalFromRequestParts<AppContext> for Auth<B> {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppContext,
    ) -> Result<Option<Self>, Self::Rejection> {
        let resolved = Resolved::cached(&parts.extensions).unwrap_or(Resolved::Absent);
        B::optional_identity(&resolved).map(|identity| identity.map(Self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_identity_values_are_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<CurrentUser>();
        assert_copy::<Resolved>();
        assert_copy::<SessionIdentity>();
    }
}
