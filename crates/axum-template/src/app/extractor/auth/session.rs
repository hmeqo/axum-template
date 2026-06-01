use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use cookie::time::Duration;

use crate::{
    app::AppState,
    bail,
    domain::{Services, db::Pk, model::User},
    error::{AppError, ErrorKind, OptionAppExt, ResultExt},
};

pub fn set_session_cookie(jar: CookieJar, state: &AppState, session_id: &str) -> CookieJar {
    let name = state.cfg().auth.session.cookie_name.clone();
    let ttl_secs = state.cfg().auth.session.ttl_hours * 3600;
    let mut cookie = Cookie::new(name, session_id.to_owned());
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(Some(Duration::seconds(ttl_secs as i64)));
    jar.add(cookie)
}

pub fn remove_session_cookie(jar: CookieJar, state: &AppState) -> CookieJar {
    let name = state.cfg().auth.session.cookie_name.clone();
    let mut cookie = Cookie::new(name, "");
    cookie.set_path("/");
    jar.remove(cookie)
}

/// Authentication context extracted from session cookie.
#[derive(Debug)]
pub struct SessionCtx {
    pub user_id: Pk,
}

impl SessionCtx {
    pub async fn user(&self, services: &Services) -> Result<User, AppError> {
        services.user.get_by_id(self.user_id).await
    }
}

impl FromRequestParts<AppState> for SessionCtx {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .err_kind(ErrorKind::Unauthorized)?;

        let config = state.cfg();
        let session_id = jar
            .get(&config.auth.session.cookie_name)
            .ok_or_err_msg(ErrorKind::Unauthorized, "Session cookie not found")?
            .value()
            .to_owned();

        let session = state
            .srv()
            .session
            .find(&session_id)
            .await?
            .ok_or(ErrorKind::Unauthorized)?;

        if session.expires_at < jiff::Timestamp::now() {
            state.srv().session.delete(&session_id).await?;
            bail!(ErrorKind::Unauthorized, "Session expired");
        }

        if state.srv().session.should_extend(&session) {
            state.srv().session.extend(&session_id).await?;
        }

        Ok(SessionCtx {
            user_id: session.user_id,
        })
    }
}

impl FromRequestParts<AppState> for Option<SessionCtx> {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(SessionCtx::from_request_parts(parts, state).await.ok())
    }
}
