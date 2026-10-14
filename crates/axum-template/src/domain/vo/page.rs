//! 一页结果：条目、总数与产生它的分页参数。

use super::Pagination;

/// `total` 与 `pagination` 必须来自同一次「列出 + 计数」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    total: u64,
    pagination: Pagination,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, total: u64, pagination: Pagination) -> Self {
        Self {
            items,
            total,
            pagination,
        }
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn page(&self) -> u64 {
        self.pagination.page()
    }

    pub fn per_page(&self) -> u64 {
        self.pagination.per_page()
    }

    pub fn map<U>(self, map: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(map).collect(),
            total: self.total,
            pagination: self.pagination,
        }
    }
}
