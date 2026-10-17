// `core::fmt`. Formatting itself runs in the interpreter's formatting
// machine; these are the trait, type and builder identities it selects.
use crate::marker::Sized;
use crate::option::Option;

// The error type returned by formatting traits.
pub struct Error;

impl crate::clone::Clone for Error {
    fn clone(&self) -> Error {
        Error
    }
}
impl crate::marker::Copy for Error {}
impl crate::cmp::PartialEq for Error {
    fn eq(&self, _other: &Error) -> bool {
        true
    }
}
impl crate::cmp::Eq for Error {}
impl crate::default::Default for Error {
    fn default() -> Error {
        Error
    }
}

pub type Result = crate::result::Result<(), Error>;

pub enum Alignment {
    Left,
    Right,
    Center,
}

pub enum Sign {
    Plus,
    Minus,
}

pub enum DebugAsHex {
    Lower,
    Upper,
}

// Configuration and output sink for one formatting request. The output is
// an interpreter buffer (`sink`); the placeholder's options are plain data:
// `spec` bits 1 `+`, 2 `-`, 4 `#`, 8 `0`, 16 width set, 32 precision set,
// 64 `x?`, 128 `X?`, bits 8..10 the alignment (0 unknown, 1 left, 2 right,
// 3 center). `'a` is the sink's lifetime in the library's own signature.
pub struct Formatter<'a> {
    sink: usize,
    spec: u64,
    fill: char,
    width: usize,
    precision: usize,
}

// `format_args!` renders its template eagerly in the interpreter, so a
// precompiled `Arguments` is its rendered text.
pub type Arguments<'a> = &'a str;

pub trait Write {
    fn write_str(&mut self, s: &str) -> Result;
    fn write_char(&mut self, c: char) -> Result {
        core::intrinsics::lrr_unsupported("core::fmt::Write::write_char")
    }
    fn write_fmt(&mut self, args: Arguments<'_>) -> Result {
        self.write_str(args)
    }
}

pub trait Display {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}
pub trait Debug {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}
pub trait Octal {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}
pub trait Binary {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}
pub trait LowerHex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}
pub trait UpperHex {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}
pub trait Pointer {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}
pub trait LowerExp {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}
pub trait UpperExp {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result;
}

pub struct DebugStruct<'a, 'b: 'a> {
    fmt: &'a mut Formatter<'b>,
    result: Result,
    has_fields: bool,
}
pub struct DebugTuple<'a, 'b: 'a> {
    fmt: &'a mut Formatter<'b>,
    result: Result,
    fields: usize,
    empty_name: bool,
}
pub struct DebugList<'a, 'b: 'a> {
    fmt: &'a mut Formatter<'b>,
    result: Result,
    has_fields: bool,
}
pub struct DebugSet<'a, 'b: 'a> {
    fmt: &'a mut Formatter<'b>,
    result: Result,
    has_fields: bool,
}
pub struct DebugMap<'a, 'b: 'a> {
    fmt: &'a mut Formatter<'b>,
    result: Result,
    has_fields: bool,
}

impl<'a> Formatter<'a> {
    pub fn write_str(&mut self, data: &str) -> Result {
        core::intrinsics::lrr_fmt_write(self.sink, data);
        crate::result::Result::Ok(())
    }
    pub fn write_fmt(&mut self, fmt: Arguments<'_>) -> Result {
        core::intrinsics::lrr_fmt_write(self.sink, fmt);
        crate::result::Result::Ok(())
    }
    pub fn write_char(&mut self, c: char) -> Result {
        core::intrinsics::lrr_fmt_value(&c, 0, self.sink, 0, ' ', 0, 0);
        crate::result::Result::Ok(())
    }
    pub fn pad(&mut self, s: &str) -> Result {
        core::intrinsics::lrr_fmt_value(s, 0, self.sink, self.spec, self.fill, self.width, self.precision);
        crate::result::Result::Ok(())
    }
    pub fn pad_integral(&mut self, is_nonnegative: bool, prefix: &str, buf: &str) -> Result {
        core::intrinsics::lrr_unsupported("core::fmt::Formatter::pad_integral")
    }
    pub fn alternate(&self) -> bool {
        self.spec & 4 != 0
    }
    pub fn width(&self) -> Option<usize> {
        if self.spec & 16 != 0 { Option::Some(self.width) } else { Option::None }
    }
    pub fn precision(&self) -> Option<usize> {
        if self.spec & 32 != 0 { Option::Some(self.precision) } else { Option::None }
    }
    pub fn fill(&self) -> char {
        self.fill
    }
    pub fn align(&self) -> Option<Alignment> {
        match (self.spec >> 8) & 3 {
            1 => Option::Some(Alignment::Left),
            2 => Option::Some(Alignment::Right),
            3 => Option::Some(Alignment::Center),
            _ => Option::None,
        }
    }
    pub fn sign_plus(&self) -> bool {
        self.spec & 1 != 0
    }
    pub fn sign_minus(&self) -> bool {
        self.spec & 2 != 0
    }
    pub fn sign_aware_zero_pad(&self) -> bool {
        self.spec & 8 != 0
    }
    pub fn debug_struct<'b>(&'b mut self, name: &str) -> DebugStruct<'b, 'a> {
        let result = self.write_str(name);
        DebugStruct { fmt: self, result: result, has_fields: false }
    }
    pub fn debug_tuple<'b>(&'b mut self, name: &str) -> DebugTuple<'b, 'a> {
        let result = self.write_str(name);
        let empty_name = name.len() == 0;
        DebugTuple { fmt: self, result: result, fields: 0, empty_name: empty_name }
    }
    pub fn debug_list<'b>(&'b mut self) -> DebugList<'b, 'a> {
        let result = self.write_str("[");
        DebugList { fmt: self, result: result, has_fields: false }
    }
    pub fn debug_set<'b>(&'b mut self) -> DebugSet<'b, 'a> {
        let result = self.write_str("{");
        DebugSet { fmt: self, result: result, has_fields: false }
    }
    pub fn debug_map<'b>(&'b mut self) -> DebugMap<'b, 'a> {
        let result = self.write_str("{");
        DebugMap { fmt: self, result: result, has_fields: false }
    }
    // A formatter with this one's options over a fresh buffer, for the
    // `{:#?}` PadAdapter.
    fn lrr_nested(&self) -> Formatter<'a> {
        Formatter {
            sink: core::intrinsics::lrr_fmt_sink(),
            spec: self.spec,
            fill: self.fill,
            width: self.width,
            precision: self.precision,
        }
    }
    // Append the nested buffer through core's PadAdapter: every line it
    // starts is indented by four spaces.
    fn lrr_pad_adapter(&mut self, nested: Formatter<'a>) {
        core::intrinsics::lrr_fmt_pad_adapter(self.sink, nested.sink);
    }
}

impl<'a, 'b: 'a> DebugStruct<'a, 'b> {
    pub fn field<T: ?Sized + Debug>(&mut self, name: &str, value: &T) -> &mut Self {
        if let crate::result::Result::Ok(()) = self.result {
            if self.fmt.alternate() {
                if !self.has_fields {
                    self.fmt.write_str(" {\n");
                }
                let mut writer = self.fmt.lrr_nested();
                writer.write_str(name);
                writer.write_str(": ");
                self.result = Debug::fmt(value, &mut writer);
                writer.write_str(",\n");
                self.fmt.lrr_pad_adapter(writer);
            } else {
                if self.has_fields {
                    self.fmt.write_str(", ");
                } else {
                    self.fmt.write_str(" { ");
                }
                self.fmt.write_str(name);
                self.fmt.write_str(": ");
                self.result = Debug::fmt(value, self.fmt);
            }
        }
        self.has_fields = true;
        self
    }
    pub fn finish_non_exhaustive(&mut self) -> Result {
        if let crate::result::Result::Ok(()) = self.result {
            if self.has_fields {
                if self.fmt.alternate() {
                    let mut writer = self.fmt.lrr_nested();
                    writer.write_str("..\n");
                    self.fmt.lrr_pad_adapter(writer);
                    self.fmt.write_str("}");
                } else {
                    self.fmt.write_str(", .. }");
                }
            } else {
                self.fmt.write_str(" { .. }");
            }
        }
        self.result
    }
    pub fn finish(&mut self) -> Result {
        if self.has_fields {
            if let crate::result::Result::Ok(()) = self.result {
                if self.fmt.alternate() {
                    self.fmt.write_str("}");
                } else {
                    self.fmt.write_str(" }");
                }
            }
        }
        self.result
    }
}
impl<'a, 'b: 'a> DebugTuple<'a, 'b> {
    pub fn field<T: ?Sized + Debug>(&mut self, value: &T) -> &mut Self {
        if let crate::result::Result::Ok(()) = self.result {
            if self.fmt.alternate() {
                if self.fields == 0 {
                    self.fmt.write_str("(\n");
                }
                let mut writer = self.fmt.lrr_nested();
                self.result = Debug::fmt(value, &mut writer);
                writer.write_str(",\n");
                self.fmt.lrr_pad_adapter(writer);
            } else {
                if self.fields == 0 {
                    self.fmt.write_str("(");
                } else {
                    self.fmt.write_str(", ");
                }
                self.result = Debug::fmt(value, self.fmt);
            }
        }
        self.fields += 1;
        self
    }
    pub fn finish(&mut self) -> Result {
        if self.fields > 0 {
            if let crate::result::Result::Ok(()) = self.result {
                if self.fields == 1 && self.empty_name && !self.fmt.alternate() {
                    self.fmt.write_str(",");
                }
                self.fmt.write_str(")");
            }
        }
        self.result
    }
}
impl<'a, 'b: 'a> DebugList<'a, 'b> {
    pub fn entry<T: ?Sized + Debug>(&mut self, entry: &T) -> &mut Self {
        if let crate::result::Result::Ok(()) = self.result {
            if self.fmt.alternate() {
                if !self.has_fields {
                    self.fmt.write_str("\n");
                }
                let mut writer = self.fmt.lrr_nested();
                self.result = Debug::fmt(entry, &mut writer);
                writer.write_str(",\n");
                self.fmt.lrr_pad_adapter(writer);
            } else {
                if self.has_fields {
                    self.fmt.write_str(", ");
                }
                self.result = Debug::fmt(entry, self.fmt);
            }
        }
        self.has_fields = true;
        self
    }
    pub fn finish(&mut self) -> Result {
        if let crate::result::Result::Ok(()) = self.result {
            self.fmt.write_str("]");
        }
        self.result
    }
}
impl<'a, 'b: 'a> DebugSet<'a, 'b> {
    pub fn entry<T: ?Sized + Debug>(&mut self, entry: &T) -> &mut Self {
        if let crate::result::Result::Ok(()) = self.result {
            if self.fmt.alternate() {
                if !self.has_fields {
                    self.fmt.write_str("\n");
                }
                let mut writer = self.fmt.lrr_nested();
                self.result = Debug::fmt(entry, &mut writer);
                writer.write_str(",\n");
                self.fmt.lrr_pad_adapter(writer);
            } else {
                if self.has_fields {
                    self.fmt.write_str(", ");
                }
                self.result = Debug::fmt(entry, self.fmt);
            }
        }
        self.has_fields = true;
        self
    }
    pub fn finish(&mut self) -> Result {
        if let crate::result::Result::Ok(()) = self.result {
            self.fmt.write_str("}");
        }
        self.result
    }
}
impl<'a, 'b: 'a> DebugMap<'a, 'b> {
    pub fn entry<K: ?Sized + Debug, V: ?Sized + Debug>(&mut self, key: &K, value: &V) -> &mut Self {
        if let crate::result::Result::Ok(()) = self.result {
            if self.fmt.alternate() {
                if !self.has_fields {
                    self.fmt.write_str("\n");
                }
                let mut writer = self.fmt.lrr_nested();
                self.result = Debug::fmt(key, &mut writer);
                writer.write_str(": ");
                if let crate::result::Result::Ok(()) = self.result {
                    self.result = Debug::fmt(value, &mut writer);
                }
                writer.write_str(",\n");
                self.fmt.lrr_pad_adapter(writer);
            } else {
                if self.has_fields {
                    self.fmt.write_str(", ");
                }
                self.result = Debug::fmt(key, self.fmt);
                self.fmt.write_str(": ");
                if let crate::result::Result::Ok(()) = self.result {
                    self.result = Debug::fmt(value, self.fmt);
                }
            }
        }
        self.has_fields = true;
        self
    }
    pub fn finish(&mut self) -> Result {
        if let crate::result::Result::Ok(()) = self.result {
            self.fmt.write_str("}");
        }
        self.result
    }
}

pub fn write(output: &mut dyn Write, args: Arguments<'_>) -> Result {
    output.write_str(args)
}

// The closed primitive family formats natively, under the formatter's options.
impl Display for i8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for i8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for i8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for i8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for i8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for i8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for i8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for i8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for i16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for i16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for i16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for i16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for i16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for i16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for i16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for i16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for i32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for i32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for i32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for i32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for i32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for i32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for i32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for i32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for i64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for i64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for i64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for i64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for i64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for i64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for i64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for i64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for i128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for i128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for i128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for i128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for i128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for i128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for i128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for i128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for isize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for isize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for isize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for isize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for isize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for isize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for isize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for isize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for u8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for u8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for u8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for u8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for u8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for u8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for u8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for u8 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for u16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for u16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for u16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for u16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for u16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for u16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for u16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for u16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for u32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for u32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for u32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for u32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for u32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for u32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for u32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for u32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for u64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for u64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for u64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for u64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for u64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for u64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for u64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for u64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for u128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for u128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for u128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for u128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for u128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for u128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for u128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for u128 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for usize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for usize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerHex for usize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 4, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperHex for usize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 5, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Octal for usize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 6, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Binary for usize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 7, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for usize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for usize {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for f32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for f32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for f32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for f32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for f64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for f64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl LowerExp for f64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 8, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl UpperExp for f64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 9, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for bool {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for bool {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for char {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for char {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Display for str {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 0, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for str {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}
impl Debug for () {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        core::intrinsics::lrr_fmt_value(self, 1, f.sink, f.spec, f.fill, f.width, f.precision);
        crate::result::Result::Ok(())
    }
}

// Interpreter entries: a `format_args!` placeholder whose argument is not a
// natively rendered value calls its trait's `fmt` with a fresh formatter.
#[doc(hidden)]
pub fn lrr_format_display<T: ?Sized + Display>(
    value: &T,
    sink: usize,
    spec: u64,
    fill: char,
    width: usize,
    precision: usize,
) -> Result {
    let mut f = Formatter { sink: sink, spec: spec, fill: fill, width: width, precision: precision };
    Display::fmt(value, &mut f)
}
#[doc(hidden)]
pub fn lrr_format_debug<T: ?Sized + Debug>(
    value: &T,
    sink: usize,
    spec: u64,
    fill: char,
    width: usize,
    precision: usize,
) -> Result {
    let mut f = Formatter { sink: sink, spec: spec, fill: fill, width: width, precision: precision };
    Debug::fmt(value, &mut f)
}
#[doc(hidden)]
pub fn lrr_format_lower_hex<T: ?Sized + LowerHex>(
    value: &T,
    sink: usize,
    spec: u64,
    fill: char,
    width: usize,
    precision: usize,
) -> Result {
    let mut f = Formatter { sink: sink, spec: spec, fill: fill, width: width, precision: precision };
    LowerHex::fmt(value, &mut f)
}
#[doc(hidden)]
pub fn lrr_format_upper_hex<T: ?Sized + UpperHex>(
    value: &T,
    sink: usize,
    spec: u64,
    fill: char,
    width: usize,
    precision: usize,
) -> Result {
    let mut f = Formatter { sink: sink, spec: spec, fill: fill, width: width, precision: precision };
    UpperHex::fmt(value, &mut f)
}
#[doc(hidden)]
pub fn lrr_format_octal<T: ?Sized + Octal>(
    value: &T,
    sink: usize,
    spec: u64,
    fill: char,
    width: usize,
    precision: usize,
) -> Result {
    let mut f = Formatter { sink: sink, spec: spec, fill: fill, width: width, precision: precision };
    Octal::fmt(value, &mut f)
}
#[doc(hidden)]
pub fn lrr_format_binary<T: ?Sized + Binary>(
    value: &T,
    sink: usize,
    spec: u64,
    fill: char,
    width: usize,
    precision: usize,
) -> Result {
    let mut f = Formatter { sink: sink, spec: spec, fill: fill, width: width, precision: precision };
    Binary::fmt(value, &mut f)
}
#[doc(hidden)]
pub fn lrr_format_lower_exp<T: ?Sized + LowerExp>(
    value: &T,
    sink: usize,
    spec: u64,
    fill: char,
    width: usize,
    precision: usize,
) -> Result {
    let mut f = Formatter { sink: sink, spec: spec, fill: fill, width: width, precision: precision };
    LowerExp::fmt(value, &mut f)
}
#[doc(hidden)]
pub fn lrr_format_upper_exp<T: ?Sized + UpperExp>(
    value: &T,
    sink: usize,
    spec: u64,
    fill: char,
    width: usize,
    precision: usize,
) -> Result {
    let mut f = Formatter { sink: sink, spec: spec, fill: fill, width: width, precision: precision };
    UpperExp::fmt(value, &mut f)
}
#[doc(hidden)]
pub fn lrr_format_pointer<T: ?Sized + Pointer>(
    value: &T,
    sink: usize,
    spec: u64,
    fill: char,
    width: usize,
    precision: usize,
) -> Result {
    let mut f = Formatter { sink: sink, spec: spec, fill: fill, width: width, precision: precision };
    Pointer::fmt(value, &mut f)
}

impl<T: ?Sized + Debug> Debug for &T {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(&**self, f)
    }
}
impl<T: ?Sized + Debug> Debug for &mut T {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(&**self, f)
    }
}
impl<T: ?Sized + Display> Display for &T {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Display::fmt(&**self, f)
    }
}
impl<T: ?Sized + Display> Display for &mut T {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Display::fmt(&**self, f)
    }
}
