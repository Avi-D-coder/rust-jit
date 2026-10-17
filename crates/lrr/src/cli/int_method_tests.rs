//! Inherent methods and associated constants of the primitive integer and
//! float types (`facade/core/int_impls.rs`, `float_impls.rs`), written as
//! ordinary Rust in the core facade and selected by the ordinary method and
//! associated-constant resolution: `x.wrapping_add(y)`, `u8::MAX`,
//! `i32::from_be_bytes(..)`, `f64::EPSILON`, ... Every program's expected
//! output was produced by real rustc 1.99.0 running the identical source, and
//! each test runs in all four editions.
use super::format_trait_facade_tests::{prints, run};
use crate::{cli::StandaloneOutcome, vm::numeric::PanicKind};

/// A program that must panic with the operator panic `kind` (the interpreter
/// reports operator overflow/division panics by kind, not by rustc's text) in
/// every edition.
fn panics_with_kind(label: &str, source: &str, expected: PanicKind) {
    for edition in [2015, 2018, 2021, 2024] {
        match run(label, source, edition) {
            Ok(StandaloneOutcome::Panicked(report)) => {
                assert_eq!(report.panic.kind, expected, "{edition}\n{source}")
            }
            Ok(_) => panic!("{edition} did not panic\n{source}"),
            Err(error) => panic!("{edition} {error}\n{source}"),
        }
    }
}

/// A program that must panic with exactly `expected` in every edition.
fn panics_with(label: &str, source: &str, expected: &str) {
    for edition in [2015, 2018, 2021, 2024] {
        match run(label, source, edition) {
            Ok(StandaloneOutcome::Panicked(report)) => {
                assert_eq!(report.panic.description(), expected, "{edition}\n{source}")
            }
            Ok(_) => panic!("{edition} did not panic\n{source}"),
            Err(error) => panic!("{edition} {error}\n{source}"),
        }
    }
}

/// Signed wrapping/checked/saturating/overflowing/euclid/abs families at several widths (expected output from rustc 1.99.0).
#[test]
fn signed_arithmetic_variants() {
    prints(
        "int-signed_arith",
        r#"fn main() {
    println!("{} {} {}", i8::MIN, i32::MAX, i64::MIN);
    println!("{} {} {}", i8::BITS, i32::BITS, i64::BITS);
    println!("{} {} {}", 100i8.wrapping_add(100), (-100i8).wrapping_sub(100), 100i8.wrapping_mul(3));
    println!("{:?} {:?} {:?}", 100i8.checked_add(27), 100i8.checked_add(28), i8::MIN.checked_abs());
    println!("{:?} {:?}", i32::MAX.overflowing_add(1), i32::MIN.overflowing_neg());
    println!("{} {} {}", i32::MAX.saturating_add(5), i32::MIN.saturating_sub(5), i32::MIN.saturating_mul(2));
    println!("{} {} {}", i64::MAX.wrapping_add(1), i64::MIN.wrapping_sub(1), i64::MAX.wrapping_mul(i64::MAX));
    println!("{:?} {:?} {:?}", i64::MAX.checked_mul(2), i64::MIN.overflowing_mul(-1), i64::MIN.checked_sub(1));
    println!("{} {} {}", (-5i32).abs(), (-5i32).signum(), 0i32.signum());
    println!("{} {} {} {}", (-7i32).rem_euclid(3), (-7i32).div_euclid(3), 7i32.rem_euclid(-3), 7i32.div_euclid(-3));
    println!("{} {}", i32::MIN.unsigned_abs(), 5i32.abs_diff(-3));
    println!("{} {} {}", i8::MIN.wrapping_abs(), i8::MIN.wrapping_neg(), i32::MIN.wrapping_div(-1));
    println!("{:?} {:?} {:?}", i32::MIN.checked_div(-1), 5i32.checked_rem(0), i32::MIN.overflowing_rem(-1));
    println!("{} {} {}", i16::MAX.saturating_pow(2), (-3i16).saturating_pow(11), (-2i64).pow(63));
    println!("{} {} {}", i32::MIN.saturating_neg(), i32::MIN.saturating_abs(), i32::MAX.is_positive());
    println!("{} {} {}", (-1i32).is_negative(), 3i32.midpoint(10), (-7i32).midpoint(0));
    println!("{:?} {:?}", 5i32.checked_add_unsigned(4000000000), 5i32.overflowing_sub_unsigned(7));
}
"#,
        r#"-128 2147483647 -9223372036854775808
8 32 64
-56 56 44
Some(127) None None
(-2147483648, true) (-2147483648, true)
2147483647 -2147483648 -2147483648
-9223372036854775808 9223372036854775807 1
None (-9223372036854775808, true) None
5 -1 0
2 -3 1 -2
2147483648 8
-128 -128 -2147483648
None None (0, true)
32767 -32768 -9223372036854775808
2147483647 2147483647 true
true 6 -3
None (-2, false)
"#,
    );
}

/// Unsigned arithmetic variants, abs_diff, division helpers and the mixed-sign forms (rustc 1.99.0 output).
#[test]
fn unsigned_arithmetic_variants() {
    prints(
        "int-unsigned_arith",
        r#"fn main() {
    println!("{} {} {}", u8::MAX, u32::MAX, u64::MAX);
    println!("{} {} {}", u8::BITS, u32::BITS, usize::BITS);
    println!("{} {} {}", 200u8.wrapping_add(100), 5u8.wrapping_sub(10), 25u8.wrapping_mul(12));
    println!("{:?} {:?} {:?}", 200u8.checked_add(55), 200u8.checked_add(56), 0u8.checked_sub(1));
    println!("{:?} {:?} {:?}", u32::MAX.overflowing_add(1), 0u32.overflowing_sub(1), u32::MAX.overflowing_mul(2));
    println!("{} {} {}", u32::MAX.saturating_add(5), 3u32.saturating_sub(5), u32::MAX.saturating_mul(2));
    println!("{} {} {}", u64::MAX.wrapping_add(2), 0u64.wrapping_sub(1), u64::MAX.wrapping_mul(u64::MAX));
    println!("{:?} {:?} {:?}", u64::MAX.checked_mul(2), u64::MAX.overflowing_mul(3), 7u64.checked_sub(7));
    println!("{} {} {}", 10u32.abs_diff(25), 25u32.abs_diff(10), 5u8.wrapping_neg());
    println!("{:?} {:?} {:?}", 0u8.checked_neg(), 1u8.checked_neg(), 1u8.overflowing_neg());
    println!("{} {} {}", 7u32.div_ceil(2), 7u32.next_multiple_of(4), 8u32.next_multiple_of(4));
    println!("{:?} {:?}", u32::MAX.checked_next_multiple_of(4), 6u32.checked_div(0));
    println!("{} {} {}", 9u32.rem_euclid(4), 9u32.div_euclid(4), 6u32.is_multiple_of(3));
    println!("{} {} {}", u8::MAX.midpoint(1), 5u64.midpoint(u64::MAX), usize::MAX.wrapping_add(1));
    println!("{:?} {:?}", 5u32.checked_add_signed(-6), 5u32.saturating_add_signed(-6));
    println!("{} {}", 5u32.wrapping_add_signed(-6), 5u8.cast_signed());
}
"#,
        r#"255 4294967295 18446744073709551615
8 32 64
44 251 44
Some(255) None None
(0, true) (4294967295, true) (4294967294, true)
4294967295 0 4294967295
1 18446744073709551615 1
None (18446744073709551613, true) Some(0)
15 15 251
Some(0) None (255, true)
4 8 8
None None
1 2 true
128 9223372036854775810 0
None 0
4294967295 5
"#,
    );
}

/// Bit counting, rotates (including counts beyond the width), byte/bit reversal, shifts and powers of two.
#[test]
fn bit_operations() {
    prints(
        "int-bit_ops",
        r#"fn main() {
    println!("{} {} {}", 0x0Fu8.count_ones(), 0x0Fu8.count_zeros(), (-1i32).count_ones());
    println!("{} {} {}", 1u32.leading_zeros(), 0u16.leading_zeros(), 8u64.trailing_zeros());
    println!("{} {} {}", 0u8.trailing_zeros(), (-1i8).leading_ones(), 7u32.trailing_ones());
    println!("{} {}", i64::MIN.leading_zeros(), i64::MIN.trailing_zeros());
    println!("{} {} {}", 0x10000b3u32.rotate_left(8), 0xb301u32.rotate_right(8), 0b10u16.rotate_left(124));
    println!("{} {}", (-128i8).rotate_left(1), 1i64.rotate_right(1));
    println!("{} {} {}", 0x12345678u32.swap_bytes(), 0x1234u16.swap_bytes(), 0x12u8.swap_bytes());
    println!("{} {} {}", 0x12345678u32.reverse_bits(), 1u8.reverse_bits(), (-2i64).reverse_bits());
    println!("{} {} {}", 0x12345678u32.to_be(), 0x12345678u32.to_le(), u32::from_be(0x78563412));
    println!("{} {} {}", 1u32.wrapping_shl(33), 128u32.wrapping_shr(35), (-8i8).wrapping_shr(9));
    println!("{:?} {:?} {:?}", 1u8.checked_shl(8), 1u8.checked_shl(7), 1u8.checked_shr(8));
    println!("{:?} {:?}", 1u32.overflowing_shl(33), 0x80u8.overflowing_shr(9));
    println!("{} {}", u32::MAX.unbounded_shl(32), (-5i32).unbounded_shr(40));
    println!("{} {} {}", 0u32.is_power_of_two(), 64u32.is_power_of_two(), 65u32.is_power_of_two());
    println!("{} {} {}", 0u32.next_power_of_two(), 17u32.next_power_of_two(), 64u8.next_power_of_two());
    println!("{:?} {:?}", 200u8.checked_next_power_of_two(), 129u8.checked_next_power_of_two());
}
"#,
        r#"4 4 32
31 16 3
8 8 3
0 63
45825 16777395 8192
1 -9223372036854775808
2018915346 13330 18
510274632 128 9223372036854775807
2018915346 305419896 305419896
2 16 -4
None Some(128) None
(2, true) (64, true)
0 -1
false true false
1 32 64
None None
"#,
    );
}

/// to_/from_{be,le,ne}_bytes at several widths, from_str_radix error and boundary cases, min_value/max_value.
#[test]
fn byte_conversions_and_parsing() {
    prints(
        "int-bytes_and_parse",
        r#"fn main() {
    println!("{:?} {:?}", 0x12345678u32.to_be_bytes(), 0x12345678u32.to_le_bytes());
    println!("{:?} {:?}", (-2i16).to_be_bytes(), 0x0102030405060708u64.to_le_bytes());
    println!("{:?} {:?}", 255u8.to_ne_bytes(), i64::MIN.to_be_bytes());
    println!("{} {}", u32::from_be_bytes([0x12, 0x34, 0x56, 0x78]), i32::from_le_bytes([0x12, 0x34, 0x56, 0x78]));
    println!("{} {}", i16::from_be_bytes([0xff, 0xfe]), u64::from_le_bytes([1, 2, 3, 4, 5, 6, 7, 8]));
    println!("{} {}", i8::from_ne_bytes([200]), usize::from_ne_bytes(usize::MAX.to_ne_bytes()));
    println!("{:?} {:?} {:?}", u8::from_str_radix("ff", 16), u8::from_str_radix("100", 16), u8::from_str_radix("-1", 10));
    println!("{:?} {:?} {:?}", i8::from_str_radix("-80", 16), i8::from_str_radix("-81", 16), i8::from_str_radix("7F", 16));
    println!("{:?} {:?} {:?}", i32::from_str_radix("zz", 36), u64::from_str_radix("1_0", 10), i64::from_str_radix("+101", 2));
    println!("{:?} {:?}", u16::from_str_radix("", 10), i16::from_str_radix("-", 10));
    println!("{:?} {:?}", i64::from_str_radix("-9223372036854775808", 10), u64::from_str_radix("18446744073709551616", 10));
    println!("{} {}", u8::min_value(), i16::max_value());
}
"#,
        r#"[18, 52, 86, 120] [120, 86, 52, 18]
[255, 254] [8, 7, 6, 5, 4, 3, 2, 1]
[255] [128, 0, 0, 0, 0, 0, 0, 0]
305419896 2018915346
-2 578437695752307201
-56 18446744073709551615
Ok(255) Err(ParseIntError { kind: PosOverflow }) Err(ParseIntError { kind: InvalidDigit })
Ok(-128) Err(ParseIntError { kind: NegOverflow }) Ok(127)
Ok(1295) Err(ParseIntError { kind: InvalidDigit }) Ok(5)
Err(ParseIntError { kind: Empty }) Err(ParseIntError { kind: InvalidDigit })
Ok(-9223372036854775808) Err(ParseIntError { kind: PosOverflow })
0 32767
"#,
    );
}

/// pow/checked/wrapping/overflowing/saturating_pow, ilog family, isqrt/checked_isqrt.
#[test]
fn powers_logarithms_and_roots() {
    prints(
        "int-pow_log_sqrt",
        r#"fn main() {
    println!("{} {} {}", 3u8.pow(5), 2i32.pow(30), (-2i64).pow(3));
    println!("{:?} {:?} {:?}", 3u8.checked_pow(5), 3u8.checked_pow(6), 2i8.checked_pow(7));
    println!("{} {} {}", 3u8.wrapping_pow(6), 2i32.wrapping_pow(32), 7u64.wrapping_pow(30));
    println!("{:?} {:?}", 3u8.overflowing_pow(6), (-3i8).overflowing_pow(5));
    println!("{} {} {}", 3u8.saturating_pow(6), (-3i8).saturating_pow(5), 10u64.saturating_pow(30));
    println!("{} {} {}", 1000u32.ilog10(), 1023u32.ilog2(), 80u32.ilog(3));
    println!("{:?} {:?} {:?}", 0u32.checked_ilog2(), 99u8.checked_ilog10(), 10i32.checked_ilog(1));
    println!("{} {} {}", 99u32.isqrt(), 100u64.isqrt(), u64::MAX.isqrt());
    println!("{} {:?} {:?}", 17i32.isqrt(), (-1i32).checked_isqrt(), i64::MAX.checked_isqrt());
}
"#,
        r#"243 1073741824 -8
Some(243) None None
217 0 1576789505350337489
(217, true) (13, true)
255 -128 18446744073709551615
3 9 3
None Some(1) None
9 10 4294967295
4 None Some(3037000499)
"#,
    );
}

/// f32/f64 associated constants and inherent methods: host math through the closed intrinsic, bit conversions and the Rust-written ones.
#[test]
fn float_inherent_methods() {
    prints(
        "int-floats",
        r#"fn main() {
    println!("{} {} {}", f64::EPSILON, f32::MAX, f64::MIN_POSITIVE);
    println!("{} {} {} {}", f64::NAN, f64::INFINITY, f32::NEG_INFINITY, f64::MANTISSA_DIGITS);
    println!("{} {} {}", 2.0f64.sqrt(), 2.0f32.sqrt(), (-4.0f64).sqrt().is_nan());
    println!("{} {} {} {}", (-1.5f64).abs(), 2.7f64.floor(), (-2.7f64).ceil(), 2.5f64.round());
    println!("{} {} {}", (-2.5f64).round(), 2.5f64.round_ties_even(), (-2.7f64).trunc());
    println!("{} {} {}", 2.75f64.fract(), 3.5f64.max(1.0), 3.5f32.min(f32::NAN));
    println!("{} {} {}", 2.0f64.powi(10), 2.0f64.powi(-2), 2.0f64.powf(0.5));
    println!("{} {} {}", 1.0f64.sin(), 1.0f64.cos(), 1.0f64.exp());
    println!("{} {} {}", 100.0f64.log10(), 8.0f64.log2(), 1.0f64.atan2(2.0));
    println!("{} {} {}", 3.0f64.hypot(4.0), 2.0f64.mul_add(3.0, 4.0), 5.5f64.rem_euclid(2.0));
    println!("{} {} {}", (-5.5f64).rem_euclid(2.0), (-5.5f64).div_euclid(2.0), 1.5f64.copysign(-0.0));
    println!("{} {} {}", 1.5f64.to_bits(), f64::from_bits(4609434218613702656), 1.5f32.to_bits());
    println!("{} {} {}", (-0.0f64).is_sign_negative(), 0.0f64.is_sign_positive(), (-0.0f64).signum());
    println!("{} {} {}", f64::INFINITY.is_infinite(), 1.0f64.is_finite(), f64::NAN.is_nan());
    println!("{:?} {:?} {:?}", 0.1f64.classify(), 0.0f64.classify(), f64::NAN.classify());
    println!("{:?} {:?}", 1.0f64.total_cmp(&2.0), f64::NAN.total_cmp(&1.0));
    println!("{} {} {}", 180.0f64.to_radians(), 1.0f64.to_degrees(), 7.0f64.clamp(0.0, 5.0));
    println!("{} {}", 1.0f64.next_up(), 1.0f64.next_down());
    println!("{:?} {:?}", 1.5f64.to_be_bytes(), f32::from_le_bytes([0, 0, 192, 63]));
}
"#,
        r#"0.0000000000000002220446049250313 340282350000000000000000000000000000000 0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000022250738585072014
NaN inf -inf 53
1.4142135623730951 1.4142135 true
1.5 2 -2 3
-3 2 -2
0.75 3.5 3.5
1024 0.25 1.4142135623730951
0.8414709848078965 0.5403023058681398 2.718281828459045
2 3 0.4636476090008061
5 10 1.5
0.5 -3 -1.5
4609434218613702656 1.5 1069547520
true true -1
true true true
Normal Zero Nan
Less Greater
3.141592653589793 57.29577951308232 5
1.0000000000000002 0.9999999999999999
[63, 248, 0, 0, 0, 0, 0, 0] 1.5
"#,
    );
}

/// u8 ASCII classification/case methods, `u8::MAX`/`i8::MIN` as pattern constants and array-length operands, Ord::clamp/min.
#[test]
fn ascii_methods_and_associated_constants_in_patterns() {
    prints(
        "int-ascii_and_patterns",
        r#"fn classify(x: u8) -> &'static str {
    match x {
        u8::MAX => "max",
        u8::MIN => "min",
        _ => "other",
    }
}
fn sign(x: i8) -> &'static str {
    match x {
        i8::MIN => "most negative",
        i8::MAX => "most positive",
        _ => "between",
    }
}
fn main() {
    let a = [0u8; u8::MAX as usize];
    println!("{} {} {}", a.len(), classify(255), classify(0));
    println!("{} {} {}", classify(7), sign(-128), sign(127));
    println!("{} {} {}", b'a'.is_ascii_alphabetic(), b'7'.is_ascii_digit(), b' '.is_ascii_whitespace());
    println!("{} {}", b'a'.to_ascii_uppercase(), b'Z'.to_ascii_lowercase());
    let mut c = b'q';
    c.make_ascii_uppercase();
    println!("{}", c);
    println!("{} {}", b'A'.eq_ignore_ascii_case(&b'a'), b'\x0b'.is_ascii_whitespace());
    println!("{} {} {}", 200u8.is_ascii(), b'!'.is_ascii_punctuation(), b'\x1f'.is_ascii_control());
    println!("{} {}", b'f'.is_ascii_hexdigit(), b'g'.is_ascii_hexdigit());
    println!("{} {}", 15i32.clamp(0, 10), 3u8.min(2));
}
"#,
        r#"255 max min
other most negative most positive
true true true
65 122
81
true false
false true true
true false
10 2
"#,
    );
}

/// The same methods and associated constants handwritten, emitted by a user macro, inside a macro-generated fn per type and nested in assert_eq!/println! (one resolution route).
#[test]
fn methods_through_macros_and_wrappers() {
    prints(
        "int-macro_nested",
        r#"macro_rules! wadd {
    ($a:expr, $b:expr) => {
        $a.wrapping_add($b)
    };
}
macro_rules! max_of {
    ($t:ident) => {
        $t::MAX
    };
}
macro_rules! make_fn {
    ($name:ident, $t:ty) => {
        fn $name(x: $t) -> $t {
            x.saturating_mul(2).rotate_left(1) ^ <$t>::MAX
        }
    };
}
make_fn!(on_u8, u8);
make_fn!(on_i32, i32);
fn wrapper(a: u8) -> u8 {
    wadd!(a, 10)
}
fn main() {
    let a = 250u8;
    println!("{} {} {}", a.wrapping_add(10), wadd!(a, 10), wrapper(a));
    assert_eq!(wadd!(a, 10), 4);
    assert_eq!(a.checked_add(10), None);
    assert!(wadd!(a, 10) == a.wrapping_add(10));
    println!("{} {}", max_of!(u8), max_of!(i64));
    println!("{} {}", on_u8(100), on_i32(-5));
    let t = (u32::MAX.count_ones(), 8u8.leading_zeros());
    println!("{:?}", t);
}
"#,
        r#"4 4 4
255 9223372036854775807
110 -2147483630
(32, 4)
"#,
    );
}

/// The same facade methods reached from const/static initializers and ordinary code (E-R5-007).
#[test]
fn integer_methods_in_const_initializers() {
    prints(
        "int-const_use",
        r#"const REVERSE: u32 = 0x12345678_u32.reverse_bits();
const FROM_BE_BYTES: i32 = i32::from_be_bytes([0x12, 0x34, 0x56, 0x78]);
const TO_BE_BYTES: [u8; 4] = 0x12_34_56_78_i32.to_be_bytes();
const POW: u8 = 3u8.pow(5);
const CHECKED_POW_OVERFLOW: Option<u8> = 3u8.checked_pow(6);
const OVERFLOWING_POW: (u8, bool) = 3u8.overflowing_pow(6);
const SHL_B: u32 = 1u32.wrapping_shl(128);
const NEXT: u32 = 3u32.next_power_of_two();
const ABS_MIN: i32 = i32::MIN.wrapping_abs();
const LIMIT: u16 = u16::MAX / 2;
static NEG: i8 = i8::MIN;
fn main() {
    println!("{} {} {:?} {}", REVERSE, FROM_BE_BYTES, TO_BE_BYTES, POW);
    println!("{:?} {:?} {} {}", CHECKED_POW_OVERFLOW, OVERFLOWING_POW, SHL_B, NEXT);
    println!("{} {} {}", ABS_MIN, LIMIT, NEG);
    let v = [1u8, 2, 3];
    let mut total = 0u32;
    for x in v.iter() {
        total += x.wrapping_mul(100) as u32;
    }
    println!("{}", total);
}
"#,
        r#"510274632 305419896 [18, 52, 86, 120] 243
None (217, true) 1 4
-2147483648 32767 -128
344
"#,
    );
}

#[test]
fn abs_of_min_panics_on_negate_overflow() {
    // rustc: "attempt to negate with overflow"
    panics_with_kind(
        "int-panic",
        r#"fn main() {
    let m = i32::MIN;
    println!("{}", m.abs());
}
"#,
        PanicKind::Overflow,
    );
}

#[test]
fn pow_overflow_panics() {
    panics_with(
        "int-panic",
        r#"fn main() {
    let e = 8u32;
    println!("{}", 2u8.pow(e));
}
"#,
        "attempt to exponentiate with overflow",
    );
}

#[test]
fn ilog2_of_zero_panics() {
    panics_with(
        "int-panic",
        r#"fn main() {
    let z = 0u32;
    println!("{}", z.ilog2());
}
"#,
        "argument of integer logarithm must be positive",
    );
}

#[test]
fn ilog_base_below_two_panics() {
    panics_with(
        "int-panic",
        r#"fn main() {
    let b = 1u32;
    println!("{}", 5u32.ilog(b));
}
"#,
        "base of integer logarithm must be at least 2",
    );
}

#[test]
fn isqrt_of_negative_panics() {
    panics_with(
        "int-panic",
        r#"fn main() {
    let n = -1i32;
    println!("{}", n.isqrt());
}
"#,
        "argument of integer square root cannot be negative",
    );
}

#[test]
fn next_power_of_two_overflow_panics() {
    // rustc: "attempt to add with overflow"
    panics_with_kind(
        "int-panic",
        r#"fn main() {
    let n = 255u8;
    println!("{}", n.next_power_of_two());
}
"#,
        PanicKind::Overflow,
    );
}

#[test]
fn div_euclid_by_zero_panics() {
    // rustc: "attempt to divide by zero"
    panics_with_kind(
        "int-panic",
        r#"fn main() {
    let z = 0u32;
    println!("{}", 10u32.div_euclid(z));
}
"#,
        PanicKind::DivisionByZero,
    );
}

#[test]
fn rem_euclid_min_by_minus_one_panics() {
    // rustc: "attempt to calculate the remainder with overflow"
    panics_with_kind(
        "int-panic",
        r#"fn main() {
    let d = -1i32;
    println!("{}", i32::MIN.rem_euclid(d));
}
"#,
        PanicKind::Overflow,
    );
}
