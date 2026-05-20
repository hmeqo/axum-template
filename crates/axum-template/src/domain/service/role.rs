use toasty::Db;

use crate::{
    bail,
    domain::{
        db::Pk,
        model::{Perm, Role, UserRole},
    },
    error::{ErrorKind, Result},
};

#[derive(Debug, Clone)]
pub struct RoleService {
    db: Db,
}

impl RoleService {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    fn db(&self) -> Db {
        self.db.clone()
    }

    pub async fn create(
        &self,
        name: String,
        description: Option<String>,
        perms: &[Perm],
    ) -> Result<Role> {
        let mut db = self.db();
        if Self::exists_by_name_inner(&mut db, &name).await? {
            bail!(ErrorKind::AlreadyExists, "Role already exists");
        }

        let permissions = serde_json::to_string(perms).unwrap_or_default();
        Ok(toasty::create!(Role {
            name,
            description,
            permissions
        })
        .exec(&mut db)
        .await?)
    }

    pub async fn find_by_id(&self, id: Pk) -> Result<Option<Role>> {
        let mut db = self.db();
        Ok(Role::get_by_id(&mut db, &id).await.ok())
    }

    pub async fn get_by_id(&self, id: Pk) -> Result<Role> {
        let mut db = self.db();
        Ok(Role::get_by_id(&mut db, &id).await?)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<Role>> {
        let mut db = self.db();
        Ok(Role::filter_by_name(name).get(&mut db).await.ok())
    }

    async fn exists_by_name_inner(db: &mut Db, name: &str) -> Result<bool> {
        Ok(Role::filter_by_name(name).get(db).await.is_ok())
    }

    pub async fn list(&self, page: u64, per_page: u64) -> Result<Vec<Role>> {
        let mut db = self.db();
        let offset = page.saturating_sub(1) * per_page;
        Ok(Role::all()
            .order_by(Role::fields().id().asc())
            .limit(per_page as usize)
            .offset(offset as usize)
            .exec(&mut db)
            .await?)
    }

    pub async fn list_all(&self) -> Result<Vec<Role>> {
        let mut db = self.db();
        Ok(Role::all().exec(&mut db).await?)
    }

    pub async fn count(&self) -> Result<u64> {
        let mut db = self.db();
        Ok(Role::all().count().exec(&mut db).await?)
    }

    pub async fn update_name(&self, id: Pk, new_name: String) -> Result<Role> {
        let mut db = self.db();
        let mut role = Role::get_by_id(&mut db, &id).await?;

        if new_name != role.name && Self::exists_by_name_inner(&mut db, &new_name).await? {
            bail!(ErrorKind::AlreadyExists, "Role name already exists");
        }

        role.update().name(new_name).exec(&mut db).await?;
        Ok(Role::get_by_id(&mut db, &id).await?)
    }

    pub async fn update_description(&self, id: Pk, description: Option<String>) -> Result<Role> {
        let mut db = self.db();
        let mut role = Role::get_by_id(&mut db, &id).await?;
        role.update().description(description).exec(&mut db).await?;
        Ok(Role::get_by_id(&mut db, &id).await?)
    }

    pub async fn delete(&self, id: Pk) -> Result<()> {
        let mut db = self.db();
        Role::filter_by_id(id).delete().exec(&mut db).await?;
        Ok(())
    }

    pub async fn assign_to_user(&self, user_id: Pk, role_id: Pk) -> Result<()> {
        let mut db = self.db();
        let existing = UserRole::all()
            .filter(UserRole::fields().user_id().eq(user_id))
            .filter(UserRole::fields().role_id().eq(role_id))
            .exec(&mut db)
            .await?;

        if !existing.is_empty() {
            bail!(ErrorKind::AlreadyExists, "Role already assigned to user");
        }

        toasty::create!(UserRole { user_id, role_id })
            .exec(&mut db)
            .await?;
        Ok(())
    }

    pub async fn remove_from_user(&self, user_id: Pk, role_id: Pk) -> Result<()> {
        let mut db = self.db();
        UserRole::all()
            .filter(UserRole::fields().user_id().eq(user_id))
            .filter(UserRole::fields().role_id().eq(role_id))
            .delete()
            .exec(&mut db)
            .await?;
        Ok(())
    }

    pub async fn get_user_roles(&self, user_id: Pk) -> Result<Vec<Role>> {
        let mut db = self.db();
        let urs = UserRole::all()
            .filter(UserRole::fields().user_id().eq(user_id))
            .include(UserRole::fields().role())
            .exec(&mut db)
            .await?;

        let mut roles: Vec<Role> = urs.iter().map(|ur| ur.role.get().clone()).collect();
        roles.sort_by_key(|r| r.id);
        roles.dedup_by_key(|r| r.id);
        Ok(roles)
    }

    pub async fn get_user_permissions(&self, user_id: Pk) -> Result<Vec<Perm>> {
        let roles = self.get_user_roles(user_id).await?;
        let mut perms: Vec<Perm> = roles.iter().flat_map(|r| r.parse_perms()).collect();
        perms.sort();
        perms.dedup();
        Ok(perms)
    }
}
