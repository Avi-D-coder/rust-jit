// `core::sync`.
pub mod atomic {
    use crate::cell::UnsafeCell;

    pub enum Ordering {
        Relaxed,
        Release,
        Acquire,
        AcqRel,
        SeqCst,
    }

    pub struct AtomicBool {
        v: UnsafeCell<u8>,
    }
    pub struct AtomicUsize {
        v: UnsafeCell<usize>,
    }
    pub struct AtomicIsize {
        v: UnsafeCell<isize>,
    }
    pub struct AtomicU32 {
        v: UnsafeCell<u32>,
    }
    pub struct AtomicI32 {
        v: UnsafeCell<i32>,
    }
    pub struct AtomicU64 {
        v: UnsafeCell<u64>,
    }
    pub struct AtomicI64 {
        v: UnsafeCell<i64>,
    }

    impl AtomicBool {
        pub const fn new(v: bool) -> AtomicBool {
            core::intrinsics::lrr_unsupported("core::sync::atomic::AtomicBool::new")
        }
        pub fn load(&self, order: Ordering) -> bool {
            core::intrinsics::lrr_unsupported("core::sync::atomic::AtomicBool::load")
        }
        pub fn store(&self, val: bool, order: Ordering) {
            core::intrinsics::lrr_unsupported("core::sync::atomic::AtomicBool::store")
        }
    }
    impl AtomicUsize {
        pub const fn new(v: usize) -> AtomicUsize {
            AtomicUsize {
                v: UnsafeCell::new(v),
            }
        }
        pub fn load(&self, order: Ordering) -> usize {
            core::intrinsics::lrr_unsupported("core::sync::atomic::AtomicUsize::load")
        }
        pub fn store(&self, val: usize, order: Ordering) {
            core::intrinsics::lrr_unsupported("core::sync::atomic::AtomicUsize::store")
        }
        pub fn fetch_add(&self, val: usize, order: Ordering) -> usize {
            core::intrinsics::lrr_unsupported("core::sync::atomic::AtomicUsize::fetch_add")
        }
        pub fn fetch_sub(&self, val: usize, order: Ordering) -> usize {
            core::intrinsics::lrr_unsupported("core::sync::atomic::AtomicUsize::fetch_sub")
        }
        pub fn swap(&self, val: usize, order: Ordering) -> usize {
            core::intrinsics::lrr_unsupported("core::sync::atomic::AtomicUsize::swap")
        }
    }
    impl AtomicIsize {
        pub const fn new(v: isize) -> AtomicIsize {
            AtomicIsize {
                v: UnsafeCell::new(v),
            }
        }
        pub fn load(&self, order: Ordering) -> isize {
            core::intrinsics::lrr_unsupported("core::sync::atomic::AtomicIsize::load")
        }
        pub fn store(&self, val: isize, order: Ordering) {
            core::intrinsics::lrr_unsupported("core::sync::atomic::AtomicIsize::store")
        }
        pub fn fetch_add(&self, val: isize, order: Ordering) -> isize {
            core::intrinsics::lrr_unsupported("core::sync::atomic::AtomicIsize::fetch_add")
        }
    }

    pub fn fence(order: Ordering) {
    }
    pub fn compiler_fence(order: Ordering) {
    }
}
