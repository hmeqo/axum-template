use toasty::Model;

#[derive(Debug, Clone, Model)]
pub struct Session {
    #[key]
    #[auto]
    pub id: i64,

    pub user_id: i64,

    #[unique]
    pub session_id: String,

    pub expires_at: jiff::Timestamp,

    #[auto]
    pub created_at: jiff::Timestamp,
}
