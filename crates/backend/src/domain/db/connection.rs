use toasty::Db;

use crate::error::Result;

pub async fn init_db(url: &str) -> Result<Db> {
    let db = Db::builder()
        .models(toasty::models!(
            crate::domain::model::User,
            crate::domain::model::Role,
            crate::domain::model::Permission,
            crate::domain::model::UserRole,
            crate::domain::model::RolePermission,
            crate::domain::model::RefreshToken,
        ))
        .connect(url)
        .await?;
    Ok(db)
}
