// LRR-provided `core` facade (Section 17).
//
// Ordinary Rust declarations of the standard library items upstream rustc
// tests name. They resolve through the ordinary module/import/prelude
// pipeline under the same crate unit as the real library macro definitions
// and the closed facade functions (`core::panicking`, `core::intrinsics`),
// which are installed separately and are not redeclared here. A reached body
// that the interpreter cannot execute yet calls the explicit
// `core::intrinsics::lrr_unsupported` intrinsic, which faults naming the
// item; nothing here substitutes different behavior. Bodies call intrinsics
// by `core::` paths (no macros, so discovery expands nothing here). The
// sources mean the same in every edition. Like the real `core` (whose
// `#[prelude_import]` and `extern crate self as core` give it the same
// names), this crate sees itself as `core` and its own prelude: as a
// `no_std` crate it receives the compiler-injected `core` import.
#![no_std]

pub mod any;
pub mod array;
pub mod borrow;
pub mod cell;
pub mod char;
pub mod clone;
pub mod cmp;
pub mod convert;
pub mod default;
pub mod error;
pub mod ffi;
pub mod fmt;
mod float_impls;
pub mod future;
pub mod hash;
pub mod hint;
mod int_impls;
pub mod iter;
pub mod marker;
pub mod mem;
pub mod num;
pub mod ops;
pub mod option;
pub mod panic;
pub mod panicking;
pub mod pin;
pub mod prelude;
mod primitive_impls;
pub mod ptr;
pub mod result;
pub mod slice;
mod step_impls;
pub mod str;
pub mod sync;
pub mod task;
pub mod time;

// Legacy integer and float modules (`core::u8::MAX`, ...).
pub mod i8 {
    pub const MIN: i8 = -128;
    pub const MAX: i8 = 127;
}
pub mod i16 {
    pub const MIN: i16 = -32768;
    pub const MAX: i16 = 32767;
}
pub mod i32 {
    pub const MIN: i32 = -2147483648;
    pub const MAX: i32 = 2147483647;
}
pub mod i64 {
    pub const MIN: i64 = -9223372036854775808;
    pub const MAX: i64 = 9223372036854775807;
}
pub mod i128 {
    pub const MIN: i128 = -170141183460469231731687303715884105728;
    pub const MAX: i128 = 170141183460469231731687303715884105727;
}
pub mod isize {
    pub const MIN: isize = -9223372036854775808;
    pub const MAX: isize = 9223372036854775807;
}
pub mod u8 {
    pub const MIN: u8 = 0;
    pub const MAX: u8 = 255;
}
pub mod u16 {
    pub const MIN: u16 = 0;
    pub const MAX: u16 = 65535;
}
pub mod u32 {
    pub const MIN: u32 = 0;
    pub const MAX: u32 = 4294967295;
}
pub mod u64 {
    pub const MIN: u64 = 0;
    pub const MAX: u64 = 18446744073709551615;
}
pub mod u128 {
    pub const MIN: u128 = 0;
    pub const MAX: u128 = 340282366920938463463374607431768211455;
}
pub mod usize {
    pub const MIN: usize = 0;
    pub const MAX: usize = 18446744073709551615;
}
pub mod f32 {
    pub const EPSILON: f32 = 1.19209290e-07_f32;
    pub const MIN: f32 = -3.40282347e+38_f32;
    pub const MAX: f32 = 3.40282347e+38_f32;
    pub const MIN_POSITIVE: f32 = 1.17549435e-38_f32;
    pub const INFINITY: f32 = 1.0_f32 / 0.0_f32;
    pub const NEG_INFINITY: f32 = -1.0_f32 / 0.0_f32;
    pub const NAN: f32 = 0.0_f32 / 0.0_f32;
    pub mod consts {
        pub const PI: f32 = 3.14159265358979323846264338327950288_f32;
        pub const E: f32 = 2.71828182845904523536028747135266250_f32;
    }
}
pub mod f64 {
    pub const EPSILON: f64 = 2.2204460492503131e-16_f64;
    pub const MIN: f64 = -1.7976931348623157e+308_f64;
    pub const MAX: f64 = 1.7976931348623157e+308_f64;
    pub const MIN_POSITIVE: f64 = 2.2250738585072014e-308_f64;
    pub const INFINITY: f64 = 1.0_f64 / 0.0_f64;
    pub const NEG_INFINITY: f64 = -1.0_f64 / 0.0_f64;
    pub const NAN: f64 = 0.0_f64 / 0.0_f64;
    pub mod consts {
        pub const PI: f64 = 3.14159265358979323846264338327950288_f64;
        pub const E: f64 = 2.71828182845904523536028747135266250_f64;
    }
}
