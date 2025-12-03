use sea_orm::{DatabaseConnection, QueryFilter, QuerySelect, Set, entity::prelude::*};
use std::sync::Arc;
use time::OffsetDateTime;

use crate::error::Result;
use entity::{prelude::*, role_permission};

#[derive(Clone)]
pub struct RolePermissionRepository {
    pub db: Arc<DatabaseConnection>,
}

impl RolePermissionRepository {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// 检查角色是否拥有指定权限
    pub async fn exists(&self, role_id: i32, permission_id: i32) -> Result<bool> {
        Ok(RolePermission::find()
            .filter(role_permission::Column::RoleId.eq(role_id))
            .filter(role_permission::Column::PermissionId.eq(permission_id))
            .one(self.db())
            .await?
            .is_some())
    }

    /// 为角色添加权限
    pub async fn assign(&self, role_id: i32, permission_id: i32) -> Result<()> {
        let rp = role_permission::ActiveModel {
            role_id: Set(role_id),
            permission_id: Set(permission_id),
            created_at: Set(OffsetDateTime::now_utc()),
            ..Default::default()
        };
        rp.insert(self.db()).await?;
        Ok(())
    }

    /// 移除角色的权限
    pub async fn remove(&self, role_id: i32, permission_id: i32) -> Result<()> {
        RolePermission::delete_many()
            .filter(role_permission::Column::RoleId.eq(role_id))
            .filter(role_permission::Column::PermissionId.eq(permission_id))
            .exec(self.db())
            .await?;
        Ok(())
    }

    /// 获取角色的所有权限 ID
    pub async fn permission_ids_for_role(&self, role_id: i32) -> Result<Vec<i32>> {
        RolePermission::find()
            .filter(role_permission::Column::RoleId.eq(role_id))
            .select_only()
            .column(role_permission::Column::PermissionId)
            .into_tuple()
            .all(self.db())
            .await
            .map_err(Into::into)
    }

    /// 获取拥有指定权限的所有角色 ID
    pub async fn role_ids_for_permission(&self, permission_id: i32) -> Result<Vec<i32>> {
        RolePermission::find()
            .filter(role_permission::Column::PermissionId.eq(permission_id))
            .select_only()
            .column(role_permission::Column::RoleId)
            .into_tuple()
            .all(self.db())
            .await
            .map_err(Into::into)
    }
}
