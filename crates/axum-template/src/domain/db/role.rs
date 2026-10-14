//! 角色与 `user_roles` 上的具名操作：分页、全量、计数、连接查询、赋值与移除。

use toasty::Db;

use crate::{
    domain::{
        model::{Role, UserRole},
        vo::{Pagination, RoleId, UserId},
    },
    error::Result,
};

/// 一页角色，按 id 升序。
pub async fn list_roles(db: &mut Db, pagination: Pagination) -> Result<Vec<Role>> {
    let roles = Role::all()
        .order_by(Role::fields().id().asc())
        .limit(pagination.limit())
        .offset(pagination.offset())
        .exec(db)
        .await?;
    Ok(roles)
}

pub async fn list_all_roles(db: &mut Db) -> Result<Vec<Role>> {
    let roles = Role::all().exec(db).await?;
    Ok(roles)
}

pub async fn count_roles(db: &mut Db) -> Result<u64> {
    Ok(Role::all().count().exec(db).await?)
}

/// 该用户的角色（`user_roles` 连接 `roles`），按 id 升序。
pub async fn find_user_roles(db: &mut Db, user_id: UserId) -> Result<Vec<Role>> {
    let urs = UserRole::all()
        .filter(UserRole::fields().user_id().eq(user_id))
        .include(UserRole::fields().role())
        .exec(db)
        .await?;

    // (user_id, role_id) 受复合唯一约束，不会出现重复角色。
    let mut roles: Vec<Role> = urs.iter().map(|ur| ur.role.get().clone()).collect();
    roles.sort_by_key(|r| r.id);
    Ok(roles)
}

/// 赋角色；已经赋过返回 `false`。
pub async fn assign_role_to_user(db: &mut Db, user_id: UserId, role_id: RoleId) -> Result<bool> {
    // 复合唯一约束是并发下的真正保证；此处的存在性判断只用于给出友好错误。
    let existing = UserRole::all()
        .filter(UserRole::fields().user_id().eq(user_id))
        .filter(UserRole::fields().role_id().eq(role_id))
        .exec(db)
        .await?;

    if !existing.is_empty() {
        return Ok(false);
    }

    toasty::create!(UserRole { user_id, role_id })
        .exec(db)
        .await?;
    Ok(true)
}

/// 移除角色；本来就没赋过也算成功。
pub async fn remove_role_from_user(db: &mut Db, user_id: UserId, role_id: RoleId) -> Result<()> {
    UserRole::all()
        .filter(UserRole::fields().user_id().eq(user_id))
        .filter(UserRole::fields().role_id().eq(role_id))
        .delete()
        .exec(db)
        .await?;
    Ok(())
}
