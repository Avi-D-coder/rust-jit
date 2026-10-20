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
pub mod intrinsics;
mod int_convert;
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
    pub const RADIX: u32 = 2;
    pub const MANTISSA_DIGITS: u32 = 24;
    pub const DIGITS: u32 = 6;
    pub const MIN_EXP: i32 = -125;
    pub const MAX_EXP: i32 = 128;
    pub const MIN_10_EXP: i32 = -37;
    pub const MAX_10_EXP: i32 = 38;
    pub mod consts {
        pub const PI: f32 = 3.14159265358979323846264338327950288_f32;
        pub const TAU: f32 = 6.28318530717958647692528676655900577_f32;
        pub const FRAC_PI_2: f32 = 1.57079632679489661923132169163975144_f32;
        pub const FRAC_PI_3: f32 = 1.04719755119659774615421446109316763_f32;
        pub const FRAC_PI_4: f32 = 0.785398163397448309615660845819875721_f32;
        pub const FRAC_PI_6: f32 = 0.52359877559829887307710723054658381_f32;
        pub const FRAC_PI_8: f32 = 0.39269908169872415480783042290993786_f32;
        pub const FRAC_1_PI: f32 = 0.318309886183790671537767526745028724_f32;
        pub const FRAC_2_PI: f32 = 0.636619772367581343075535053490057448_f32;
        pub const FRAC_2_SQRT_PI: f32 = 1.12837916709551257389615890312154517_f32;
        pub const SQRT_2: f32 = 1.41421356237309504880168872420969808_f32;
        pub const FRAC_1_SQRT_2: f32 = 0.707106781186547524400844362104849039_f32;
        pub const E: f32 = 2.71828182845904523536028747135266250_f32;
        pub const LOG2_10: f32 = 3.32192809488736234787031942948939018_f32;
        pub const LOG2_E: f32 = 1.44269504088896340735992468100189214_f32;
        pub const LOG10_2: f32 = 0.301029995663981195213738894724493027_f32;
        pub const LOG10_E: f32 = 0.434294481903251827651128918916605082_f32;
        pub const LN_2: f32 = 0.693147180559945309417232121458176568_f32;
        pub const LN_10: f32 = 2.30258509299404568401799145468436421_f32;
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
    pub const RADIX: u32 = 2;
    pub const MANTISSA_DIGITS: u32 = 53;
    pub const DIGITS: u32 = 15;
    pub const MIN_EXP: i32 = -1021;
    pub const MAX_EXP: i32 = 1024;
    pub const MIN_10_EXP: i32 = -307;
    pub const MAX_10_EXP: i32 = 308;
    pub mod consts {
        pub const PI: f64 = 3.14159265358979323846264338327950288_f64;
        pub const TAU: f64 = 6.28318530717958647692528676655900577_f64;
        pub const FRAC_PI_2: f64 = 1.57079632679489661923132169163975144_f64;
        pub const FRAC_PI_3: f64 = 1.04719755119659774615421446109316763_f64;
        pub const FRAC_PI_4: f64 = 0.785398163397448309615660845819875721_f64;
        pub const FRAC_PI_6: f64 = 0.52359877559829887307710723054658381_f64;
        pub const FRAC_PI_8: f64 = 0.39269908169872415480783042290993786_f64;
        pub const FRAC_1_PI: f64 = 0.318309886183790671537767526745028724_f64;
        pub const FRAC_2_PI: f64 = 0.636619772367581343075535053490057448_f64;
        pub const FRAC_2_SQRT_PI: f64 = 1.12837916709551257389615890312154517_f64;
        pub const SQRT_2: f64 = 1.41421356237309504880168872420969808_f64;
        pub const FRAC_1_SQRT_2: f64 = 0.707106781186547524400844362104849039_f64;
        pub const E: f64 = 2.71828182845904523536028747135266250_f64;
        pub const LOG2_10: f64 = 3.32192809488736234787031942948939018_f64;
        pub const LOG2_E: f64 = 1.44269504088896340735992468100189214_f64;
        pub const LOG10_2: f64 = 0.301029995663981195213738894724493027_f64;
        pub const LOG10_E: f64 = 0.434294481903251827651128918916605082_f64;
        pub const LN_2: f64 = 0.693147180559945309417232121458176568_f64;
        pub const LN_10: f64 = 2.30258509299404568401799145468436421_f64;
    }
}
