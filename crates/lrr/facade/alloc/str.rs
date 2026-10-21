// `alloc::str`: the `str` methods that build a `String`.
pub use core::str::{
    from_utf8, Bytes, CharIndices, Chars, FromStr, Lines, Split, SplitWhitespace, Utf8Error,
};

use crate::string::String;

impl str {
    // The bytes of a `&str` as a byte slice. A `&str` is a range of the
    // interpreter's immutable string pool and a `&[u8]` is a window of byte
    // cells, so the slice is a copy held for the rest of the run (like the
    // string pool itself); the text is immutable, so the copy is never
    // observably different from the original.
    pub fn as_bytes(&self) -> &[u8] {
        let n: usize = core::intrinsics::lrr_str_len(self);
        let mut bytes: crate::vec::Vec<u8> = crate::vec::Vec::new();
        let mut i: usize = 0;
        while i < n {
            bytes.push(core::intrinsics::lrr_str_byte(self, i));
            i += 1;
        }
        let kept = core::intrinsics::lrr_box_leak(crate::boxed::Box::new(bytes));
        core::intrinsics::lrr_vec_as_slice(kept)
    }
    pub fn to_uppercase(&self) -> String {
        let mut out = String::new();
        let mut it = self.chars();
        while let Some(c) = it.next() {
            out.push(c.to_ascii_uppercase());
        }
        out
    }
    pub fn to_lowercase(&self) -> String {
        let mut out = String::new();
        let mut it = self.chars();
        while let Some(c) = it.next() {
            out.push(c.to_ascii_lowercase());
        }
        out
    }
    pub fn to_ascii_uppercase(&self) -> String {
        self.to_uppercase()
    }
    pub fn to_ascii_lowercase(&self) -> String {
        self.to_lowercase()
    }
    pub fn repeat(&self, n: usize) -> String {
        let mut out = String::new();
        let mut i: usize = 0;
        while i < n {
            out.push_str(self);
            i += 1;
        }
        out
    }
    pub fn replace<P: core::str::pattern::Pattern>(&self, from: P, to: &str) -> String {
        let mut out = String::new();
        let len: usize = self.len();
        let mut last: usize = 0;
        let mut pos: usize = 0;
        while pos <= len {
            match from.lrr_find_in(self, pos) {
                Some((start, end)) => {
                    out.push_str(core::intrinsics::lrr_str_slice(self, last, start));
                    out.push_str(to);
                    last = end;
                    pos = if end > start { end } else { end + 1 };
                }
                None => break,
            }
        }
        out.push_str(core::intrinsics::lrr_str_slice(self, last, len));
        out
    }
}
