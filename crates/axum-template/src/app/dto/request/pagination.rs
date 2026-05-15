use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(default)]
pub struct PaginationReq {
    #[validate(range(min = 1))]
    pub page: u64,
    pub per_page: u64,
}

impl Default for PaginationReq {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 10,
        }
    }
}
