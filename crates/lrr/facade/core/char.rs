// `core::char`.
use crate::option::Option;

pub const MAX: char = '\u{10FFFF}';
pub const REPLACEMENT_CHARACTER: char = '\u{FFFD}';

pub fn from_u32(i: u32) -> Option<char> {
    if i > 0x10FFFF || (i >= 0xD800 && i <= 0xDFFF) {
        Option::None
    } else {
        Option::Some(core::intrinsics::lrr_char_from_u32(i))
    }
}

pub fn from_digit(num: u32, radix: u32) -> Option<char> {
    if radix < 2 || radix > 36 || num >= radix {
        return Option::None;
    }
    if num < 10 {
        Option::Some(core::intrinsics::lrr_char_from_u32(48 + num))
    } else {
        Option::Some(core::intrinsics::lrr_char_from_u32(87 + num))
    }
}

// Classification follows the ASCII definitions; the Unicode properties
// beyond ASCII are not modelled.
impl char {
    pub fn is_whitespace(self) -> bool {
        let c: u32 = self as u32;
        c == 32
            || (c >= 9 && c <= 13)
            || c == 0x85
            || c == 0xA0
            || c == 0x1680
            || (c >= 0x2000 && c <= 0x200A)
            || c == 0x2028
            || c == 0x2029
            || c == 0x202F
            || c == 0x205F
            || c == 0x3000
    }
    pub fn is_ascii(&self) -> bool {
        (*self as u32) < 128
    }
    pub fn is_ascii_digit(&self) -> bool {
        let c: u32 = *self as u32;
        c >= 48 && c <= 57
    }
    pub fn is_ascii_lowercase(&self) -> bool {
        let c: u32 = *self as u32;
        c >= 97 && c <= 122
    }
    pub fn is_ascii_uppercase(&self) -> bool {
        let c: u32 = *self as u32;
        c >= 65 && c <= 90
    }
    pub fn is_ascii_alphabetic(&self) -> bool {
        self.is_ascii_lowercase() || self.is_ascii_uppercase()
    }
    pub fn is_ascii_alphanumeric(&self) -> bool {
        self.is_ascii_alphabetic() || self.is_ascii_digit()
    }
    pub fn is_ascii_whitespace(&self) -> bool {
        let c: u32 = *self as u32;
        c == 32 || c == 9 || c == 10 || c == 12 || c == 13
    }
    pub fn is_alphabetic(self) -> bool {
        self.is_ascii_alphabetic()
    }
    pub fn is_numeric(self) -> bool {
        self.is_ascii_digit()
    }
    pub fn is_alphanumeric(self) -> bool {
        self.is_ascii_alphanumeric()
    }
    pub fn is_lowercase(self) -> bool {
        self.is_ascii_lowercase()
    }
    pub fn is_uppercase(self) -> bool {
        self.is_ascii_uppercase()
    }
    pub fn to_digit(self, radix: u32) -> Option<u32> {
        let c: u32 = self as u32;
        let mut value: u32 = 99;
        if c >= 48 && c <= 57 {
            value = c - 48;
        } else if c >= 97 && c <= 122 {
            value = c - 87;
        } else if c >= 65 && c <= 90 {
            value = c - 55;
        }
        if value < radix {
            Option::Some(value)
        } else {
            Option::None
        }
    }
    pub fn is_digit(self, radix: u32) -> bool {
        self.to_digit(radix).is_some()
    }
    pub fn to_ascii_uppercase(&self) -> char {
        if self.is_ascii_lowercase() {
            core::intrinsics::lrr_char_from_u32((*self as u32) - 32)
        } else {
            *self
        }
    }
    pub fn to_ascii_lowercase(&self) -> char {
        if self.is_ascii_uppercase() {
            core::intrinsics::lrr_char_from_u32((*self as u32) + 32)
        } else {
            *self
        }
    }
    pub fn len_utf8(self) -> usize {
        let c: u32 = self as u32;
        if c < 0x80 {
            1
        } else if c < 0x800 {
            2
        } else if c < 0x10000 {
            3
        } else {
            4
        }
    }
}

pub struct ParseCharError;
pub struct CharTryFromError;
