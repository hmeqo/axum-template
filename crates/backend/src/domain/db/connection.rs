use toasty::Db;

use crate::{domain::model, error::Result};

pub async fn init_db(url: &str) -> Result<Db> {
    let db = Db::builder()
        .models(toasty::models!(
            model::User,
            model::Role,
            model::Permission,
            model::UserRole,
            model::RolePermission,
            model::RefreshToken,
        ))
        .connect(url)
        .await?;
    Ok(db)
}
