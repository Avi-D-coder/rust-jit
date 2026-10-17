// LRR-provided `alloc` facade (Section 17). See `core/lib.rs`; the closed
// facade function `alloc::__export::must_use` is installed separately and not
// redeclared here.
#![no_std]


pub mod borrow;
pub mod boxed;
pub mod collections;
pub mod fmt;
pub mod intrinsics;
pub mod rc;
pub mod slice;
pub mod str;
pub mod string;
pub mod sync;
pub mod vec;
