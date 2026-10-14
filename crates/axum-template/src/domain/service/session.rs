use std::sync::Arc;

use uuid::Uuid;

use crate::{
    domain::{
        db::{ToastyResultExt, session as session_db},
        model::Session,
        vo::{ClientInfo, SessionId, UserId},
    },
    error::{AppError, Result},
    infra::realtime::Sockets,
};

const MAX_LISTED_SESSIONS: usize = 50;

#[derive(Debug, Clone)]
pub struct SessionService {
    db: toasty::Db,
    ttl_hours: u64,
    sockets: Arc<Sockets>,
}

impl SessionService {
    pub fn new(db: toasty::Db, ttl_hours: u64, sockets: Arc<Sockets>) -> Self {
        Self {
            db,
            ttl_hours,
            sockets,
        }
    }

    pub fn ttl_secs(&self) -> i64 {
        (self.ttl_hours as i64).saturating_mul(3600)
    }

    /// 返回令牌与写入行的 `expires_at`，调用方据此设置 cookie 的 `Max-Age`。
    pub async fn create(
        &self,
        user_id: UserId,
        client: &ClientInfo,
    ) -> Result<(String, jiff::Timestamp)> {
        let mut db = self.db.clone();
        let token = Uuid::new_v4().to_string();
        let now = jiff::Timestamp::now();
        let expires_at = Session::expiry(now, self.ttl_secs());
        toasty::create!(Session {
            user_id,
            token: token.clone(),
            expires_at,
            ip: client.ip.map(|ip| ip.to_string()),
            user_agent: client.user_agent.clone(),
        })
        .exec(&mut db)
        .await?;
        Ok((token, expires_at))
    }

    pub async fn find_by_token(&self, token: &str) -> Result<Option<Session>> {
        let mut db = self.db.clone();
        Session::filter_by_token(token)
            .get(&mut db)
            .await
            .optional()
    }

    /// 剩余寿命不足半个 TTL 时续期，返回写入后的 `expires_at`；否则 `None`（不写库）。
    pub async fn renew_if_due(&self, session: &Session) -> Result<Option<jiff::Timestamp>> {
        if !session.should_extend(jiff::Timestamp::now(), self.ttl_secs()) {
            return Ok(None);
        }

        let mut db = self.db.clone();
        let renewed =
            session_db::renew_session(&mut db, session, jiff::Timestamp::now(), self.ttl_secs())
                .await?;
        Ok(Some(renewed.expires_at))
    }

    pub async fn delete_by_token(&self, token: &str) -> Result<()> {
        let mut db = self.db.clone();
        if let Some(id) = session_db::delete_session_by_token(&mut db, token).await? {
            self.sockets.revoke_session(id);
        }
        Ok(())
    }

    pub async fn delete_by_user_id(&self, user_id: UserId) -> Result<()> {
        let mut db = self.db.clone();
        session_db::delete_sessions_by_user(&mut db, user_id).await?;

        let revoked = self.sockets.revoke_user(user_id);
        tracing::debug!(revoked, "all sessions deleted; live connections revoked");
        Ok(())
    }

    /// 删掉该用户除 `keep` 之外的全部会话，并断开它们的活连接；`keep` 不存在不算错。
    pub async fn delete_others(&self, user_id: UserId, keep: SessionId) -> Result<()> {
        let mut db = self.db.clone();
        session_db::delete_other_sessions(&mut db, user_id, keep).await?;

        let revoked = self.sockets.revoke_user_except(user_id, keep);
        tracing::debug!(
            revoked,
            "other sessions deleted; their live connections revoked"
        );
        Ok(())
    }

    pub async fn list_by_user_id(&self, user_id: UserId) -> Result<Vec<Session>> {
        let mut db = self.db.clone();
        session_db::list_sessions_by_user(&mut db, user_id, MAX_LISTED_SESSIONS).await
    }

    /// 只撤销属于该用户的会话;别人的 id 一律当作不存在。
    pub async fn revoke(&self, id: SessionId, user_id: UserId) -> Result<()> {
        let mut db = self.db.clone();
        if !session_db::delete_owned_session(&mut db, id, user_id).await? {
            return Err(AppError::not_found("Session not found"));
        }

        let revoked = self.sockets.revoke_session(id);
        tracing::debug!(revoked, "session revoked; live connections revoked");
        Ok(())
    }

    /// 吊销一条会话；这行不存在也算成功。
    pub async fn revoke_own(&self, id: SessionId) -> Result<()> {
        let mut db = self.db.clone();
        Session::delete_by_id(&mut db, id).await?;

        let revoked = self.sockets.revoke_session(id);
        tracing::debug!(revoked, "own session revoked; live connections revoked");
        Ok(())
    }
}
