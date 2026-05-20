use toasty::Db;

use crate::{
    bail,
    domain::{db::Pk, model::User},
    error::{ErrorKind, Result},
    util::password,
};

#[derive(Debug, Clone)]
pub struct UserService {
    db: Db,
}

impl UserService {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    fn db(&self) -> Db {
        self.db.clone()
    }

    pub async fn create(&self, username: String, password: String) -> Result<User> {
        let mut db = self.db();
        if Self::exists_by_username_inner(&mut db, &username).await? {
            bail!(ErrorKind::AlreadyExists, "Username already exists");
        }

        let hashed = password::hash(&password)?;
        Ok(toasty::create!(User {
            username,
            password: hashed
        })
        .exec(&mut db)
        .await?)
    }

    pub async fn find_by_id(&self, id: Pk) -> Result<Option<User>> {
        let mut db = self.db();
        Ok(User::get_by_id(&mut db, &id).await.ok())
    }

    pub async fn get_by_id(&self, id: Pk) -> Result<User> {
        let mut db = self.db();
        Ok(User::get_by_id(&mut db, &id).await?)
    }

    pub async fn exists_by_username(&self, username: &str) -> Result<bool> {
        let mut db = self.db();
        Self::exists_by_username_inner(&mut db, username).await
    }

    async fn exists_by_username_inner(db: &mut Db, username: &str) -> Result<bool> {
        Ok(User::filter_by_username(username).get(db).await.is_ok())
    }

    pub async fn list(&self, page: u64, per_page: u64) -> Result<Vec<User>> {
        let mut db = self.db();
        let offset = page.saturating_sub(1) * per_page;
        Ok(User::all()
            .order_by(User::fields().id().asc())
            .limit(per_page as usize)
            .offset(offset as usize)
            .exec(&mut db)
            .await?)
    }

    pub async fn count(&self) -> Result<u64> {
        let mut db = self.db();
        Ok(User::all().count().exec(&mut db).await?)
    }

    pub async fn update_username(&self, id: Pk, new_username: String) -> Result<User> {
        let mut db = self.db();
        let mut user = User::get_by_id(&mut db, &id).await?;

        if new_username != user.username
            && Self::exists_by_username_inner(&mut db, &new_username).await?
        {
            bail!(ErrorKind::AlreadyExists, "Username already exists");
        }

        user.update().username(new_username).exec(&mut db).await?;
        Ok(User::get_by_id(&mut db, &id).await?)
    }

    pub async fn change_password(
        &self,
        id: Pk,
        old_password: &str,
        new_password: &str,
    ) -> Result<()> {
        let mut db = self.db();
        let mut user = User::get_by_id(&mut db, &id).await?;

        if !password::verify(old_password, &user.password)? {
            bail!(ErrorKind::InvalidCredentials, "Invalid old password");
        }

        let hashed = password::hash(new_password)?;
        user.update().password(hashed).exec(&mut db).await?;
        Ok(())
    }

    pub async fn reset_password(&self, id: Pk, new_password: &str) -> Result<()> {
        let mut db = self.db();
        let mut user = User::get_by_id(&mut db, &id).await?;
        let hashed = password::hash(new_password)?;
        user.update().password(hashed).exec(&mut db).await?;
        Ok(())
    }

    pub async fn delete(&self, id: Pk) -> Result<()> {
        let mut db = self.db();
        User::filter_by_id(id).delete().exec(&mut db).await?;
        Ok(())
    }
}
