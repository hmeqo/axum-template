use toasty::Db;

use crate::{
    domain::{
        db::Pk,
        model::{Perm, Permission},
    },
    error::{ErrorKind, Result},
};

#[derive(Debug, Clone)]
pub struct PermissionService {
    db: Db,
}

impl PermissionService {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn create(&self, perm: Perm) -> Result<Permission> {
        let mut db = self.db.clone();
        if Permission::filter_by_code(perm.code())
            .get(&mut db)
            .await
            .is_ok()
        {
            return Err(ErrorKind::AlreadyExists.msg("Permission already exists"));
        }

        Ok(toasty::create!(Permission {
            code: perm.code().to_owned(),
            description: Some(perm.description().to_owned()),
        })
        .exec(&mut db)
        .await?)
    }

    pub async fn find_by_id(&self, id: Pk) -> Result<Option<Permission>> {
        let mut db = self.db.clone();
        Ok(Permission::get_by_id(&mut db, &id).await.ok())
    }

    pub async fn get_by_id(&self, id: Pk) -> Result<Permission> {
        let mut db = self.db.clone();
        Ok(Permission::get_by_id(&mut db, &id).await?)
    }

    pub async fn find(&self, perm: Perm) -> Result<Option<Permission>> {
        let mut db = self.db.clone();
        Ok(Permission::filter_by_code(perm.code()).get(&mut db).await.ok())
    }

    pub async fn list(&self, page: u64, per_page: u64) -> Result<Vec<Permission>> {
        let mut db = self.db.clone();
        let offset = page.saturating_sub(1) * per_page;
        Ok(Permission::all()
            .order_by(Permission::fields().id().asc())
            .limit(per_page as usize)
            .offset(offset as usize)
            .exec(&mut db)
            .await?)
    }

    pub async fn list_all(&self) -> Result<Vec<Permission>> {
        let mut db = self.db.clone();
        Ok(Permission::all().exec(&mut db).await?)
    }

    pub async fn count(&self) -> Result<u64> {
        let mut db = self.db.clone();
        let perms = Permission::all().exec(&mut db).await?;
        Ok(perms.len() as u64)
    }

    pub async fn update_description(
        &self,
        id: Pk,
        description: Option<String>,
    ) -> Result<Permission> {
        let mut db = self.db.clone();
        let mut perm = Permission::get_by_id(&mut db, &id).await?;
        perm.update().description(description).exec(&mut db).await?;
        Ok(Permission::get_by_id(&mut db, &id).await?)
    }

    pub async fn delete_by_id(&self, id: Pk) -> Result<()> {
        let mut db = self.db.clone();
        if Permission::get_by_id(&mut db, &id).await.is_ok() {
            Permission::filter_by_id(id).delete().exec(&mut db).await?;
        }
        Ok(())
    }
}
