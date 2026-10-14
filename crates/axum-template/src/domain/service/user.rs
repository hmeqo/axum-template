use toasty::{Db, stmt::Expr};

use crate::{
    bail,
    domain::{
        db::{ToastyResultExt, user as user_db},
        model::User,
        vo::{LIKE_ESCAPE, Page, Pagination, Password, PasswordHash, UserFilter, UserId, Username},
    },
    error::{AppError, ErrorCode, Result},
};

#[derive(Debug, Clone)]
pub struct UserService {
    db: Db,
}

impl UserService {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn create(&self, username: Username, password: Password) -> Result<User> {
        let mut db = self.db.clone();
        if Self::exists_by_username_inner(&mut db, username.as_str()).await? {
            bail!(AppError::business(ErrorCode::UserUsernameTaken));
        }

        let hashed = PasswordHash::hash(&password)?;
        let username = username.into_string();
        let user = toasty::create!(User {
            username,
            password: hashed
        })
        .exec(&mut db)
        .await?;
        Ok(user)
    }

    pub async fn find_by_id(&self, id: UserId) -> Result<Option<User>> {
        let mut db = self.db.clone();
        User::get_by_id(&mut db, &id).await.optional()
    }

    pub async fn get_by_id(&self, id: UserId) -> Result<User> {
        let mut db = self.db.clone();
        Self::get_by_id_inner(&mut db, id).await
    }

    async fn get_by_id_inner(db: &mut Db, id: UserId) -> Result<User> {
        User::get_by_id(db, &id)
            .await
            .optional()?
            .ok_or_else(|| AppError::not_found("User not found"))
    }

    pub async fn exists_by_username(&self, username: &str) -> Result<bool> {
        let mut db = self.db.clone();
        Self::exists_by_username_inner(&mut db, username).await
    }

    async fn exists_by_username_inner(db: &mut Db, username: &str) -> Result<bool> {
        Ok(User::filter_by_username(username)
            .get(db)
            .await
            .optional()?
            .is_some())
    }

    pub async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let mut db = self.db.clone();
        User::filter_by_username(username)
            .get(&mut db)
            .await
            .optional()
    }

    pub async fn list(&self, pagination: Pagination, filter: &UserFilter) -> Result<Vec<User>> {
        let mut db = self.db.clone();
        user_db::list_users(&mut db, pagination, Self::filter_expr(filter)).await
    }

    pub async fn count(&self, filter: &UserFilter) -> Result<u64> {
        let mut db = self.db.clone();
        user_db::count_users(&mut db, Self::filter_expr(filter)).await
    }

    /// `total` 用与这一页相同的过滤表达式。
    pub async fn page(&self, pagination: Pagination, filter: &UserFilter) -> Result<Page<User>> {
        let items = self.list(pagination, filter).await?;
        let total = self.count(filter).await?;
        Ok(Page::new(items, total, pagination))
    }

    // 子串搜索用不上 `users.username` 的唯一索引(`ILIKE '%word%'` 左锚定不了);
    // 真要大表走索引需要 pg_trgm + GIN,或改成精确匹配。
    // `ILIKE` 的大小写折叠按库的 collation(`C.UTF-8` 下只折叠 ASCII)。
    fn filter_expr(filter: &UserFilter) -> Option<Expr<bool>> {
        let patterns = filter.username_patterns();
        (!patterns.is_empty()).then(|| {
            Expr::and_all(patterns.into_iter().map(|pattern| {
                User::fields()
                    .username()
                    .ilike_with_escape(pattern, LIKE_ESCAPE)
            }))
        })
    }

    pub async fn update_username(&self, id: UserId, new_username: Username) -> Result<User> {
        let mut db = self.db.clone();
        let mut user = Self::get_by_id_inner(&mut db, id).await?;

        let new_username = new_username.into_string();
        if new_username != user.username
            && Self::exists_by_username_inner(&mut db, &new_username).await?
        {
            bail!(AppError::business(ErrorCode::UserUsernameTaken));
        }

        toasty::update!(user {
            username: new_username
        })
        .exec(&mut db)
        .await?;
        Ok(user)
    }

    /// 只改列；失效登录态由 `AuthService::change_password` 负责。
    pub(crate) async fn change_password(
        &self,
        id: UserId,
        old_password: &str,
        new_password: Password,
    ) -> Result<()> {
        let mut db = self.db.clone();
        let user = Self::get_by_id_inner(&mut db, id).await?;

        if !user.password.verify(old_password)? {
            bail!(AppError::business(ErrorCode::UserInvalidOldPassword));
        }

        Self::set_password(&mut db, user, PasswordHash::hash(&new_password)?).await
    }

    /// 只改列；失效登录态由 `AuthService::reset_password` 负责。
    pub(crate) async fn reset_password(&self, id: UserId, new_password: Password) -> Result<()> {
        let mut db = self.db.clone();
        let user = Self::get_by_id_inner(&mut db, id).await?;
        Self::set_password(&mut db, user, PasswordHash::hash(&new_password)?).await
    }

    async fn set_password(db: &mut Db, mut user: User, hash: PasswordHash) -> Result<()> {
        toasty::update!(user { password: hash }).exec(db).await?;
        Ok(())
    }

    /// 只删行；吊销长连接由 `AuthService::delete_user` 负责。
    pub async fn delete(&self, id: UserId) -> Result<()> {
        let mut db = self.db.clone();
        User::delete_by_id(&mut db, id).await?;
        Ok(())
    }
}
