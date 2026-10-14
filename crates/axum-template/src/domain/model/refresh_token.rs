use toasty::Model;

use super::User;
use crate::domain::vo::{RefreshTokenId, UserId};

#[derive(Debug, Clone, Model)]
pub struct RefreshToken {
    #[key]
    #[auto]
    pub id: RefreshTokenId,

    #[index]
    pub user_id: UserId,

    #[belongs_to(key = user_id, references = id)]
    pub user: toasty::Deferred<User>,

    #[unique]
    pub token: String,

    pub expires_at: jiff::Timestamp,

    #[auto]
    pub created_at: jiff::Timestamp,

    /// 建立这条刷新令牌时的客户端地址。
    pub ip: Option<String>,

    pub user_agent: Option<String>,
}

impl RefreshToken {
    pub const TTL_HOURS: i64 = 30 * 24;

    pub fn new_expiry(now: jiff::Timestamp) -> jiff::Timestamp {
        now + jiff::Span::new().hours(Self::TTL_HOURS)
    }
}
