//! 分页参数：offset/limit 语义的唯一出处。

use crate::error::{AppError, Result};

pub const MAX_PER_PAGE: u64 = 100;

/// 结果窗口上限：一页的起始行不得落在它之后；越过的请求是 `400`。
pub const MAX_OFFSET: u64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pagination {
    page: u64,
    per_page: u64,
    offset: usize,
}

impl Pagination {
    pub fn new(page: u64, per_page: u64) -> Result<Self> {
        if page == 0 {
            return Err(AppError::malformed_request("page must be >= 1"));
        }
        if per_page == 0 || per_page > MAX_PER_PAGE {
            return Err(AppError::malformed_request(format!(
                "per_page must be between 1 and {MAX_PER_PAGE}"
            )));
        }

        let offset = page
            .checked_sub(1)
            .and_then(|pages| pages.checked_mul(per_page))
            .ok_or_else(|| AppError::malformed_request("page is too large"))?;
        if offset > MAX_OFFSET {
            return Err(AppError::malformed_request(format!(
                "page starts after the {MAX_OFFSET}-row result window"
            )));
        }
        let offset = usize::try_from(offset)
            .map_err(|_| AppError::malformed_request("page is too large"))?;

        Ok(Self {
            page,
            per_page,
            offset,
        })
    }

    pub fn page(&self) -> u64 {
        self.page
    }

    pub fn per_page(&self) -> u64 {
        self.per_page
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn limit(&self) -> usize {
        self.per_page as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_is_derived_from_page_and_per_page() {
        assert_eq!(Pagination::new(3, 20).unwrap().offset(), 40);
        assert_eq!(Pagination::new(1, MAX_PER_PAGE).unwrap().offset(), 0);
    }

    #[test]
    fn rejects_a_page_whose_offset_would_overflow() {
        assert!(Pagination::new(u64::MAX, MAX_PER_PAGE).is_err());
    }

    #[test]
    fn accepts_the_last_page_inside_the_result_window() {
        let last = Pagination::new(MAX_OFFSET / 10 + 1, 10).unwrap();
        assert_eq!(last.offset(), MAX_OFFSET as usize);
        assert!(Pagination::new(MAX_OFFSET / 10 + 2, 10).is_err());
    }

    #[test]
    fn rejects_page_zero_and_an_oversized_per_page() {
        assert!(Pagination::new(0, 10).is_err());
        assert!(Pagination::new(1, 0).is_err());
        assert!(Pagination::new(1, MAX_PER_PAGE + 1).is_err());
    }
}
