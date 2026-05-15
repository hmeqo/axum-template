use serde::Deserialize;
use validator::Validate;

use crate::domain::db::Pk;

#[derive(Debug, Deserialize, Validate)]
pub struct PkPath {
    #[validate(range(min = 1))]
    pub id: Pk,
}
