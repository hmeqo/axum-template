use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::cookie::{Cookie, CookieJar};

use crate::{
    app::AppState,
    bail,
    domain::{Services, db::Pk, model::User},
    error::{AppError, ErrorKind},
};

pub fn set_session_cookie(jar: CookieJar, state: &AppState, session_id: &str) -> CookieJar {
    let name = state.cfg().auth.session.cookie_name.clone();
    let mut cookie = Cookie::new(name, session_id.to_owned());
    cookie.set_path("/");
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
            .map_err(|_| ErrorKind::Unauthorized)?;

        let config = state.cfg();
        let session_id = jar
            .get(&config.auth.session.cookie_name)
            .ok_or(ErrorKind::Unauthorized)?
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
