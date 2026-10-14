use toasty::Db;

use crate::{
    bail,
    domain::{
        db::{ToastyResultExt, role as role_db},
        model::Role,
        vo::{Pagination, Perm, RoleId, UserId},
    },
    error::{AppError, ErrorCode, Result},
};

#[derive(Debug, Clone)]
pub struct RoleService {
    db: Db,
}

impl RoleService {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn create(
        &self,
        name: String,
        description: Option<String>,
        perms: &[Perm],
    ) -> Result<Role> {
        let mut db = self.db.clone();
        if Self::exists_by_name_inner(&mut db, &name).await? {
            bail!(AppError::business(ErrorCode::RoleNameTaken));
        }

        let role = toasty::create!(Role {
            name,
            description,
            permissions: perms.to_vec()
        })
        .exec(&mut db)
        .await?;
        Ok(role)
    }

    pub async fn find_by_id(&self, id: RoleId) -> Result<Option<Role>> {
        let mut db = self.db.clone();
        Role::get_by_id(&mut db, &id).await.optional()
    }

    pub async fn get_by_id(&self, id: RoleId) -> Result<Role> {
        let mut db = self.db.clone();
        Self::get_by_id_inner(&mut db, id).await
    }

    async fn get_by_id_inner(db: &mut Db, id: RoleId) -> Result<Role> {
        Role::get_by_id(db, &id)
            .await
            .optional()?
            .ok_or_else(|| AppError::not_found("Role not found"))
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<Role>> {
        let mut db = self.db.clone();
        Role::filter_by_name(name).get(&mut db).await.optional()
    }

    async fn exists_by_name_inner(db: &mut Db, name: &str) -> Result<bool> {
        Ok(Role::filter_by_name(name)
            .get(db)
            .await
            .optional()?
            .is_some())
    }

    pub async fn list(&self, pagination: Pagination) -> Result<Vec<Role>> {
        let mut db = self.db.clone();
        role_db::list_roles(&mut db, pagination).await
    }

    pub async fn list_all(&self) -> Result<Vec<Role>> {
        let mut db = self.db.clone();
        role_db::list_all_roles(&mut db).await
    }

    pub async fn count(&self) -> Result<u64> {
        let mut db = self.db.clone();
        role_db::count_roles(&mut db).await
    }

    pub async fn update_name(&self, id: RoleId, new_name: String) -> Result<Role> {
        let mut db = self.db.clone();
        let mut role = Self::get_by_id_inner(&mut db, id).await?;

        if new_name != role.name && Self::exists_by_name_inner(&mut db, &new_name).await? {
            bail!(AppError::business(ErrorCode::RoleNameTaken));
        }

        toasty::update!(role { name: new_name })
            .exec(&mut db)
            .await?;
        Ok(role)
    }

    pub async fn update_description(
        &self,
        id: RoleId,
        description: Option<String>,
    ) -> Result<Role> {
        let mut db = self.db.clone();
        let mut role = Self::get_by_id_inner(&mut db, id).await?;
        toasty::update!(role { description }).exec(&mut db).await?;
        Ok(role)
    }

    pub async fn delete(&self, id: RoleId) -> Result<()> {
        let mut db = self.db.clone();
        Role::delete_by_id(&mut db, id).await?;
        Ok(())
    }

    pub async fn assign_to_user(&self, user_id: UserId, role_id: RoleId) -> Result<()> {
        let mut db = self.db.clone();
        if !role_db::assign_role_to_user(&mut db, user_id, role_id).await? {
            bail!(AppError::business(ErrorCode::RoleAlreadyAssigned));
        }
        Ok(())
    }

    pub async fn remove_from_user(&self, user_id: UserId, role_id: RoleId) -> Result<()> {
        let mut db = self.db.clone();
        role_db::remove_role_from_user(&mut db, user_id, role_id).await
    }

    pub async fn get_user_roles(&self, user_id: UserId) -> Result<Vec<Role>> {
        let mut db = self.db.clone();
        role_db::find_user_roles(&mut db, user_id).await
    }

    pub async fn get_user_permissions(&self, user_id: UserId) -> Result<Vec<Perm>> {
        let roles = self.get_user_roles(user_id).await?;
        let mut perms: Vec<Perm> = roles.iter().flat_map(|r| r.permissions()).collect();
        perms.sort();
        perms.dedup();
        Ok(perms)
    }
}
