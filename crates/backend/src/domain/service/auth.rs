use toasty::Db;

use crate::{domain::model::User, error::Result, util::password};

#[derive(Debug, Clone)]
pub struct AuthService {
    db: Db,
}

impl AuthService {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn authenticate(&self, username: &str, password_str: &str) -> Result<Option<User>> {
        let mut db = self.db.clone();
        let Some(user) = User::filter_by_username(username).get(&mut db).await.ok() else {
            return Ok(None);
        };

        if password::verify(password_str, &user.password)? {
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }
}
