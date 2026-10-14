//! 权限提取器：身份只读请求扩展中的解析结果（缺席或无效 → 401），权限不足 → 403。

use std::marker::PhantomData;

use axum::{extract::FromRequestParts, http::request::Parts};

use crate::{
    app::{
        AppContext,
        identity::{CurrentUser, Resolved},
    },
    domain::vo::PermSpec,
    error::AppError,
};

pub struct Require<P>(PhantomData<P>);

impl<P: PermSpec> FromRequestParts<AppContext> for Require<P> {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppContext,
    ) -> Result<Self, Self::Rejection> {
        let resolved = Resolved::cached(&parts.extensions).unwrap_or(Resolved::Absent);
        let user = CurrentUser::of(&resolved)?;
        state
            .srv()
            .auth
            .require_permission(user.user_id, P::PERM)
            .await?;
        Ok(Self(PhantomData))
    }
}
