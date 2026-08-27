use serde::{Deserialize, Serialize};

const DEFAULT_PAGE: u32 = 1;
const DEFAULT_PAGE_SIZE: u32 = 20;
const MAX_PAGE_SIZE: u32 = 1000;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

impl PaginationQuery {
    pub fn from_options(page: Option<u32>, page_size: Option<u32>) -> Self {
        Self {
            page: page.unwrap_or(DEFAULT_PAGE),
            page_size: page_size.unwrap_or(DEFAULT_PAGE_SIZE),
        }
    }

    pub fn normalized(self) -> PageRequest {
        let page = self.page.max(1);
        let page_size = self.page_size.clamp(1, MAX_PAGE_SIZE);
        PageRequest {
            page,
            page_size,
            offset: i64::from((page - 1).saturating_mul(page_size)),
        }
    }
}

impl Default for PaginationQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PageRequest {
    pub page: u32,
    pub page_size: u32,
    pub offset: i64,
}

impl PageRequest {
    pub fn meta(self, total: i64) -> PaginationMeta {
        let total = total.max(0) as u64;
        let page_size = u64::from(self.page_size);
        PaginationMeta {
            page: self.page,
            page_size: self.page_size,
            total,
            total_pages: total.div_ceil(page_size),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginationMeta {
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
    pub total_pages: u64,
}

const fn default_page() -> u32 {
    DEFAULT_PAGE
}

const fn default_page_size() -> u32 {
    DEFAULT_PAGE_SIZE
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ApiEnvelope<T> {
    pub code: &'static str,
    pub data: T,
}

#[cfg(test)]
mod tests {
    use super::{MAX_PAGE_SIZE, PaginationQuery};

    #[test]
    fn normalizes_invalid_pagination_without_overflow() {
        let page = PaginationQuery {
            page: 0,
            page_size: u32::MAX,
        }
        .normalized();

        assert_eq!(page.page, 1);
        assert_eq!(page.page_size, MAX_PAGE_SIZE);
        assert_eq!(page.offset, 0);
    }

    #[test]
    fn calculates_pagination_metadata() {
        let request = PaginationQuery {
            page: 3,
            page_size: 20,
        }
        .normalized();
        let meta = request.meta(41);

        assert_eq!(request.offset, 40);
        assert_eq!(meta.total_pages, 3);
        assert_eq!(meta.total, 41);
    }
}
