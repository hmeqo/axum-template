use uuid::Uuid;

use crate::{
    domain::{db::Pk, model::Session},
    error::Result,
};

#[derive(Debug, Clone)]
pub struct SessionService {
    db: toasty::Db,
    ttl_hours: u64,
}

impl SessionService {
    pub fn new(db: toasty::Db, ttl_hours: u64) -> Self {
        Self { db, ttl_hours }
    }

    fn expires_at(&self) -> jiff::Timestamp {
        jiff::Timestamp::now() + jiff::Span::new().hours(self.ttl_hours as i64)
    }

    pub fn should_extend(&self, session: &Session) -> bool {
        let now = jiff::Timestamp::now();
        let elapsed_secs = now.as_second() as i64 - session.created_at.as_second() as i64;
        elapsed_secs >= (self.ttl_hours * 3600 / 2) as i64
    }

    pub async fn create(&self, user_id: Pk) -> Result<String> {
        let mut db = self.db.clone();
        let session_id = Uuid::new_v4().to_string();
        let expires_at = self.expires_at();
        toasty::create!(Session {
            user_id,
            session_id: session_id.clone(),
            expires_at,
        })
        .exec(&mut db)
        .await?;
        Ok(session_id)
    }

    pub async fn find(&self, session_id: &str) -> Result<Option<Session>> {
        let mut db = self.db.clone();
        Ok(Session::filter_by_session_id(session_id)
            .get(&mut db)
            .await
            .ok())
    }

    pub async fn extend(&self, session_id: &str) -> Result<()> {
        let mut db = self.db.clone();
        let mut session = Session::filter_by_session_id(session_id)
            .get(&mut db)
            .await?;
        session
            .update()
            .expires_at(self.expires_at())
            .exec(&mut db)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, session_id: &str) -> Result<()> {
        let mut db = self.db.clone();
        Session::filter_by_session_id(session_id)
            .delete()
            .exec(&mut db)
            .await?;
        Ok(())
    }

    pub async fn delete_by_user_id(&self, user_id: Pk) -> Result<()> {
        let mut db = self.db.clone();
        let sessions = Session::all()
            .filter(Session::fields().user_id().eq(user_id))
            .exec(&mut db)
            .await?;
        for s in &sessions {
            Session::filter_by_id(s.id).delete().exec(&mut db).await?;
        }
        Ok(())
    }
}
