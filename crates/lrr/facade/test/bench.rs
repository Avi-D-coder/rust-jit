// The ordinary forwarding function from pinned rustc library/test/src/bench.rs.
// This remains a source call through std::hint, with its normal generic value
// transfer and cleanup; no test-crate-specific runtime dispatch is involved.
#[inline(always)]
pub fn black_box<T>(dummy: T) -> T {
    std::hint::black_box(dummy)
}
