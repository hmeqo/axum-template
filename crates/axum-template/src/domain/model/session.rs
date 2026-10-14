use toasty::Model;

use super::User;
use crate::domain::vo::{SessionId, UserId};

#[derive(Debug, Clone, Model)]
pub struct Session {
    #[key]
    #[auto]
    pub id: SessionId,

    #[index]
    pub user_id: UserId,

    #[belongs_to(key = user_id, references = id)]
    pub user: toasty::Deferred<User>,

    #[unique]
    pub token: String,

    pub expires_at: jiff::Timestamp,

    #[auto]
    pub created_at: jiff::Timestamp,

    /// 建立这条会话时的客户端地址。
    pub ip: Option<String>,

    pub user_agent: Option<String>,
}

impl Session {
    pub fn is_expired(&self, now: jiff::Timestamp) -> bool {
        self.expires_at < now
    }

    pub fn should_extend(&self, now: jiff::Timestamp, ttl_secs: i64) -> bool {
        self.expires_at.as_second() - now.as_second() < ttl_secs / 2
    }

    pub fn expiry(now: jiff::Timestamp, ttl_secs: i64) -> jiff::Timestamp {
        now + jiff::Span::new().seconds(ttl_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600;
    const TTL: i64 = 24 * HOUR;

    fn session(now: jiff::Timestamp, ttl_secs: i64) -> Session {
        Session {
            id: 1.into(),
            user_id: 1.into(),
            user: Default::default(),
            token: "t".into(),
            expires_at: now + jiff::Span::new().seconds(ttl_secs),
            created_at: now,
            ip: None,
            user_agent: None,
        }
    }

    #[test]
    fn expiry_is_judged_against_now() {
        let now = jiff::Timestamp::now();

        assert!(!session(now, HOUR).is_expired(now));
        assert!(session(now, -1).is_expired(now));
    }

    #[test]
    fn extends_only_when_less_than_half_the_ttl_remains() {
        let now = jiff::Timestamp::now();

        assert!(!session(now, TTL).should_extend(now, TTL));
        assert!(session(now, TTL / 4).should_extend(now, TTL));
    }

    #[test]
    fn renewal_starts_a_full_ttl_from_now() {
        let now = jiff::Timestamp::now();

        assert_eq!(
            Session::expiry(now, TTL),
            now + jiff::Span::new().seconds(TTL)
        );
    }
}
