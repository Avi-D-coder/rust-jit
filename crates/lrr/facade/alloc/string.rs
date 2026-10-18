// `alloc::string`. Owned UTF-8 text is a `Vec<u8>` (the interpreter object of
// `alloc::vec`) whose bytes are always valid UTF-8; `as_str` views them as a
// `&str` through the closed intrinsic `lrr_vec_as_str`. Everything else is
// ordinary Rust over `Vec<u8>` and the `lrr_str_byte` byte access.
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
        let valid = core::str::from_utf8(vec.as_slice()).is_ok();
        if valid {
            Ok(String { vec: vec })
        } else {
            Err(FromUtf8Error { bytes: vec })
        }
    }
    pub fn pop(&mut self) -> Option<char> {
        let last = self.as_str().chars().next_back();
        match last {
            Some(ch) => {
                let keep: usize = self.vec.len() - ch.len_utf8();
                self.vec.truncate(keep);
                Some(ch)
            }
            None => None,
        }
    }
    pub fn insert(&mut self, index: usize, ch: char) {
        let tail = String::from(core::intrinsics::lrr_str_slice(self.as_str(), index, self.vec.len()));
        self.vec.truncate(index);
        self.push(ch);
        self.push_str(tail.as_str());
    }
    pub fn insert_str(&mut self, index: usize, string: &str) {
        let tail = String::from(core::intrinsics::lrr_str_slice(self.as_str(), index, self.vec.len()));
        self.vec.truncate(index);
        self.push_str(string);
        self.push_str(tail.as_str());
    }
    pub fn remove(&mut self, index: usize) -> char {
        let ch = match core::intrinsics::lrr_str_slice(self.as_str(), index, self.vec.len()).chars().next() {
            Some(ch) => ch,
            None => core::panicking::panic("cannot remove a char from the end of a string"),
        };
        let tail = String::from(core::intrinsics::lrr_str_slice(
            self.as_str(),
            index + ch.len_utf8(),
            self.vec.len(),
        ));
        self.vec.truncate(index);
        self.push_str(tail.as_str());
        ch
    }
    pub fn as_str(&self) -> &str {
        core::intrinsics::lrr_vec_as_str(&self.vec)
    }
    pub fn push_str(&mut self, string: &str) {
        let n: usize = core::intrinsics::lrr_str_len(string);
        let mut i: usize = 0;
        while i < n {
            self.vec.push(core::intrinsics::lrr_str_byte(string, i));
            i += 1;
        }
    }
    pub fn push(&mut self, ch: char) {
        let c: u32 = ch as u32;
        if c < 0x80 {
            self.vec.push(c as u8);
        } else if c < 0x800 {
            self.vec.push((0xC0 | (c >> 6)) as u8);
            self.vec.push((0x80 | (c & 0x3F)) as u8);
        } else if c < 0x10000 {
            self.vec.push((0xE0 | (c >> 12)) as u8);
            self.vec.push((0x80 | ((c >> 6) & 0x3F)) as u8);
            self.vec.push((0x80 | (c & 0x3F)) as u8);
        } else {
            self.vec.push((0xF0 | (c >> 18)) as u8);
            self.vec.push((0x80 | ((c >> 12) & 0x3F)) as u8);
            self.vec.push((0x80 | ((c >> 6) & 0x3F)) as u8);
            self.vec.push((0x80 | (c & 0x3F)) as u8);
        }
    }
    pub fn len(&self) -> usize {
        self.vec.len()
    }
    pub fn is_empty(&self) -> bool {
        self.vec.len() == 0
    }
    pub fn clear(&mut self) {
        self.vec.clear();
    }
    pub fn truncate(&mut self, new_len: usize) {
        self.vec.truncate(new_len);
    }
    pub fn capacity(&self) -> usize {
        self.vec.len()
    }
    pub fn reserve(&mut self, additional: usize) {}
    pub fn into_bytes(self) -> Vec<u8> {
        self.vec
    }
    pub fn as_bytes(&self) -> &[u8] {
        self.vec.as_slice()
    }
}

impl crate::borrow::ToOwned for str {
    type Owned = String;
    fn to_owned(&self) -> String {
        String::from(self)
    }
}
impl core::iter::FromIterator<char> for String {
    fn from_iter<I: IntoIterator<Item = char>>(iter: I) -> String {
        let mut out = String::new();
        let mut it = iter.into_iter();
        while let Some(ch) = it.next() {
            out.push(ch);
        }
        out
    }
}
impl core::iter::Extend<char> for String {
    fn extend<I: IntoIterator<Item = char>>(&mut self, iter: I) {
        let mut it = iter.into_iter();
        while let Some(ch) = it.next() {
            self.push(ch);
        }
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
        let mut out = String::new();
        out.push_str(s);
        out
    }
}
// Text a `String` can be compared with. One generic impl (rather than an impl
// per right operand type) lets the operator's impl selection by self type
// stay unique; the right operand binds the impl's parameter.
pub trait LrrText {
    fn lrr_text(&self) -> &str;
}
impl LrrText for String {
    fn lrr_text(&self) -> &str {
        self.as_str()
    }
}
impl LrrText for str {
    fn lrr_text(&self) -> &str {
        self
    }
}
impl<'a> LrrText for &'a str {
    fn lrr_text(&self) -> &str {
        *self
    }
}
impl<S: ?Sized + LrrText> PartialEq<S> for String {
    fn eq(&self, other: &S) -> bool {
        core::intrinsics::lrr_str_eq(self.as_str(), other.lrr_text())
    }
}
impl Eq for String {}
impl Ord for String {
    fn cmp(&self, other: &String) -> core::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}
impl PartialOrd for String {
    fn partial_cmp(&self, other: &String) -> Option<core::cmp::Ordering> {
        Option::Some(self.as_str().cmp(other.as_str()))
    }
}
impl fmt::Display for String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
impl fmt::Debug for String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}
impl core::ops::Add<&str> for String {
    type Output = String;
    fn add(mut self, other: &str) -> String {
        self.push_str(other);
        self
    }
}
impl core::ops::AddAssign<&str> for String {
    fn add_assign(&mut self, other: &str) {
        self.push_str(other);
    }
}
impl core::fmt::Write for String {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }
}

pub trait ToString {
    fn to_string(&self) -> String;
}

impl<T: fmt::Display + ?Sized> ToString for T {
    fn to_string(&self) -> String {
        let mut f = fmt::lrr_formatter();
        fmt::Display::fmt(self, &mut f);
        let text = core::intrinsics::lrr_fmt_take(f.lrr_sink());
        String::from(text)
    }
}

pub type ParseError = core::convert::Infallible;
impl core::borrow::Borrow<str> for String {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl core::hash::Hash for String {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}
