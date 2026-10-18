// `core::sync`. Atomic scalar accesses are one serialized GuestStore
// intrinsic action; the method bodies and Ordering selection are ordinary Rust.
pub mod atomic {
    use crate::cell::UnsafeCell;

    #[derive(Clone, Copy)]
    pub enum Ordering {
        Relaxed,
        Release,
        Acquire,
        AcqRel,
        SeqCst,
    }

    fn ordering(order: Ordering) -> u8 {
        match order {
            Ordering::Relaxed => 0,
            Ordering::Release => 1,
            Ordering::Acquire => 2,
            Ordering::AcqRel => 3,
            Ordering::SeqCst => 4,
        }
    }

    pub struct AtomicBool {
        v: UnsafeCell<bool>,
    }
    impl AtomicBool {
        pub const fn new(value: bool) -> AtomicBool {
            AtomicBool {
                v: UnsafeCell::new(value),
            }
        }
        pub fn load(&self, order: Ordering) -> bool {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), false, 0u8, ordering(order))
        }
        pub fn store(&self, value: bool, order: Ordering) {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 1u8, ordering(order));
        }
        pub fn swap(&self, value: bool, order: Ordering) -> bool {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 2u8, ordering(order))
        }
        pub fn fetch_and(&self, value: bool, order: Ordering) -> bool {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 5u8, ordering(order))
        }
        pub fn fetch_or(&self, value: bool, order: Ordering) -> bool {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 6u8, ordering(order))
        }
        pub fn fetch_xor(&self, value: bool, order: Ordering) -> bool {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 7u8, ordering(order))
        }
        pub fn fetch_nand(&self, value: bool, order: Ordering) -> bool {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 8u8, ordering(order))
        }
    }

    pub struct AtomicUsize {
        v: UnsafeCell<usize>,
    }
    impl AtomicUsize {
        pub const fn new(value: usize) -> AtomicUsize {
            AtomicUsize {
                v: UnsafeCell::new(value),
            }
        }
        pub fn load(&self, order: Ordering) -> usize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), 0usize, 0u8, ordering(order))
        }
        pub fn store(&self, value: usize, order: Ordering) {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 1u8, ordering(order));
        }
        pub fn swap(&self, value: usize, order: Ordering) -> usize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 2u8, ordering(order))
        }
        pub fn fetch_add(&self, value: usize, order: Ordering) -> usize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 3u8, ordering(order))
        }
        pub fn fetch_sub(&self, value: usize, order: Ordering) -> usize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 4u8, ordering(order))
        }
        pub fn fetch_and(&self, value: usize, order: Ordering) -> usize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 5u8, ordering(order))
        }
        pub fn fetch_or(&self, value: usize, order: Ordering) -> usize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 6u8, ordering(order))
        }
        pub fn fetch_xor(&self, value: usize, order: Ordering) -> usize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 7u8, ordering(order))
        }
        pub fn fetch_nand(&self, value: usize, order: Ordering) -> usize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 8u8, ordering(order))
        }
    }

    pub struct AtomicIsize {
        v: UnsafeCell<isize>,
    }
    impl AtomicIsize {
        pub const fn new(value: isize) -> AtomicIsize {
            AtomicIsize {
                v: UnsafeCell::new(value),
            }
        }
        pub fn load(&self, order: Ordering) -> isize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), 0isize, 0u8, ordering(order))
        }
        pub fn store(&self, value: isize, order: Ordering) {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 1u8, ordering(order));
        }
        pub fn swap(&self, value: isize, order: Ordering) -> isize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 2u8, ordering(order))
        }
        pub fn fetch_add(&self, value: isize, order: Ordering) -> isize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 3u8, ordering(order))
        }
        pub fn fetch_sub(&self, value: isize, order: Ordering) -> isize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 4u8, ordering(order))
        }
        pub fn fetch_and(&self, value: isize, order: Ordering) -> isize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 5u8, ordering(order))
        }
        pub fn fetch_or(&self, value: isize, order: Ordering) -> isize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 6u8, ordering(order))
        }
        pub fn fetch_xor(&self, value: isize, order: Ordering) -> isize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 7u8, ordering(order))
        }
        pub fn fetch_nand(&self, value: isize, order: Ordering) -> isize {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 8u8, ordering(order))
        }
    }

    pub struct AtomicU32 {
        v: UnsafeCell<u32>,
    }
    impl AtomicU32 {
        pub const fn new(value: u32) -> AtomicU32 {
            AtomicU32 {
                v: UnsafeCell::new(value),
            }
        }
        pub fn load(&self, order: Ordering) -> u32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), 0u32, 0u8, ordering(order))
        }
        pub fn store(&self, value: u32, order: Ordering) {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 1u8, ordering(order));
        }
        pub fn swap(&self, value: u32, order: Ordering) -> u32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 2u8, ordering(order))
        }
        pub fn fetch_add(&self, value: u32, order: Ordering) -> u32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 3u8, ordering(order))
        }
        pub fn fetch_sub(&self, value: u32, order: Ordering) -> u32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 4u8, ordering(order))
        }
        pub fn fetch_and(&self, value: u32, order: Ordering) -> u32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 5u8, ordering(order))
        }
        pub fn fetch_or(&self, value: u32, order: Ordering) -> u32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 6u8, ordering(order))
        }
        pub fn fetch_xor(&self, value: u32, order: Ordering) -> u32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 7u8, ordering(order))
        }
        pub fn fetch_nand(&self, value: u32, order: Ordering) -> u32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 8u8, ordering(order))
        }
    }

    pub struct AtomicI32 {
        v: UnsafeCell<i32>,
    }
    impl AtomicI32 {
        pub const fn new(value: i32) -> AtomicI32 {
            AtomicI32 {
                v: UnsafeCell::new(value),
            }
        }
        pub fn load(&self, order: Ordering) -> i32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), 0i32, 0u8, ordering(order))
        }
        pub fn store(&self, value: i32, order: Ordering) {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 1u8, ordering(order));
        }
        pub fn swap(&self, value: i32, order: Ordering) -> i32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 2u8, ordering(order))
        }
        pub fn fetch_add(&self, value: i32, order: Ordering) -> i32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 3u8, ordering(order))
        }
        pub fn fetch_sub(&self, value: i32, order: Ordering) -> i32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 4u8, ordering(order))
        }
        pub fn fetch_and(&self, value: i32, order: Ordering) -> i32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 5u8, ordering(order))
        }
        pub fn fetch_or(&self, value: i32, order: Ordering) -> i32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 6u8, ordering(order))
        }
        pub fn fetch_xor(&self, value: i32, order: Ordering) -> i32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 7u8, ordering(order))
        }
        pub fn fetch_nand(&self, value: i32, order: Ordering) -> i32 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 8u8, ordering(order))
        }
    }

    pub struct AtomicU64 {
        v: UnsafeCell<u64>,
    }
    impl AtomicU64 {
        pub const fn new(value: u64) -> AtomicU64 {
            AtomicU64 {
                v: UnsafeCell::new(value),
            }
        }
        pub fn load(&self, order: Ordering) -> u64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), 0u64, 0u8, ordering(order))
        }
        pub fn store(&self, value: u64, order: Ordering) {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 1u8, ordering(order));
        }
        pub fn swap(&self, value: u64, order: Ordering) -> u64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 2u8, ordering(order))
        }
        pub fn fetch_add(&self, value: u64, order: Ordering) -> u64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 3u8, ordering(order))
        }
        pub fn fetch_sub(&self, value: u64, order: Ordering) -> u64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 4u8, ordering(order))
        }
        pub fn fetch_and(&self, value: u64, order: Ordering) -> u64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 5u8, ordering(order))
        }
        pub fn fetch_or(&self, value: u64, order: Ordering) -> u64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 6u8, ordering(order))
        }
        pub fn fetch_xor(&self, value: u64, order: Ordering) -> u64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 7u8, ordering(order))
        }
        pub fn fetch_nand(&self, value: u64, order: Ordering) -> u64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 8u8, ordering(order))
        }
    }

    pub struct AtomicI64 {
        v: UnsafeCell<i64>,
    }
    impl AtomicI64 {
        pub const fn new(value: i64) -> AtomicI64 {
            AtomicI64 {
                v: UnsafeCell::new(value),
            }
        }
        pub fn load(&self, order: Ordering) -> i64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), 0i64, 0u8, ordering(order))
        }
        pub fn store(&self, value: i64, order: Ordering) {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 1u8, ordering(order));
        }
        pub fn swap(&self, value: i64, order: Ordering) -> i64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 2u8, ordering(order))
        }
        pub fn fetch_add(&self, value: i64, order: Ordering) -> i64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 3u8, ordering(order))
        }
        pub fn fetch_sub(&self, value: i64, order: Ordering) -> i64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 4u8, ordering(order))
        }
        pub fn fetch_and(&self, value: i64, order: Ordering) -> i64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 5u8, ordering(order))
        }
        pub fn fetch_or(&self, value: i64, order: Ordering) -> i64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 6u8, ordering(order))
        }
        pub fn fetch_xor(&self, value: i64, order: Ordering) -> i64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 7u8, ordering(order))
        }
        pub fn fetch_nand(&self, value: i64, order: Ordering) -> i64 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 8u8, ordering(order))
        }
    }

    pub struct AtomicU8 {
        v: UnsafeCell<u8>,
    }
    impl AtomicU8 {
        pub const fn new(value: u8) -> AtomicU8 {
            AtomicU8 {
                v: UnsafeCell::new(value),
            }
        }
        pub fn load(&self, order: Ordering) -> u8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), 0u8, 0u8, ordering(order))
        }
        pub fn store(&self, value: u8, order: Ordering) {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 1u8, ordering(order));
        }
        pub fn swap(&self, value: u8, order: Ordering) -> u8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 2u8, ordering(order))
        }
        pub fn fetch_add(&self, value: u8, order: Ordering) -> u8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 3u8, ordering(order))
        }
        pub fn fetch_sub(&self, value: u8, order: Ordering) -> u8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 4u8, ordering(order))
        }
        pub fn fetch_and(&self, value: u8, order: Ordering) -> u8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 5u8, ordering(order))
        }
        pub fn fetch_or(&self, value: u8, order: Ordering) -> u8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 6u8, ordering(order))
        }
        pub fn fetch_xor(&self, value: u8, order: Ordering) -> u8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 7u8, ordering(order))
        }
        pub fn fetch_nand(&self, value: u8, order: Ordering) -> u8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 8u8, ordering(order))
        }
    }

    pub struct AtomicI8 {
        v: UnsafeCell<i8>,
    }
    impl AtomicI8 {
        pub const fn new(value: i8) -> AtomicI8 {
            AtomicI8 {
                v: UnsafeCell::new(value),
            }
        }
        pub fn load(&self, order: Ordering) -> i8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), 0i8, 0u8, ordering(order))
        }
        pub fn store(&self, value: i8, order: Ordering) {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 1u8, ordering(order));
        }
        pub fn swap(&self, value: i8, order: Ordering) -> i8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 2u8, ordering(order))
        }
        pub fn fetch_add(&self, value: i8, order: Ordering) -> i8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 3u8, ordering(order))
        }
        pub fn fetch_sub(&self, value: i8, order: Ordering) -> i8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 4u8, ordering(order))
        }
        pub fn fetch_and(&self, value: i8, order: Ordering) -> i8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 5u8, ordering(order))
        }
        pub fn fetch_or(&self, value: i8, order: Ordering) -> i8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 6u8, ordering(order))
        }
        pub fn fetch_xor(&self, value: i8, order: Ordering) -> i8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 7u8, ordering(order))
        }
        pub fn fetch_nand(&self, value: i8, order: Ordering) -> i8 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 8u8, ordering(order))
        }
    }

    pub struct AtomicU16 {
        v: UnsafeCell<u16>,
    }
    impl AtomicU16 {
        pub const fn new(value: u16) -> AtomicU16 {
            AtomicU16 {
                v: UnsafeCell::new(value),
            }
        }
        pub fn load(&self, order: Ordering) -> u16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), 0u16, 0u8, ordering(order))
        }
        pub fn store(&self, value: u16, order: Ordering) {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 1u8, ordering(order));
        }
        pub fn swap(&self, value: u16, order: Ordering) -> u16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 2u8, ordering(order))
        }
        pub fn fetch_add(&self, value: u16, order: Ordering) -> u16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 3u8, ordering(order))
        }
        pub fn fetch_sub(&self, value: u16, order: Ordering) -> u16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 4u8, ordering(order))
        }
        pub fn fetch_and(&self, value: u16, order: Ordering) -> u16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 5u8, ordering(order))
        }
        pub fn fetch_or(&self, value: u16, order: Ordering) -> u16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 6u8, ordering(order))
        }
        pub fn fetch_xor(&self, value: u16, order: Ordering) -> u16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 7u8, ordering(order))
        }
        pub fn fetch_nand(&self, value: u16, order: Ordering) -> u16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 8u8, ordering(order))
        }
    }

    pub struct AtomicI16 {
        v: UnsafeCell<i16>,
    }
    impl AtomicI16 {
        pub const fn new(value: i16) -> AtomicI16 {
            AtomicI16 {
                v: UnsafeCell::new(value),
            }
        }
        pub fn load(&self, order: Ordering) -> i16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), 0i16, 0u8, ordering(order))
        }
        pub fn store(&self, value: i16, order: Ordering) {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 1u8, ordering(order));
        }
        pub fn swap(&self, value: i16, order: Ordering) -> i16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 2u8, ordering(order))
        }
        pub fn fetch_add(&self, value: i16, order: Ordering) -> i16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 3u8, ordering(order))
        }
        pub fn fetch_sub(&self, value: i16, order: Ordering) -> i16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 4u8, ordering(order))
        }
        pub fn fetch_and(&self, value: i16, order: Ordering) -> i16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 5u8, ordering(order))
        }
        pub fn fetch_or(&self, value: i16, order: Ordering) -> i16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 6u8, ordering(order))
        }
        pub fn fetch_xor(&self, value: i16, order: Ordering) -> i16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 7u8, ordering(order))
        }
        pub fn fetch_nand(&self, value: i16, order: Ordering) -> i16 {
            core::intrinsics::lrr_atomic_op(self.v.lrr_get(), value, 8u8, ordering(order))
        }
    }

    pub fn fence(order: Ordering) {
        match order {
            Ordering::Relaxed => {
                core::panicking::panic("there is no such thing as a relaxed fence")
            }
            _ => (),
        }
    }
    pub fn compiler_fence(order: Ordering) {
        fence(order);
    }
}
