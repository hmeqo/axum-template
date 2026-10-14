use toasty::Db;

use crate::error::Result;

pub mod role;
pub mod session;
pub mod token;
pub mod user;

#[macro_export]
macro_rules! app_models {
    () => {
        toasty::models!(
            $crate::domain::model::User,
            $crate::domain::model::Role,
            $crate::domain::model::UserRole,
            $crate::domain::model::RefreshToken,
            $crate::domain::model::Session,
            $crate::domain::model::File,
        )
    };
}

/// 仅当记录不存在时把 toasty 错误视为 `None`；其它错误（连接失败等）照常上抛。
pub trait ToastyResultExt<T> {
    fn optional(self) -> Result<Option<T>>;
}

impl<T> ToastyResultExt<T> for std::result::Result<T, toasty::Error> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.is_record_not_found() => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

pub async fn init_db(url: &str) -> Result<Db> {
    let db = Db::builder()
        .models(crate::app_models!())
        .connect(url)
        .await?;
    Ok(db)
}
