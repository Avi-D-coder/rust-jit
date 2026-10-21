// `core::str`. A `&str` is an immutable byte range of the interpreter's string
// pool: its length, bytes and sub-ranges are the closed `lrr_str_*` intrinsics;
// decoding, searching, splitting and trimming are ordinary Rust over them.
use crate::option::Option;
use crate::result::Result;

pub struct Utf8Error {
    valid_up_to: usize,
}
pub struct ParseBoolError;

pub struct Chars<'a> {
    text: &'a str,
    pos: usize,
    end: usize,
}
pub struct Bytes<'a> {
    text: &'a str,
    pos: usize,
    end: usize,
}
pub struct CharIndices<'a> {
    text: &'a str,
    pos: usize,
}
pub struct Split<'a, P> {
    text: &'a str,
    pattern: P,
    pos: usize,
    done: bool,
}
pub struct Lines<'a> {
    text: &'a str,
    pos: usize,
}
pub struct SplitWhitespace<'a> {
    text: &'a str,
    pos: usize,
}

// UTF-8 decoding over the closed byte access `lrr_str_byte`.
fn lrr_width_at(text: &str, pos: usize) -> usize {
    let first: u32 = core::intrinsics::lrr_str_byte(text, pos) as u32;
    if first >= 0xF0 {
        4
    } else if first >= 0xE0 {
        3
    } else if first >= 0xC0 {
        2
    } else {
        1
    }
}
fn lrr_decode_at(text: &str, pos: usize) -> char {
    let first: u32 = core::intrinsics::lrr_str_byte(text, pos) as u32;
    let width: usize = lrr_width_at(text, pos);
    let mut code: u32 = first;
    if width == 4 {
        code = first & 0x07;
    } else if width == 3 {
        code = first & 0x0F;
    } else if width == 2 {
        code = first & 0x1F;
    }
    let mut k: usize = 1;
    while k < width {
        let next: u32 = core::intrinsics::lrr_str_byte(text, pos + k) as u32;
        code = (code << 6) | (next & 0x3F);
        k += 1;
    }
    core::intrinsics::lrr_char_from_u32(code)
}
// The start of the character ending at byte `end`.
fn lrr_start_before(text: &str, end: usize) -> usize {
    let mut start: usize = end - 1;
    while start > 0 && ((core::intrinsics::lrr_str_byte(text, start) as u32) & 0xC0) == 0x80 {
        start -= 1;
    }
    start
}

impl<'a> crate::iter::Iterator for Chars<'a> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        if self.pos >= self.end {
            return Option::None;
        }
        let c: char = lrr_decode_at(self.text, self.pos);
        self.pos += c.len_utf8();
        Option::Some(c)
    }
}
impl<'a> crate::iter::DoubleEndedIterator for Chars<'a> {
    fn next_back(&mut self) -> Option<char> {
        if self.pos >= self.end {
            return Option::None;
        }
        let start: usize = lrr_start_before(self.text, self.end);
        self.end = start;
        Option::Some(lrr_decode_at(self.text, start))
    }
}
impl<'a> crate::iter::Iterator for CharIndices<'a> {
    type Item = (usize, char);
    fn next(&mut self) -> Option<(usize, char)> {
        let n: usize = core::intrinsics::lrr_str_len(self.text);
        if self.pos >= n {
            return Option::None;
        }
        let at: usize = self.pos;
        let c: char = lrr_decode_at(self.text, at);
        self.pos += c.len_utf8();
        Option::Some((at, c))
    }
}
impl<'a> crate::iter::Iterator for Bytes<'a> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        if self.pos >= self.end {
            return Option::None;
        }
        let byte: u8 = core::intrinsics::lrr_str_byte(self.text, self.pos);
        self.pos += 1;
        Option::Some(byte)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining: usize = self.end - self.pos;
        (remaining, Option::Some(remaining))
    }
}
impl<'a> crate::iter::DoubleEndedIterator for Bytes<'a> {
    fn next_back(&mut self) -> Option<u8> {
        if self.pos >= self.end {
            return Option::None;
        }
        self.end -= 1;
        Option::Some(core::intrinsics::lrr_str_byte(self.text, self.end))
    }
}
impl<'a> crate::iter::ExactSizeIterator for Bytes<'a> {}
impl<'a> crate::clone::Clone for Bytes<'a> {
    fn clone(&self) -> Bytes<'a> {
        Bytes { text: self.text, pos: self.pos, end: self.end }
    }
}
impl<'a> crate::clone::Clone for Chars<'a> {
    fn clone(&self) -> Chars<'a> {
        Chars { text: self.text, pos: self.pos, end: self.end }
    }
}
impl<'a> crate::clone::Clone for CharIndices<'a> {
    fn clone(&self) -> CharIndices<'a> {
        CharIndices { text: self.text, pos: self.pos }
    }
}
impl<'a, P: pattern::Pattern> crate::iter::Iterator for Split<'a, P> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        if self.done {
            return Option::None;
        }
        let len: usize = core::intrinsics::lrr_str_len(self.text);
        match self.pattern.lrr_find_in(self.text, self.pos) {
            Option::Some((start, end)) => {
                let piece = core::intrinsics::lrr_str_slice(self.text, self.pos, start);
                self.pos = end;
                Option::Some(piece)
            }
            Option::None => {
                self.done = true;
                Option::Some(core::intrinsics::lrr_str_slice(self.text, self.pos, len))
            }
        }
    }
}
impl<'a> crate::iter::Iterator for SplitWhitespace<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        let len: usize = core::intrinsics::lrr_str_len(self.text);
        while self.pos < len {
            let c: char = lrr_decode_at(self.text, self.pos);
            if !c.is_whitespace() {
                break;
            }
            self.pos += c.len_utf8();
        }
        if self.pos >= len {
            return Option::None;
        }
        let start: usize = self.pos;
        while self.pos < len {
            let c: char = lrr_decode_at(self.text, self.pos);
            if c.is_whitespace() {
                break;
            }
            self.pos += c.len_utf8();
        }
        Option::Some(core::intrinsics::lrr_str_slice(self.text, start, self.pos))
    }
}
impl<'a> crate::iter::Iterator for Lines<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        let len: usize = core::intrinsics::lrr_str_len(self.text);
        if self.pos >= len {
            return Option::None;
        }
        let start: usize = self.pos;
        let mut end: usize = start;
        while end < len && core::intrinsics::lrr_str_byte(self.text, end) != 10 {
            end += 1;
        }
        self.pos = if end < len { end + 1 } else { len };
        if end > start && core::intrinsics::lrr_str_byte(self.text, end - 1) == 13 {
            end -= 1;
        }
        Option::Some(core::intrinsics::lrr_str_slice(self.text, start, end))
    }
}

pub trait FromStr: crate::marker::Sized {
    type Err;
    fn from_str(s: &str) -> Result<Self, Self::Err>;
}

impl Utf8Error {
    pub fn valid_up_to(&self) -> usize {
        self.valid_up_to
    }
}
impl crate::fmt::Debug for Utf8Error {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.debug_struct("Utf8Error").field("valid_up_to", &self.valid_up_to).finish()
    }
}
impl crate::fmt::Display for Utf8Error {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.write_str("invalid utf-8 sequence")
    }
}
impl crate::clone::Clone for Utf8Error {
    fn clone(&self) -> Utf8Error {
        Utf8Error { valid_up_to: self.valid_up_to }
    }
}
impl crate::cmp::PartialEq for Utf8Error {
    fn eq(&self, other: &Utf8Error) -> bool {
        self.valid_up_to == other.valid_up_to
    }
}

// Whether `v[at]` is a UTF-8 continuation byte in `lo..=hi`.
fn lrr_cont(v: &[u8], at: usize, lo: u8, hi: u8) -> bool {
    at < v.len() && v[at] >= lo && v[at] <= hi
}
// The length of the valid UTF-8 prefix of `v`.
fn lrr_utf8_prefix(v: &[u8]) -> usize {
    let n: usize = v.len();
    let mut i: usize = 0;
    while i < n {
        let b: u8 = v[i];
        if b < 0x80 {
            i += 1;
        } else if b >= 0xC2 && b <= 0xDF {
            if !lrr_cont(v, i + 1, 0x80, 0xBF) {
                return i;
            }
            i += 2;
        } else if b >= 0xE0 && b <= 0xEF {
            let mut lo: u8 = 0x80;
            let mut hi: u8 = 0xBF;
            if b == 0xE0 {
                lo = 0xA0;
            }
            if b == 0xED {
                hi = 0x9F;
            }
            if !lrr_cont(v, i + 1, lo, hi) || !lrr_cont(v, i + 2, 0x80, 0xBF) {
                return i;
            }
            i += 3;
        } else if b >= 0xF0 && b <= 0xF4 {
            let mut lo: u8 = 0x80;
            let mut hi: u8 = 0xBF;
            if b == 0xF0 {
                lo = 0x90;
            }
            if b == 0xF4 {
                hi = 0x8F;
            }
            if !lrr_cont(v, i + 1, lo, hi)
                || !lrr_cont(v, i + 2, 0x80, 0xBF)
                || !lrr_cont(v, i + 3, 0x80, 0xBF)
            {
                return i;
            }
            i += 4;
        } else {
            return i;
        }
    }
    n
}

pub fn from_utf8(v: &[u8]) -> Result<&str, Utf8Error> {
    let valid: usize = lrr_utf8_prefix(v);
    if valid == v.len() {
        Result::Ok(core::intrinsics::lrr_bytes_as_str(v))
    } else {
        Result::Err(Utf8Error { valid_up_to: valid })
    }
}

pub mod pattern {
    use crate::option::Option;
    // A string pattern (`char` or `&str`): where it first matches, and
    // whether it is a prefix or suffix. Positions are byte offsets.
    pub trait Pattern: crate::marker::Sized {
        fn lrr_find_in(&self, haystack: &str, from: usize) -> Option<(usize, usize)>;
        fn lrr_prefix_len(&self, haystack: &str) -> Option<usize>;
        fn lrr_suffix_len(&self, haystack: &str) -> Option<usize>;
    }
    // `needle` occurs in `haystack` at `pos`.
    fn lrr_str_at(haystack: &str, pos: usize, needle: &str) -> bool {
        let n: usize = core::intrinsics::lrr_str_len(needle);
        if pos + n > core::intrinsics::lrr_str_len(haystack) {
            return false;
        }
        let mut k: usize = 0;
        while k < n {
            if core::intrinsics::lrr_str_byte(haystack, pos + k)
                != core::intrinsics::lrr_str_byte(needle, k)
            {
                return false;
            }
            k += 1;
        }
        true
    }
    impl<'b> Pattern for &'b str {
        fn lrr_find_in(&self, haystack: &str, from: usize) -> Option<(usize, usize)> {
            let n: usize = core::intrinsics::lrr_str_len(*self);
            let len: usize = core::intrinsics::lrr_str_len(haystack);
            let mut pos: usize = from;
            while pos + n <= len {
                if lrr_str_at(haystack, pos, *self) {
                    return Option::Some((pos, pos + n));
                }
                pos += 1;
            }
            Option::None
        }
        fn lrr_prefix_len(&self, haystack: &str) -> Option<usize> {
            if lrr_str_at(haystack, 0, *self) {
                Option::Some(core::intrinsics::lrr_str_len(*self))
            } else {
                Option::None
            }
        }
        fn lrr_suffix_len(&self, haystack: &str) -> Option<usize> {
            let n: usize = core::intrinsics::lrr_str_len(*self);
            let len: usize = core::intrinsics::lrr_str_len(haystack);
            if n <= len && lrr_str_at(haystack, len - n, *self) {
                Option::Some(n)
            } else {
                Option::None
            }
        }
    }
    impl Pattern for char {
        fn lrr_find_in(&self, haystack: &str, from: usize) -> Option<(usize, usize)> {
            let len: usize = core::intrinsics::lrr_str_len(haystack);
            let mut pos: usize = from;
            while pos < len {
                let c: char = super::lrr_decode_at(haystack, pos);
                let width: usize = c.len_utf8();
                if c == *self {
                    return Option::Some((pos, pos + width));
                }
                pos += width;
            }
            Option::None
        }
        fn lrr_prefix_len(&self, haystack: &str) -> Option<usize> {
            if core::intrinsics::lrr_str_len(haystack) > 0
                && super::lrr_decode_at(haystack, 0) == *self
            {
                Option::Some(self.len_utf8())
            } else {
                Option::None
            }
        }
        fn lrr_suffix_len(&self, haystack: &str) -> Option<usize> {
            let len: usize = core::intrinsics::lrr_str_len(haystack);
            if len == 0 {
                return Option::None;
            }
            let start: usize = super::lrr_start_before(haystack, len);
            if super::lrr_decode_at(haystack, start) == *self {
                Option::Some(len - start)
            } else {
                Option::None
            }
        }
    }
}

impl str {
    pub const fn len(&self) -> usize {
        core::intrinsics::lrr_str_len(self)
    }
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn chars(&self) -> Chars<'_> {
        Chars { text: self, pos: 0, end: core::intrinsics::lrr_str_len(self) }
    }
    pub fn bytes(&self) -> Bytes<'_> {
        Bytes { text: self, pos: 0, end: core::intrinsics::lrr_str_len(self) }
    }
    pub fn char_indices(&self) -> CharIndices<'_> {
        CharIndices { text: self, pos: 0 }
    }
    pub fn lines(&self) -> Lines<'_> {
        Lines { text: self, pos: 0 }
    }
    pub fn split_whitespace(&self) -> SplitWhitespace<'_> {
        SplitWhitespace { text: self, pos: 0 }
    }
    pub fn split<P: pattern::Pattern>(&self, pat: P) -> Split<'_, P> {
        Split { text: self, pattern: pat, pos: 0, done: false }
    }
    pub fn starts_with<P: pattern::Pattern>(&self, pat: P) -> bool {
        pat.lrr_prefix_len(self).is_some()
    }
    pub fn ends_with<P: pattern::Pattern>(&self, pat: P) -> bool {
        pat.lrr_suffix_len(self).is_some()
    }
    pub fn contains<P: pattern::Pattern>(&self, pat: P) -> bool {
        pat.lrr_find_in(self, 0).is_some()
    }
    pub fn find<P: pattern::Pattern>(&self, pat: P) -> Option<usize> {
        match pat.lrr_find_in(self, 0) {
            Option::Some((start, _)) => Option::Some(start),
            Option::None => Option::None,
        }
    }
    pub fn trim(&self) -> &str {
        self.trim_start().trim_end()
    }
    pub fn trim_start(&self) -> &str {
        let len: usize = self.len();
        let mut pos: usize = 0;
        while pos < len {
            let c: char = lrr_decode_at(self, pos);
            if !c.is_whitespace() {
                break;
            }
            pos += c.len_utf8();
        }
        core::intrinsics::lrr_str_slice(self, pos, len)
    }
    pub fn trim_end(&self) -> &str {
        let mut end: usize = self.len();
        while end > 0 {
            let start: usize = lrr_start_before(self, end);
            let c: char = lrr_decode_at(self, start);
            if !c.is_whitespace() {
                break;
            }
            end = start;
        }
        core::intrinsics::lrr_str_slice(self, 0, end)
    }
    pub fn trim_start_matches<P: pattern::Pattern>(&self, pat: P) -> &str {
        let len: usize = self.len();
        let mut pos: usize = 0;
        loop {
            let rest = core::intrinsics::lrr_str_slice(self, pos, len);
            match pat.lrr_prefix_len(rest) {
                Option::Some(n) if n > 0 => pos += n,
                _ => break,
            }
        }
        core::intrinsics::lrr_str_slice(self, pos, len)
    }
    pub fn trim_end_matches<P: pattern::Pattern>(&self, pat: P) -> &str {
        let mut end: usize = self.len();
        loop {
            let head = core::intrinsics::lrr_str_slice(self, 0, end);
            match pat.lrr_suffix_len(head) {
                Option::Some(n) if n > 0 => end -= n,
                _ => break,
            }
        }
        core::intrinsics::lrr_str_slice(self, 0, end)
    }
    pub fn trim_matches<P: pattern::Pattern>(&self, pat: P) -> &str {
        let len: usize = self.len();
        let mut pos: usize = 0;
        loop {
            let rest = core::intrinsics::lrr_str_slice(self, pos, len);
            match pat.lrr_prefix_len(rest) {
                Option::Some(n) if n > 0 => pos += n,
                _ => break,
            }
        }
        let mut end: usize = len;
        while end > pos {
            let head = core::intrinsics::lrr_str_slice(self, pos, end);
            match pat.lrr_suffix_len(head) {
                Option::Some(n) if n > 0 => end -= n,
                _ => break,
            }
        }
        core::intrinsics::lrr_str_slice(self, pos, end)
    }
    pub fn is_char_boundary(&self, index: usize) -> bool {
        let len: usize = self.len();
        if index == 0 || index == len {
            return true;
        }
        if index > len {
            return false;
        }
        ((core::intrinsics::lrr_str_byte(self, index) as u32) & 0xC0) != 0x80
    }
    pub fn parse<F: FromStr>(&self) -> Result<F, <F as FromStr>::Err> {
        <F as FromStr>::from_str(self)
    }
}

impl crate::cmp::PartialEq for str {
    fn eq(&self, other: &str) -> bool {
        core::intrinsics::lrr_str_eq(self, other)
    }
}
impl crate::cmp::Eq for str {}
impl crate::cmp::Ord for str {
    fn cmp(&self, other: &str) -> crate::cmp::Ordering {
        let (a, b): (usize, usize) = (self.len(), other.len());
        let common: usize = if a < b { a } else { b };
        let mut i: usize = 0;
        while i < common {
            let x: u8 = core::intrinsics::lrr_str_byte(self, i);
            let y: u8 = core::intrinsics::lrr_str_byte(other, i);
            if x != y {
                return if x < y { crate::cmp::Ordering::Less } else { crate::cmp::Ordering::Greater };
            }
            i += 1;
        }
        a.cmp(&b)
    }
}
impl crate::cmp::PartialOrd for str {
    fn partial_cmp(&self, other: &str) -> Option<crate::cmp::Ordering> {
        Option::Some(self.cmp(other))
    }
}

// `str::escape_debug`, `escape_default` and `escape_unicode`: each character's
// escape in turn. `escape_debug` escapes a leading combining mark (so the
// mark cannot attach to a preceding character) but not later ones.
pub struct StrEscape<'a> {
    chars: Chars<'a>,
    mode: u8,
    first: bool,
    current: crate::char::CharEscape,
}
pub type EscapeUnicode<'a> = StrEscape<'a>;
pub type EscapeDefault<'a> = StrEscape<'a>;
pub type EscapeDebug<'a> = StrEscape<'a>;

impl<'a> crate::iter::Iterator for StrEscape<'a> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        loop {
            if let Option::Some(c) = self.current.next() {
                return Option::Some(c);
            }
            match self.chars.next() {
                Option::Some(ch) => {
                    self.current = if self.mode == 0 {
                        ch.escape_unicode()
                    } else if self.mode == 1 {
                        ch.escape_default()
                    } else {
                        let escape_grapheme_extended: bool = self.first;
                        crate::char::lrr_escape_debug_ext(ch, escape_grapheme_extended, true, true)
                    };
                    self.first = false;
                }
                Option::None => return Option::None,
            }
        }
    }
}
impl<'a> crate::iter::FusedIterator for StrEscape<'a> {}
impl<'a> crate::clone::Clone for StrEscape<'a> {
    fn clone(&self) -> StrEscape<'a> {
        StrEscape {
            chars: self.chars.clone(),
            mode: self.mode,
            first: self.first,
            current: self.current.clone(),
        }
    }
}
impl<'a> crate::fmt::Display for StrEscape<'a> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        let mut rest: StrEscape<'a> = self.clone();
        while let Option::Some(c) = rest.next() {
            match f.write_char(c) {
                Result::Ok(()) => {}
                Result::Err(e) => return Result::Err(e),
            }
        }
        Result::Ok(())
    }
}
impl<'a> crate::fmt::Debug for StrEscape<'a> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.write_str("StrEscape")
    }
}

impl str {
    pub fn escape_unicode(&self) -> EscapeUnicode<'_> {
        StrEscape { chars: self.chars(), mode: 0, first: true, current: crate::char::lrr_escape_empty() }
    }
    pub fn escape_default(&self) -> EscapeDefault<'_> {
        StrEscape { chars: self.chars(), mode: 1, first: true, current: crate::char::lrr_escape_empty() }
    }
    pub fn escape_debug(&self) -> EscapeDebug<'_> {
        StrEscape { chars: self.chars(), mode: 2, first: true, current: crate::char::lrr_escape_empty() }
    }
}
