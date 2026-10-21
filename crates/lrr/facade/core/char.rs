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

// `char::escape_unicode`, `escape_default` and `escape_debug` (core/char/methods.rs).
// The escape of one character is at most ten characters (`\u{10ffff}`), held
// in a fixed buffer. Printability follows the facade's ASCII-centred
// classification: control characters, the Unicode format/separator/private-use
// ranges and non-characters are not printable; everything else is, and the
// common combining-mark blocks stand in for `Grapheme_Extend`.
pub struct CharEscape {
    buf: [char; 10],
    pos: usize,
    len: usize,
}
pub type EscapeUnicode = CharEscape;
pub type EscapeDefault = CharEscape;
pub type EscapeDebug = CharEscape;

fn lrr_escape_unicode(c: char) -> CharEscape {
    let mut buf: [char; 10] = ['\0'; 10];
    buf[0] = '\\';
    buf[1] = 'u';
    buf[2] = '{';
    let code: u32 = c as u32;
    let mut digits: usize = 1;
    let mut probe: u32 = code >> 4;
    while probe != 0 {
        digits += 1;
        probe >>= 4;
    }
    let mut i: usize = 0;
    while i < digits {
        let shift: u32 = (4 * (digits - 1 - i)) as u32;
        let nibble: u32 = (code >> shift) & 0xF;
        let digit: u32 = if nibble < 10 { 48 + nibble } else { 87 + nibble };
        buf[3 + i] = core::intrinsics::lrr_char_from_u32(digit);
        i += 1;
    }
    buf[3 + digits] = '}';
    CharEscape { buf: buf, pos: 0, len: 4 + digits }
}
fn lrr_escape_backslash(c: char) -> CharEscape {
    let mut buf: [char; 10] = ['\0'; 10];
    buf[0] = '\\';
    buf[1] = c;
    CharEscape { buf: buf, pos: 0, len: 2 }
}
fn lrr_escape_literal(c: char) -> CharEscape {
    let mut buf: [char; 10] = ['\0'; 10];
    buf[0] = c;
    CharEscape { buf: buf, pos: 0, len: 1 }
}
fn lrr_is_grapheme_extend(c: char) -> bool {
    let u: u32 = c as u32;
    (u >= 0x300 && u <= 0x36F)
        || (u >= 0x483 && u <= 0x489)
        || (u >= 0x591 && u <= 0x5BD)
        || (u >= 0x1AB0 && u <= 0x1AFF)
        || (u >= 0x1DC0 && u <= 0x1DFF)
        || (u >= 0x200C && u <= 0x200C)
        || (u >= 0x20D0 && u <= 0x20FF)
        || (u >= 0xFE00 && u <= 0xFE0F)
        || (u >= 0xFE20 && u <= 0xFE2F)
        || (u >= 0xE0100 && u <= 0xE01EF)
}
fn lrr_is_printable(c: char) -> bool {
    let u: u32 = c as u32;
    if u < 0x20 || (u >= 0x7F && u <= 0x9F) || u == 0xAD {
        return false;
    }
    !((u >= 0x200B && u <= 0x200F)
        || (u >= 0x2028 && u <= 0x202E)
        || (u >= 0x2060 && u <= 0x206F)
        || (u >= 0xE000 && u <= 0xF8FF)
        || (u >= 0xFDD0 && u <= 0xFDEF)
        || u == 0xFEFF
        || (u >= 0xFFF0 && u <= 0xFFFB)
        || u == 0xFFFE
        || u == 0xFFFF
        || (u >= 0xE0000 && u <= 0xE0FFF)
        || u >= 0xF0000)
}
// The `char::escape_debug_ext` of core: `escape_grapheme_extended` and the
// quote flags are the `EscapeDebugExtArgs` fields.
// An exhausted escape (what `str`'s escape iterators start from).
pub fn lrr_escape_empty() -> CharEscape {
    CharEscape { buf: ['\0'; 10], pos: 0, len: 0 }
}
pub fn lrr_escape_debug_ext(
    c: char,
    escape_grapheme_extended: bool,
    escape_single_quote: bool,
    escape_double_quote: bool,
) -> CharEscape {
    match c {
        '\0' => lrr_escape_backslash('0'),
        '\t' => lrr_escape_backslash('t'),
        '\r' => lrr_escape_backslash('r'),
        '\n' => lrr_escape_backslash('n'),
        '\\' => lrr_escape_backslash('\\'),
        '"' if escape_double_quote => lrr_escape_backslash('"'),
        '\'' if escape_single_quote => lrr_escape_backslash('\''),
        _ if escape_grapheme_extended && lrr_is_grapheme_extend(c) => lrr_escape_unicode(c),
        _ if lrr_is_printable(c) => lrr_escape_literal(c),
        _ => lrr_escape_unicode(c),
    }
}

impl char {
    pub fn escape_unicode(self) -> EscapeUnicode {
        lrr_escape_unicode(self)
    }
    pub fn escape_default(self) -> EscapeDefault {
        match self {
            '\t' => lrr_escape_backslash('t'),
            '\r' => lrr_escape_backslash('r'),
            '\n' => lrr_escape_backslash('n'),
            '\\' | '\'' | '"' => lrr_escape_backslash(self),
            '\x20'..='\x7e' => lrr_escape_literal(self),
            _ => lrr_escape_unicode(self),
        }
    }
    pub fn escape_debug(self) -> EscapeDebug {
        lrr_escape_debug_ext(self, true, true, true)
    }
}

impl crate::iter::Iterator for CharEscape {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        if self.pos < self.len {
            let c: char = self.buf[self.pos];
            self.pos += 1;
            Option::Some(c)
        } else {
            Option::None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining: usize = self.len - self.pos;
        (remaining, Option::Some(remaining))
    }
}
impl crate::iter::ExactSizeIterator for CharEscape {}
impl crate::iter::FusedIterator for CharEscape {}
impl crate::clone::Clone for CharEscape {
    fn clone(&self) -> CharEscape {
        CharEscape { buf: self.buf, pos: self.pos, len: self.len }
    }
}
impl crate::fmt::Display for CharEscape {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        let mut i: usize = self.pos;
        while i < self.len {
            match f.write_char(self.buf[i]) {
                crate::result::Result::Ok(()) => {}
                crate::result::Result::Err(e) => return crate::result::Result::Err(e),
            }
            i += 1;
        }
        crate::result::Result::Ok(())
    }
}
impl crate::fmt::Debug for CharEscape {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.write_str("CharEscape")
    }
}
