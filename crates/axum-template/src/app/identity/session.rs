use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use cookie::time::Duration;

use super::{CredentialSpec, Resolved, SESSION_COOKIE_NOT_FOUND};
use crate::{
    app::AppContext,
    bail,
    config::SameSite as SessionSameSite,
    domain::vo::{SessionId, UserId},
    error::{AppError, ErrorCode},
};

/// 会话 cookie：`Max-Age` 取该行剩余寿命。
pub(crate) fn session_cookie(
    ctx: &AppContext,
    token: &str,
    expires_at: jiff::Timestamp,
) -> Cookie<'static> {
    let session_cfg = &ctx.cfg().auth.session;
    let mut cookie = Cookie::new(session_cfg.cookie_name.clone(), token.to_owned());
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(session_cfg.secure);
    cookie.set_same_site(SameSite::from(session_cfg.same_site));
    let remaining = (expires_at.as_second() - jiff::Timestamp::now().as_second()).max(0);
    cookie.set_max_age(Some(Duration::seconds(remaining)));
    cookie
}

pub fn set_session_cookie(
    jar: CookieJar,
    state: &AppContext,
    token: &str,
    expires_at: jiff::Timestamp,
) -> CookieJar {
    jar.add(session_cookie(state, token, expires_at))
}

/// 本次请求续期后的会话 cookie 内容，由组认证层重发。
#[derive(Debug, Clone)]
pub(crate) struct RenewedSession {
    pub token: String,
    pub expires_at: jiff::Timestamp,
}

impl From<SessionSameSite> for SameSite {
    fn from(value: SessionSameSite) -> Self {
        match value {
            SessionSameSite::Lax => SameSite::Lax,
            SessionSameSite::Strict => SameSite::Strict,
            SessionSameSite::None => SameSite::None,
        }
    }
}

pub fn remove_session_cookie(jar: CookieJar, state: &AppContext) -> CookieJar {
    let name = state.cfg().auth.session.cookie_name.clone();
    let mut cookie = Cookie::new(name, "");
    cookie.set_path("/");
    jar.remove(cookie)
}

pub fn session_token<'a>(jar: &'a CookieJar, ctx: &AppContext) -> Option<&'a str> {
    jar.get(&ctx.cfg().auth.session.cookie_name)
        .map(Cookie::value)
}

pub(crate) const COOKIE: CredentialSpec = CredentialSpec {
    name: "session cookie",
    rejected_challenge: None,
};

/// 会话组产出的身份：主体，以及这条请求所在的会话。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionIdentity {
    pub user_id: UserId,
    pub session_id: SessionId,
}

impl SessionIdentity {
    /// 这条解析结果里的会话身份；缺席、无效或不是会话时 401。
    pub(crate) fn of(resolved: &Resolved) -> Result<Self, AppError> {
        match resolved {
            Resolved::Authenticated {
                user_id,
                session: Some(session_id),
                ..
            } => Ok(Self {
                user_id: *user_id,
                session_id: *session_id,
            }),
            Resolved::Authenticated { credential, .. } => Err(AppError::unauthenticated(format!(
                "Request is authenticated as {}, but this endpoint requires {}",
                credential.name, COOKIE.name
            ))),
            Resolved::Invalid { .. } => Err(resolved.rejection()),
            Resolved::Absent => Err(AppError::unauthenticated(SESSION_COOKIE_NOT_FOUND)),
        }
    }
}

/// 验证会话；过期时删除该行。剩余寿命不足半个 TTL 时顺带续期，把这次续期交给调用方重发 cookie。
async fn from_token(
    ctx: &AppContext,
    token: &str,
) -> Result<(SessionIdentity, Option<RenewedSession>), AppError> {
    let session = ctx
        .srv()
        .session
        .find_by_token(token)
        .await?
        .ok_or_else(|| AppError::unauthenticated("Invalid session"))?;

    let now = jiff::Timestamp::now();

    if session.is_expired(now) {
        ctx.srv().session.delete_by_token(token).await?;
        bail!(AppError::business(ErrorCode::AuthSessionExpired));
    }

    let renewed = ctx
        .srv()
        .session
        .renew_if_due(&session)
        .await?
        .map(|expires_at| RenewedSession {
            token: token.to_owned(),
            expires_at,
        });

    Ok((
        SessionIdentity {
            user_id: session.user_id,
            session_id: session.id,
        },
        renewed,
    ))
}

/// 会话 cookie 的解析：没有 cookie 是 `Ok(None)`，cookie 无效沿用 [`from_token`] 的错误。
pub(crate) async fn resolve(
    parts: &mut Parts,
    state: &AppContext,
) -> Result<Option<SessionIdentity>, AppError> {
    let jar = CookieJar::from_request_parts(parts, state)
        .await
        .map_err(|_| AppError::unauthenticated("Session cookie not readable"))?;

    let Some(token) = session_token(&jar, state) else {
        return Ok(None);
    };

    let (identity, renewed) = from_token(state, token).await?;
    if let Some(renewed) = renewed {
        parts.extensions.insert(renewed);
    }
    Ok(Some(identity))
}
