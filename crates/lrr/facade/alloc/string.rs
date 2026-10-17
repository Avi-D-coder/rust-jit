// `alloc::string`. Owned UTF-8 text is an interpreter intrinsic.
use crate::vec::Vec;
use core::fmt;
use core::ops::Deref;

pub struct String {
    vec: Vec<u8>,
}

pub struct FromUtf8Error {
    bytes: Vec<u8>,
}

impl String {
    pub const fn new() -> String {
        String { vec: Vec::new() }
    }
    pub fn with_capacity(capacity: usize) -> String {
        String::new()
    }
    pub fn from_utf8(vec: Vec<u8>) -> Result<String, FromUtf8Error> {
        core::intrinsics::lrr_unsupported("alloc::string::String::from_utf8")
    }
    pub fn as_str(&self) -> &str {
        core::intrinsics::lrr_unsupported("alloc::string::String::as_str")
    }
    pub fn push_str(&mut self, string: &str) {
        core::intrinsics::lrr_unsupported("alloc::string::String::push_str")
    }
    pub fn push(&mut self, ch: char) {
        core::intrinsics::lrr_unsupported("alloc::string::String::push")
    }
    pub fn pop(&mut self) -> Option<char> {
        core::intrinsics::lrr_unsupported("alloc::string::String::pop")
    }
    pub fn len(&self) -> usize {
        core::intrinsics::lrr_unsupported("alloc::string::String::len")
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn clear(&mut self) {
        core::intrinsics::lrr_unsupported("alloc::string::String::clear")
    }
    pub fn capacity(&self) -> usize {
        core::intrinsics::lrr_unsupported("alloc::string::String::capacity")
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.vec
    }
}

impl Deref for String {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl Clone for String {
    fn clone(&self) -> String {
        String {
            vec: self.vec.clone(),
        }
    }
}
impl Default for String {
    fn default() -> String {
        String::new()
    }
}
impl<'a> From<&'a str> for String {
    fn from(s: &'a str) -> String {
        core::intrinsics::lrr_unsupported("alloc::string::String::from")
    }
}
impl PartialEq for String {
    fn eq(&self, other: &String) -> bool {
        core::intrinsics::lrr_unsupported("alloc::string::String::eq")
    }
}
impl Eq for String {}

pub trait ToString {
    fn to_string(&self) -> String;
}

impl<T: fmt::Display + ?Sized> ToString for T {
    fn to_string(&self) -> String {
        core::intrinsics::lrr_unsupported("alloc::string::ToString::to_string")
    }
}

pub type ParseError = core::convert::Infallible;
