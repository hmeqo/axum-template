//! 认证后端：一个路由组一个后端，由它决定接受哪些凭证、按什么顺序解析。
//!
//! 组合也是后端（[`JwtOrSession`]）。

use axum::http::request::Parts;
#[cfg(feature = "webtransport")]
use axum::http::{HeaderMap, Request as HttpRequest};

use super::{CredentialSpec, CurrentUser, Resolved, SessionIdentity, jwt, session};
use crate::{
    app::AppContext,
    domain::vo::{SessionId, UserId},
    error::AppError,
};

pub trait AuthBackend: Clone + Send + Sync + 'static {
    type Identity: Copy + Send + Sync + 'static;

    /// 这条解析结果里的本类型身份；缺席、无效或身份类型不符时给出 401。
    fn identity(resolved: &Resolved) -> Result<Self::Identity, AppError>;

    /// `Option<Auth<Self>>` 的取值：本组不接受这条凭证（会话组遇到令牌身份）是 `None`，其余同
    /// [`Self::identity`]。
    fn optional_identity(resolved: &Resolved) -> Result<Option<Self::Identity>, AppError> {
        Self::identity(resolved).map(Some)
    }

    /// 凭证缺席是 [`Resolved::Absent`]，凭证无效是 [`Resolved::Invalid`]（不再尝试后续来源）。
    fn resolve(&self, parts: &mut Parts, ctx: &AppContext)
    -> impl Future<Output = Resolved> + Send;

    /// 401 该带的 `WWW-Authenticate`（RFC 9110 §15.5.2）；没有可命名的方案就是 `None`。
    /// 返回值必须是合法的 header 值。
    fn challenge(&self, _resolved: &Resolved) -> Option<&'static str> {
        None
    }
}

/// 缺席、无效、已认证三种定论只写在这里。
fn resolved<T>(
    result: Result<Option<T>, AppError>,
    credential: CredentialSpec,
    identity: impl FnOnce(T) -> (UserId, Option<SessionId>),
) -> Resolved {
    match result {
        Ok(Some(found)) => {
            let (user_id, session) = identity(found);
            Resolved::Authenticated {
                user_id,
                session,
                credential,
            }
        }
        Ok(None) => Resolved::Absent,
        Err(error) => Resolved::Invalid {
            credential,
            code: error.code(),
        },
    }
}

/// 只认会话 cookie。
#[derive(Debug, Clone, Copy)]
pub struct SessionAuth;

impl AuthBackend for SessionAuth {
    type Identity = SessionIdentity;

    fn identity(resolved: &Resolved) -> Result<Self::Identity, AppError> {
        SessionIdentity::of(resolved)
    }

    fn optional_identity(resolved: &Resolved) -> Result<Option<Self::Identity>, AppError> {
        match resolved {
            // 令牌身份不在任何会话上：可选提取就此收手，不是 401。
            Resolved::Authenticated { session: None, .. } => Ok(None),
            _ => Self::identity(resolved).map(Some),
        }
    }

    async fn resolve(&self, parts: &mut Parts, ctx: &AppContext) -> Resolved {
        resolved(
            session::resolve(parts, ctx).await,
            session::COOKIE,
            |identity| (identity.user_id, Some(identity.session_id)),
        )
    }
}

/// 只认 `Authorization: Bearer <token>`。
#[derive(Debug, Clone, Copy)]
pub struct JwtAuth;

impl AuthBackend for JwtAuth {
    type Identity = CurrentUser;

    fn identity(resolved: &Resolved) -> Result<Self::Identity, AppError> {
        CurrentUser::of(resolved)
    }

    async fn resolve(&self, parts: &mut Parts, ctx: &AppContext) -> Resolved {
        resolved(jwt::resolve(parts, ctx).await, jwt::BEARER, |user_id| {
            (user_id, None)
        })
    }

    fn challenge(&self, resolved: &Resolved) -> Option<&'static str> {
        Some(resolved.rejected_challenge().unwrap_or(jwt::CHALLENGE))
    }
}

/// 令牌优先，其次会话 cookie。
#[derive(Debug, Clone, Copy)]
pub struct JwtOrSession;

impl AuthBackend for JwtOrSession {
    type Identity = CurrentUser;

    fn identity(resolved: &Resolved) -> Result<Self::Identity, AppError> {
        CurrentUser::of(resolved)
    }

    async fn resolve(&self, parts: &mut Parts, ctx: &AppContext) -> Resolved {
        let resolved = JwtAuth.resolve(parts, ctx).await;
        if let Resolved::Absent = resolved {
            SessionAuth.resolve(parts, ctx).await
        } else {
            resolved
        }
    }

    fn challenge(&self, resolved: &Resolved) -> Option<&'static str> {
        JwtAuth.challenge(resolved)
    }
}

/// 非 HTTP 入口（如 WebTransport 的 CONNECT）复用同一个后端：把传输给出的头当请求头解析。
#[cfg(feature = "webtransport")]
pub(crate) async fn resolve_headers<B: AuthBackend>(
    backend: &B,
    headers: HeaderMap,
    ctx: &AppContext,
) -> Resolved {
    let (mut parts, _) = HttpRequest::new(()).into_parts();
    parts.headers = headers;
    backend.resolve(&mut parts, ctx).await
}
