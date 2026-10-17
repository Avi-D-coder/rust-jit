// `core::str`. String slice operations are interpreter intrinsics.
use crate::option::Option;
use crate::result::Result;

pub struct Utf8Error {
    valid_up_to: usize,
}
pub struct ParseBoolError;

pub struct Chars<'a> {
    text: &'a str,
}
pub struct Bytes<'a> {
    text: &'a str,
}
pub struct CharIndices<'a> {
    text: &'a str,
}
pub struct Split<'a, P> {
    text: &'a str,
    pattern: P,
}
pub struct Lines<'a> {
    text: &'a str,
}
pub struct SplitWhitespace<'a> {
    text: &'a str,
}

impl<'a> crate::iter::Iterator for Chars<'a> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        core::intrinsics::lrr_unsupported("core::str::Chars::next")
    }
}
impl<'a> crate::iter::Iterator for Bytes<'a> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        core::intrinsics::lrr_unsupported("core::str::Bytes::next")
    }
}

pub trait FromStr: crate::marker::Sized {
    type Err;
    fn from_str(s: &str) -> Result<Self, Self::Err>;
}

pub fn from_utf8(v: &[u8]) -> Result<&str, Utf8Error> {
    core::intrinsics::lrr_unsupported("core::str::from_utf8")
}

pub mod pattern {
    pub trait Pattern {}
}

impl str {
    pub const fn len(&self) -> usize {
        core::intrinsics::lrr_str_len(self)
    }
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl crate::cmp::PartialEq for str {
    fn eq(&self, other: &str) -> bool {
        core::intrinsics::lrr_str_eq(self, other)
    }
}
impl crate::cmp::Eq for str {}
