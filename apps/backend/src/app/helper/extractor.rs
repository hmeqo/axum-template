use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Path, Query, Request},
    http::request::Parts,
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::{
    app::helper::auth::AuthSession,
    domain::model::UserPrincipal,
    error::{AppError, ErrorKind},
};

/// A validated Path extractor that returns AppError on failure
#[derive(Debug)]
pub struct AppPath<T>(pub T);

impl<S, T> FromRequestParts<S> for AppPath<T>
where
    T: DeserializeOwned + Validate + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state).await?;
        value.validate()?;
        Ok(AppPath(value))
    }
}

/// A validated Query extractor that returns AppError on failure
#[derive(Debug)]
pub struct AppQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for AppQuery<T>
where
    T: DeserializeOwned + Validate + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state).await?;
        value.validate()?;
        Ok(AppQuery(value))
    }
}

/// A validated Json extractor that returns AppError on failure
#[derive(Debug)]
pub struct AppJson<T>(pub T);

impl<S, T> FromRequest<S> for AppJson<T>
where
    T: DeserializeOwned + Validate + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state).await?;
        value.validate()?;
        Ok(AppJson(value))
    }
}

/// 提取已认证的用户
#[derive(Debug)]
pub struct AuthedUser(pub UserPrincipal);

impl AuthedUser {
    /// 获取用户主体
    pub fn user(&self) -> &UserPrincipal {
        &self.0
    }
}

impl<S> FromRequestParts<S> for AuthedUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_session = AuthSession::from_request_parts(parts, state)
            .await
            .map_err(|_| ErrorKind::Internal.with_message("Failed to extract auth session"))?;

        let user = auth_session.user.as_ref().ok_or(ErrorKind::Unauthorized)?;

        Ok(AuthedUser(user.clone()))
    }
}
