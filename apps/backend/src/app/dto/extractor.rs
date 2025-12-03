use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Path, Query, Request},
    http::request::Parts,
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::{
    app::helper::auth::AuthSession,
    domain::model::{Permission, UserPrincipal},
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
        value.validate().map_err(AppError::Validation)?;
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
        value.validate().map_err(AppError::Validation)?;
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
        value.validate().map_err(AppError::Validation)?;
        Ok(AppJson(value))
    }
}

/// 权限检查提取器 - 自动验证用户认证并提供权限检查方法
#[derive(Debug)]
pub struct RequirePermission(pub UserPrincipal);

impl RequirePermission {
    /// 检查用户是否有指定权限
    pub fn has_permission(&self, perm: Permission) -> bool {
        self.0.has_permission(perm)
    }

    /// 检查用户是否有指定角色
    pub fn has_role(&self, role_name: &str) -> bool {
        self.0.has_role(role_name)
    }

    /// 获取用户主体
    pub fn user(&self) -> &UserPrincipal {
        &self.0
    }
}

impl<S> FromRequestParts<S> for RequirePermission
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // 提取 AuthSession
        let auth_session = AuthSession::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::internal("Failed to extract auth session"))?;

        let user = auth_session
            .user
            .as_ref()
            .ok_or(ErrorKind::Unauthenticated)?;

        Ok(RequirePermission(user.clone()))
    }
}
