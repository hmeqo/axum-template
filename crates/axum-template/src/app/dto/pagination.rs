//! 分页查询串。

use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::domain::vo::{MAX_PER_PAGE, Pagination};

#[derive(Debug, Deserialize, Validate, ToSchema, IntoParams)]
#[serde(default)]
#[into_params(parameter_in = Query)]
pub struct PaginationReq {
    /// One-based page number; a page starting past the result window is rejected.
    #[param(minimum = 1)]
    #[validate(range(min = 1))]
    pub page: u64,
    // utoipa 的属性值只接受字面量,`#[param]` 这里必须再写一份数值;运行时的边界由
    // `#[validate]`(经 `MAX_PER_PAGE`)决定,这一份只影响文档。
    #[param(minimum = 1, maximum = 100)]
    #[validate(range(min = 1, max = MAX_PER_PAGE))]
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

impl TryFrom<PaginationReq> for Pagination {
    type Error = crate::error::AppError;

    fn try_from(req: PaginationReq) -> Result<Self, Self::Error> {
        Pagination::new(req.page, req.per_page)
    }
}
