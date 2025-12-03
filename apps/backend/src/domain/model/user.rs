use entity::user;
use sea_orm::ActiveValue::Set;
use time::OffsetDateTime;

/// User 领域模型行为扩展
pub trait UserExt {}

/// User ActiveModel 创建方法
pub trait UserActiveModelExt {
    /// 创建新用户的 ActiveModel
    fn new_user(username: String, password_hash: String) -> user::ActiveModel;

    /// 更新密码
    fn set_password(&mut self, password_hash: String);

    /// 更新用户名
    fn set_username(&mut self, username: String);
}

impl UserExt for user::Model {}

impl UserActiveModelExt for user::ActiveModel {
    fn new_user(username: String, password_hash: String) -> user::ActiveModel {
        let now = OffsetDateTime::now_utc();
        user::ActiveModel {
            username: Set(username),
            password: Set(password_hash),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
    }

    fn set_password(&mut self, password_hash: String) {
        self.password = Set(password_hash);
        self.updated_at = Set(OffsetDateTime::now_utc());
    }

    fn set_username(&mut self, username: String) {
        self.username = Set(username);
        self.updated_at = Set(OffsetDateTime::now_utc());
    }
}
