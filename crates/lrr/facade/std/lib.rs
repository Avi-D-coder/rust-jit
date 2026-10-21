// LRR-provided `std` facade (Section 17). See `core/lib.rs`. The module
// layout follows the real `std` (re-exporting `core`/`alloc` modules); the
// closed facade functions in `std::rt` and `std::io::{_print, _eprint}` are
// installed separately and not redeclared here.
#![no_std]


extern crate alloc as alloc_crate;

pub mod prelude;

pub use core::any;
pub use core::array;
pub use core::cell;
pub use core::char;
pub use core::clone;
pub use core::cmp;
pub use core::convert;
pub use core::default;
pub use core::future;
pub use core::hint;
pub use core::intrinsics;
pub use core::iter;
pub use core::marker;
pub use core::mem;
pub use core::ops;
pub use core::option;
pub use core::pin;
pub use core::ptr;
pub use core::result;

pub use core::{i128, i16, i32, i64, i8, isize, u128, u16, u32, u64, u8, usize};

pub use alloc_crate::borrow;
pub use alloc_crate::boxed;
pub use alloc_crate::fmt;
pub use alloc_crate::rc;
pub use alloc_crate::slice;
pub use alloc_crate::str;
pub use alloc_crate::string;
pub use alloc_crate::vec;

pub mod collections;
pub mod env;
pub mod error;
pub mod ffi;
pub mod hash;
pub mod io;
pub mod num;
pub mod panic;
pub mod process;
pub mod sync;
pub mod thread;
pub mod time;

pub mod f32 {
    pub use core::f32::{
        consts, DIGITS, EPSILON, INFINITY, MANTISSA_DIGITS, MAX, MAX_10_EXP, MAX_EXP, MIN, MIN_10_EXP,
        MIN_EXP, MIN_POSITIVE, NAN, NEG_INFINITY, RADIX,
    };
}
pub mod f64 {
    pub use core::f64::{
        consts, DIGITS, EPSILON, INFINITY, MANTISSA_DIGITS, MAX, MAX_10_EXP, MAX_EXP, MIN, MIN_10_EXP,
        MIN_EXP, MIN_POSITIVE, NAN, NEG_INFINITY, RADIX,
    };
}
// `std::alloc`: the allocation API (real std re-exports `alloc::alloc`).
pub mod alloc {
    pub use alloc_crate::alloc::{alloc, alloc_zeroed, dealloc, handle_alloc_error, realloc};
    pub use alloc_crate::alloc::{AllocError, Allocator, Global, GlobalAlloc, Layout, LayoutError, System};
}
// `std::os::raw`: the C type aliases (real std re-exports `core::ffi`).
pub mod os {
    pub mod raw {
        pub use core::ffi::{c_char, c_double, c_float, c_int, c_long, c_longlong, c_schar, c_short};
        pub use core::ffi::{c_uchar, c_uint, c_ulong, c_ulonglong, c_ushort, c_void};
    }
}
pub mod task {
    pub use core::task::{Context, Poll, RawWaker, Waker};
}
