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
    pub use core::f32::{consts, EPSILON, INFINITY, MAX, MIN, MIN_POSITIVE, NAN, NEG_INFINITY};
}
pub mod f64 {
    pub use core::f64::{consts, EPSILON, INFINITY, MAX, MIN, MIN_POSITIVE, NAN, NEG_INFINITY};
}
pub mod task {
    pub use core::task::{Context, Poll, RawWaker, Waker};
}
