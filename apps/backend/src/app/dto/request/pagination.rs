use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(default)]
pub struct PaginationQuery {
    pub page: u64,
    pub per_page: u64,
}

impl Default for PaginationQuery {
    fn default() -> Self {
        Self {
            page: 0,
            per_page: 10,
        }
    }
}
