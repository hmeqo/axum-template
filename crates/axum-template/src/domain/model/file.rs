use toasty::Model;

use super::User;
use crate::domain::vo::{FileId, FileStateCode, UserId};

#[derive(Debug, Clone, Model)]
pub struct File {
    #[key]
    pub id: FileId,

    #[index]
    pub user_id: UserId,

    #[belongs_to(key = user_id, references = id)]
    pub user: toasty::Deferred<User>,

    pub original_name: String,

    pub mime_type: String,

    pub file_size: i64,

    /// 显示尺寸（已按 EXIF 方向修正）；非栅格图或解不开时为 `None`。
    pub width: Option<u32>,

    pub height: Option<u32>,

    pub hash: String,

    pub state: FileStateCode,

    pub attached_at: Option<jiff::Timestamp>,

    #[auto]
    pub created_at: jiff::Timestamp,

    #[auto]
    pub updated_at: jiff::Timestamp,
}
