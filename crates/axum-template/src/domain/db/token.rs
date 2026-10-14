//! 刷新令牌表上的具名操作：按用户批量删、按 token 取行、以未过期为条件的原子消费。

use toasty::Db;

use crate::{
    domain::{
        db::ToastyResultExt,
        model::RefreshToken,
        vo::{RefreshTokenId, UserId},
    },
    error::{AppError, ErrorCode, Result},
};

pub async fn delete_refresh_tokens_by_user(db: &mut Db, user_id: UserId) -> Result<()> {
    RefreshToken::all()
        .filter(RefreshToken::fields().user_id().eq(user_id))
        .delete()
        .exec(db)
        .await?;
    Ok(())
}

/// 按 token 取行；不存在即视为无效令牌。
pub async fn take_refresh_token(db: &mut Db, token: &str) -> Result<RefreshToken> {
    RefreshToken::filter_by_token(token)
        .get(db)
        .await
        .optional()?
        .ok_or_else(|| AppError::business(ErrorCode::AuthRefreshTokenInvalid))
}

/// 以「未过期」为条件原子删除；不存在、已被并发消费或已过期均映射为 `AuthRefreshTokenInvalid`。
pub async fn consume_refresh_token(db: &mut Db, id: RefreshTokenId) -> Result<()> {
    // 条件失败会报错，`filter` 失败只是删 0 行，所以并发消费必须走条件而不是过滤器。
    RefreshToken::filter_by_id(id)
        .delete()
        .set_condition(
            RefreshToken::fields()
                .expires_at()
                .gt(jiff::Timestamp::now())
                .into(),
        )
        .exec(db)
        .await
        .map_err(|e| {
            if e.is_condition_failed() || e.is_record_not_found() {
                AppError::business(ErrorCode::AuthRefreshTokenInvalid)
            } else {
                e.into()
            }
        })?;
    Ok(())
}
