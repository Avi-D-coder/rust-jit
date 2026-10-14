use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct CacheBudgets {
    pub query_bytes: usize,
    pub parse_bytes: usize,
    pub source_input_bytes: usize,
}
impl Default for CacheBudgets {
    fn default() -> Self {
        Self {
            query_bytes: 128 * 1024 * 1024,
            parse_bytes: 128 * 1024 * 1024,
            source_input_bytes: 128 * 1024 * 1024,
        }
    }
}
pub const INSTALLED_LRIR_BUDGET: usize = 64 * 1024 * 1024;

/// Capacity-based estimates, not measured allocator/RSS totals.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CacheUsage {
    pub memo_entries: usize,
    pub memo_bytes: usize,
    pub parse_bytes: usize,
    pub pinned_memo_bytes: usize,
    pub pinned_parse_bytes: usize,
    pub source_input_bytes: usize,
}
#[derive(Debug, Serialize)]
pub struct CacheEviction {
    pub before: CacheUsage,
    pub after: CacheUsage,
    pub victims: Vec<crate::db::key::QueryKey>,
}
