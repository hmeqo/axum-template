use toasty::Db;

use crate::{
    bail,
    domain::{
        db::Pk,
        model::{Perm, User, UserRole},
    },
    error::{ErrorKind, Result},
    util::password,
};

pub struct AuthUser {
    pub user: User,
    pub permissions: Vec<Perm>,
}

impl AuthUser {
    pub fn new(user: User, permissions: Vec<Perm>) -> Self {
        Self { user, permissions }
    }
}

#[derive(Debug, Clone)]
pub struct AuthService {
    db: Db,
}

impl AuthService {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    fn db(&self) -> Db {
        self.db.clone()
    }

    async fn get_user_permissions(&self, user_id: Pk) -> Result<Vec<Perm>> {
        let mut db = self.db();
        let urs = UserRole::all()
            .filter(UserRole::fields().user_id().eq(user_id))
            .include(UserRole::fields().role())
            .exec(&mut db)
            .await?;

        let mut perms: Vec<Perm> = urs
            .iter()
            .flat_map(|ur| ur.role.get().parse_perms())
            .collect();
        perms.sort();
        perms.dedup();
        Ok(perms)
    }

    pub async fn authenticate(
        &self,
        username: &str,
        password_str: &str,
    ) -> Result<Option<AuthUser>> {
        let mut db = self.db();
        let Some(user) = User::filter_by_username(username).get(&mut db).await.ok() else {
            return Ok(None);
        };

        if !password::verify(password_str, &user.password)? {
            return Ok(None);
        }

        let permissions = self.get_user_permissions(user.id).await?;
        Ok(Some(AuthUser::new(user, permissions)))
    }

    pub async fn get_auth_user(&self, user_id: Pk) -> Result<AuthUser> {
        let mut db = self.db();
        let user = User::get_by_id(&mut db, &user_id).await?;
        let permissions = self.get_user_permissions(user_id).await?;
        Ok(AuthUser::new(user, permissions))
    }

    pub async fn check_permission(&self, user_id: Pk, perm: Perm) -> Result<bool> {
        let perms = self.get_user_permissions(user_id).await?;
        Ok(perms.iter().any(|p| p.matches(perm.code())))
    }

    pub async fn require_permission(&self, user_id: Pk, perm: Perm) -> Result<()> {
        if !self.check_permission(user_id, perm).await? {
            bail!(ErrorKind::PermissionDenied, "Insufficient permissions");
        }
        Ok(())
    }
}
