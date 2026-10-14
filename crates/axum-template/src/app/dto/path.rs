use serde::Deserialize;
use validator::Validate;

use crate::domain::vo::{FileId, SessionId, UserId};

#[derive(Debug, Deserialize, Validate)]
pub struct UserIdPath {
    #[validate(range(min = 1))]
    pub id: UserId,
}

#[derive(Debug, Deserialize, Validate)]
pub struct FileIdPath {
    pub id: FileId,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SessionIdPath {
    #[validate(range(min = 1))]
    pub id: SessionId,
}
