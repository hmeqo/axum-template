use toasty::Db;

use crate::{
    domain::{
        db::Pk,
        model::{Permission, Role, RolePermission, UserRole},
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

    pub async fn create(&self, name: String, description: Option<String>) -> Result<Role> {
        let mut db = self.db.clone();
        if Self::exists_by_name_inner(&mut db, &name).await? {
            return Err(ErrorKind::AlreadyExists.msg("Role already exists"));
        }

        Ok(toasty::create!(Role { name, description })
            .exec(&mut db)
            .await?)
    }

    pub async fn find_by_id(&self, id: Pk) -> Result<Option<Role>> {
        let mut db = self.db.clone();
        Ok(Role::get_by_id(&mut db, &id).await.ok())
    }

    pub async fn get_by_id(&self, id: Pk) -> Result<Role> {
        let mut db = self.db.clone();
        Ok(Role::get_by_id(&mut db, &id).await?)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<Role>> {
        let mut db = self.db.clone();
        Ok(Role::filter_by_name(name).get(&mut db).await.ok())
    }

    async fn exists_by_name_inner(db: &mut Db, name: &str) -> Result<bool> {
        Ok(Role::filter_by_name(name).get(db).await.is_ok())
    }

    pub async fn list(&self, page: u64, per_page: u64) -> Result<Vec<Role>> {
        let mut db = self.db.clone();
        let offset = page.saturating_sub(1) * per_page;
        Ok(Role::all()
            .order_by(Role::fields().id().asc())
            .limit(per_page as usize)
            .offset(offset as usize)
            .exec(&mut db)
            .await?)
    }

    pub async fn list_all(&self) -> Result<Vec<Role>> {
        let mut db = self.db.clone();
        Ok(Role::all().exec(&mut db).await?)
    }

    pub async fn count(&self) -> Result<u64> {
        let mut db = self.db.clone();
        let roles = Role::all().exec(&mut db).await?;
        Ok(roles.len() as u64)
    }

    pub async fn update_name(&self, id: Pk, new_name: String) -> Result<Role> {
        let mut db = self.db.clone();
        let mut role = Role::get_by_id(&mut db, &id).await?;

        if new_name != role.name && Self::exists_by_name_inner(&mut db, &new_name).await? {
            return Err(ErrorKind::AlreadyExists.msg("Role name already exists"));
        }

        role.update().name(new_name).exec(&mut db).await?;
        Ok(Role::get_by_id(&mut db, &id).await?)
    }

    pub async fn update_description(&self, id: Pk, description: Option<String>) -> Result<Role> {
        let mut db = self.db.clone();
        let mut role = Role::get_by_id(&mut db, &id).await?;
        role.update().description(description).exec(&mut db).await?;
        Ok(Role::get_by_id(&mut db, &id).await?)
    }

    pub async fn delete(&self, id: Pk) -> Result<()> {
        let mut db = self.db.clone();
        if Role::get_by_id(&mut db, &id).await.is_ok() {
            Role::filter_by_id(id).delete().exec(&mut db).await?;
        }
        Ok(())
    }

    pub async fn add_permission(&self, role_id: Pk, perm_id: Pk) -> Result<()> {
        let mut db = self.db.clone();
        let _ = Role::get_by_id(&mut db, &role_id).await?;
        let _ = Permission::get_by_id(&mut db, &perm_id).await?;

        let existing = RolePermission::all()
            .filter(RolePermission::fields().role_id().eq(role_id))
            .filter(RolePermission::fields().permission_id().eq(perm_id))
            .exec(&mut db)
            .await?;

        if !existing.is_empty() {
            return Err(ErrorKind::AlreadyExists.msg("Permission already assigned to role"));
        }

        toasty::create!(RolePermission {
            role_id,
            permission_id: perm_id
        })
        .exec(&mut db)
        .await?;
        Ok(())
    }

    pub async fn remove_permission(&self, role_id: Pk, perm_id: Pk) -> Result<()> {
        let mut db = self.db.clone();
        let rps = RolePermission::all()
            .filter(RolePermission::fields().role_id().eq(role_id))
            .filter(RolePermission::fields().permission_id().eq(perm_id))
            .exec(&mut db)
            .await?;

        for rp in &rps {
            RolePermission::filter_by_id(rp.id)
                .delete()
                .exec(&mut db)
                .await?;
        }
        Ok(())
    }

    pub async fn get_permissions(&self, role_id: Pk) -> Result<Vec<Permission>> {
        let mut db = self.db.clone();
        let rps = RolePermission::all()
            .filter(RolePermission::fields().role_id().eq(role_id))
            .exec(&mut db)
            .await?;

        let mut perms = Vec::new();
        for rp in &rps {
            if let Ok(p) = Permission::get_by_id(&mut db, &rp.permission_id).await {
                perms.push(p);
            }
        }
        Ok(perms)
    }

    pub async fn assign_to_user(&self, user_id: Pk, role_id: Pk) -> Result<()> {
        let mut db = self.db.clone();
        let existing = UserRole::all()
            .filter(UserRole::fields().user_id().eq(user_id))
            .filter(UserRole::fields().role_id().eq(role_id))
            .exec(&mut db)
            .await?;

        if !existing.is_empty() {
            return Err(ErrorKind::AlreadyExists.msg("Role already assigned to user"));
        }

        toasty::create!(UserRole { user_id, role_id })
            .exec(&mut db)
            .await?;
        Ok(())
    }

    pub async fn remove_from_user(&self, user_id: Pk, role_id: Pk) -> Result<()> {
        let mut db = self.db.clone();
        let urs = UserRole::all()
            .filter(UserRole::fields().user_id().eq(user_id))
            .filter(UserRole::fields().role_id().eq(role_id))
            .exec(&mut db)
            .await?;

        for ur in &urs {
            UserRole::filter_by_id(ur.id).delete().exec(&mut db).await?;
        }
        Ok(())
    }

    pub async fn get_user_roles(&self, user_id: Pk) -> Result<Vec<Role>> {
        let mut db = self.db.clone();
        let urs = UserRole::all()
            .filter(UserRole::fields().user_id().eq(user_id))
            .exec(&mut db)
            .await?;

        let mut roles = Vec::new();
        for ur in &urs {
            if let Ok(role) = Role::get_by_id(&mut db, &ur.role_id).await {
                roles.push(role);
            }
        }
        Ok(roles)
    }

    pub async fn get_user_permissions(&self, user_id: Pk) -> Result<Vec<Permission>> {
        let roles = self.get_user_roles(user_id).await?;
        let mut all_permissions = Vec::new();

        for role in &roles {
            let perms = self.get_permissions(role.id).await?;
            all_permissions.extend(perms);
        }

        all_permissions.sort_by_key(|p| p.id);
        all_permissions.dedup_by_key(|p| p.id);
        Ok(all_permissions)
    }
}
