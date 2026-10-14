//! 会话表上的具名操作：批量删除、按 token 的两段式删除、把 `expires_at` 推到 `now + ttl`。

use toasty::Db;

use crate::{
    domain::{
        db::ToastyResultExt,
        model::Session,
        vo::{SessionId, UserId},
    },
    error::Result,
};

/// 该用户的会话，最近创建的在前，最多 `limit` 条。
pub async fn list_sessions_by_user(
    db: &mut Db,
    user_id: UserId,
    limit: usize,
) -> Result<Vec<Session>> {
    let sessions = Session::all()
        .filter(Session::fields().user_id().eq(user_id))
        .order_by(Session::fields().created_at().desc())
        .limit(limit)
        .exec(db)
        .await?;
    Ok(sessions)
}

pub async fn delete_sessions_by_user(db: &mut Db, user_id: UserId) -> Result<()> {
    Session::all()
        .filter(Session::fields().user_id().eq(user_id))
        .delete()
        .exec(db)
        .await?;
    Ok(())
}

pub async fn delete_other_sessions(db: &mut Db, user_id: UserId, keep: SessionId) -> Result<()> {
    Session::all()
        .filter(
            Session::fields()
                .user_id()
                .eq(user_id)
                .and(Session::fields().id().ne(keep)),
        )
        .delete()
        .exec(db)
        .await?;
    Ok(())
}

/// 按 token 删一条会话并返回它的 id；这行不存在是 `None`（调用方据此决定要不要断开连接）。
pub async fn delete_session_by_token(db: &mut Db, token: &str) -> Result<Option<SessionId>> {
    let session = Session::filter_by_token(token).get(db).await.optional()?;
    Session::filter_by_token(token).delete().exec(db).await?;
    Ok(session.map(|session| session.id))
}

/// 删一条属于 `user_id` 的会话；不存在或不属于他都是 `false`。
pub async fn delete_owned_session(db: &mut Db, id: SessionId, user_id: UserId) -> Result<bool> {
    // 两段式是因为 toasty 的 `Delete<T>` 固定 `T = ()`、`exec()` 不返回影响行数。
    let Some(session) = Session::get_by_id(db, &id).await.optional()? else {
        return Ok(false);
    };

    if session.user_id != user_id {
        return Ok(false);
    }

    Session::filter_by_token(&session.token)
        .delete()
        .exec(db)
        .await?;
    Ok(true)
}

/// 把 `expires_at` 推到 `now + ttl`；该不该续由调用方按 [`Session::should_extend`] 判断。
pub async fn renew_session(
    db: &mut Db,
    session: &Session,
    now: jiff::Timestamp,
    ttl_secs: i64,
) -> Result<Session> {
    let mut session = session.clone();
    let extended = Session::expiry(now, ttl_secs);
    toasty::update!(session {
        expires_at: extended
    })
    .exec(db)
    .await?;
    Ok(session)
}
