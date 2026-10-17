// `core::char`.
use crate::option::Option;

pub const MAX: char = '\u{10FFFF}';
pub const REPLACEMENT_CHARACTER: char = '\u{FFFD}';

pub fn from_u32(i: u32) -> Option<char> {
    core::intrinsics::lrr_unsupported("core::char::from_u32")
}

pub fn from_digit(num: u32, radix: u32) -> Option<char> {
    core::intrinsics::lrr_unsupported("core::char::from_digit")
}

pub struct ParseCharError;
pub struct CharTryFromError;
