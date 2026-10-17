// `std::time`.
pub use core::time::Duration;

pub struct Instant {
    t: u64,
}
pub struct SystemTime {
    t: u64,
}

impl Instant {
    pub fn now() -> Instant {
        core::intrinsics::lrr_unsupported("std::time::Instant::now")
    }
    pub fn elapsed(&self) -> Duration {
        core::intrinsics::lrr_unsupported("std::time::Instant::elapsed")
    }
}
