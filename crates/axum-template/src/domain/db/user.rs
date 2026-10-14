//! 用户表上的具名操作：带过滤表达式的分页与计数。

use toasty::{Db, stmt::Expr};

use crate::{
    domain::{model::User, vo::Pagination},
    error::Result,
};

/// 一页用户，按 id 升序；`filter` 为 `None` 表示不过滤。
pub async fn list_users(
    db: &mut Db,
    pagination: Pagination,
    filter: Option<Expr<bool>>,
) -> Result<Vec<User>> {
    let mut query = User::all();
    if let Some(filter) = filter {
        query = query.filter(filter);
    }

    let users = query
        .order_by(User::fields().id().asc())
        .limit(pagination.limit())
        .offset(pagination.offset())
        .exec(db)
        .await?;
    Ok(users)
}

/// 与 [`list_users`] 同一过滤条件下的总数。
pub async fn count_users(db: &mut Db, filter: Option<Expr<bool>>) -> Result<u64> {
    let mut query = User::all();
    if let Some(filter) = filter {
        query = query.filter(filter);
    }

    Ok(query.count().exec(db).await?)
}
