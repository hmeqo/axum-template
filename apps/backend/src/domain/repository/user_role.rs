use chrono::Utc;
use sea_orm::{DatabaseConnection, QueryFilter, QuerySelect, Set, entity::prelude::*};
use std::sync::Arc;

use crate::{domain::db::Pk, error::Result};
use entity::{prelude::*, user_role};

#[derive(Debug)]
pub struct UserRoleRepository {
    pub db: Arc<DatabaseConnection>,
}

impl UserRoleRepository {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// 检查用户是否拥有指定角色
    pub async fn exists(&self, user_id: Pk, role_id: Pk) -> Result<bool> {
        Ok(UserRole::find()
            .filter(user_role::Column::UserId.eq(user_id))
            .filter(user_role::Column::RoleId.eq(role_id))
            .one(self.db())
            .await?
            .is_some())
    }

    /// 为用户分配角色
    pub async fn assign(&self, user_id: Pk, role_id: Pk) -> Result<()> {
        let ur = user_role::ActiveModel {
            user_id: Set(user_id),
            role_id: Set(role_id),
            created_at: Set(Utc::now().into()),
            ..Default::default()
        };
        ur.insert(self.db()).await?;
        Ok(())
    }

    /// 移除用户的角色
    pub async fn remove(&self, user_id: Pk, role_id: Pk) -> Result<()> {
        UserRole::delete_many()
            .filter(user_role::Column::UserId.eq(user_id))
            .filter(user_role::Column::RoleId.eq(role_id))
            .exec(self.db())
            .await?;
        Ok(())
    }

    /// 获取用户的所有角色 ID
    pub async fn role_ids_for_user(&self, user_id: Pk) -> Result<Vec<Pk>> {
        UserRole::find()
            .filter(user_role::Column::UserId.eq(user_id))
            .select_only()
            .column(user_role::Column::RoleId)
            .into_tuple()
            .all(self.db())
            .await
            .map_err(Into::into)
    }

    /// 获取拥有指定角色的所有用户 ID
    pub async fn user_ids_for_role(&self, role_id: Pk) -> Result<Vec<Pk>> {
        UserRole::find()
            .filter(user_role::Column::RoleId.eq(role_id))
            .select_only()
            .column(user_role::Column::UserId)
            .into_tuple()
            .all(self.db())
            .await
            .map_err(Into::into)
    }
}
