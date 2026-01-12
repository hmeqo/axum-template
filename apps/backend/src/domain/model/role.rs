use chrono::Utc;
use entity::role;
use sea_orm::ActiveValue::Set;

/// Role 领域模型行为扩展
pub trait RoleExt {
    /// 判断是否为管理员角色
    fn is_admin(&self) -> bool;
}

/// Role ActiveModel 创建方法
pub trait RoleActiveModelExt {
    /// 创建新角色的 ActiveModel
    fn new_role(name: String, description: Option<String>) -> role::ActiveModel;

    /// 更新角色名
    fn set_name(&mut self, name: String);

    /// 更新角色描述
    fn set_description(&mut self, description: Option<String>);
}

impl RoleActiveModelExt for role::ActiveModel {
    fn new_role(name: String, description: Option<String>) -> role::ActiveModel {
        let now = Utc::now().into();
        role::ActiveModel {
            name: Set(name),
            description: Set(description),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
    }

    fn set_name(&mut self, name: String) {
        self.name = Set(name);
        self.updated_at = Set(Utc::now().into());
    }

    fn set_description(&mut self, description: Option<String>) {
        self.description = Set(description);
        self.updated_at = Set(Utc::now().into());
    }
}
