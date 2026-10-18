// `core::num`.
pub struct ParseIntError {
    kind: IntErrorKind,
}
pub enum IntErrorKind {
    Empty,
    InvalidDigit,
    PosOverflow,
    NegOverflow,
    Zero,
}
impl ParseIntError {
    pub fn kind(&self) -> &IntErrorKind {
        &self.kind
    }
}
impl crate::clone::Clone for IntErrorKind {
    fn clone(&self) -> IntErrorKind {
        match self {
            IntErrorKind::Empty => IntErrorKind::Empty,
            IntErrorKind::InvalidDigit => IntErrorKind::InvalidDigit,
            IntErrorKind::PosOverflow => IntErrorKind::PosOverflow,
            IntErrorKind::NegOverflow => IntErrorKind::NegOverflow,
            IntErrorKind::Zero => IntErrorKind::Zero,
        }
    }
}
impl crate::cmp::PartialEq for IntErrorKind {
    fn eq(&self, other: &IntErrorKind) -> bool {
        lrr_int_error_code(self) == lrr_int_error_code(other)
    }
}
impl crate::cmp::Eq for IntErrorKind {}
fn lrr_int_error_code(kind: &IntErrorKind) -> u32 {
    match kind {
        IntErrorKind::Empty => 0,
        IntErrorKind::InvalidDigit => 1,
        IntErrorKind::PosOverflow => 2,
        IntErrorKind::NegOverflow => 3,
        IntErrorKind::Zero => 4,
    }
}
impl crate::fmt::Debug for IntErrorKind {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.write_str(match self {
            IntErrorKind::Empty => "Empty",
            IntErrorKind::InvalidDigit => "InvalidDigit",
            IntErrorKind::PosOverflow => "PosOverflow",
            IntErrorKind::NegOverflow => "NegOverflow",
            IntErrorKind::Zero => "Zero",
        })
    }
}
impl crate::clone::Clone for ParseIntError {
    fn clone(&self) -> ParseIntError {
        ParseIntError { kind: self.kind.clone() }
    }
}
impl crate::cmp::PartialEq for ParseIntError {
    fn eq(&self, other: &ParseIntError) -> bool {
        self.kind == other.kind
    }
}
impl crate::cmp::Eq for ParseIntError {}
impl crate::fmt::Debug for ParseIntError {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.debug_struct("ParseIntError").field("kind", &self.kind).finish()
    }
}
impl crate::fmt::Display for ParseIntError {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        f.pad(match self.kind {
            IntErrorKind::Empty => "cannot parse integer from empty string",
            IntErrorKind::InvalidDigit => "invalid digit found in string",
            IntErrorKind::PosOverflow => "number too large to fit in target type",
            IntErrorKind::NegOverflow => "number too small to fit in target type",
            IntErrorKind::Zero => "number would be zero for non-zero type",
        })
    }
}

// The digits of `s` in `radix` from byte `from`, as a value at most `limit`.
fn lrr_parse_digits(
    s: &str,
    from: usize,
    radix: u64,
    limit: u64,
    overflow: IntErrorKind,
) -> crate::result::Result<u64, ParseIntError> {
    let n: usize = core::intrinsics::lrr_str_len(s);
    if from >= n {
        return crate::result::Result::Err(ParseIntError { kind: IntErrorKind::InvalidDigit });
    }
    let mut value: u64 = 0;
    let mut pos: usize = from;
    while pos < n {
        let byte: u64 = core::intrinsics::lrr_str_byte(s, pos) as u64;
        let digit: u64 = if byte >= 48 && byte <= 57 {
            byte - 48
        } else if byte >= 97 && byte <= 122 {
            byte - 87
        } else if byte >= 65 && byte <= 90 {
            byte - 55
        } else {
            return crate::result::Result::Err(ParseIntError { kind: IntErrorKind::InvalidDigit });
        };
        if digit >= radix {
            return crate::result::Result::Err(ParseIntError { kind: IntErrorKind::InvalidDigit });
        }
        if value > (limit - digit) / radix {
            return crate::result::Result::Err(ParseIntError { kind: overflow });
        }
        value = value * radix + digit;
        pos += 1;
    }
    crate::result::Result::Ok(value)
}
fn lrr_check_radix(radix: u32) {
    if radix < 2 || radix > 36 {
        // Real core appends ` - found {radix}`; the facade cannot format a
        // runtime message (`format_args!` in facade source is not lowered).
        core::panicking::panic("from_ascii_bytes_radix: radix must lie in the range `[2, 36]`");
    }
}
pub(crate) fn lrr_parse_radix_u64(
    s: &str,
    radix: u32,
    max: u64,
) -> crate::result::Result<u64, ParseIntError> {
    lrr_check_radix(radix);
    let n: usize = core::intrinsics::lrr_str_len(s);
    if n == 0 {
        return crate::result::Result::Err(ParseIntError { kind: IntErrorKind::Empty });
    }
    let mut from: usize = 0;
    if core::intrinsics::lrr_str_byte(s, 0) == 43 {
        from = 1;
    }
    lrr_parse_digits(s, from, radix as u64, max, IntErrorKind::PosOverflow)
}
pub(crate) fn lrr_parse_radix_i64(
    s: &str,
    radix: u32,
    min: i64,
    max: i64,
) -> crate::result::Result<i64, ParseIntError> {
    lrr_check_radix(radix);
    let n: usize = core::intrinsics::lrr_str_len(s);
    if n == 0 {
        return crate::result::Result::Err(ParseIntError { kind: IntErrorKind::Empty });
    }
    let first: u8 = core::intrinsics::lrr_str_byte(s, 0);
    if first == 45 {
        let limit: u64 = ((0i64 - (min + 1)) as u64) + 1;
        match lrr_parse_digits(s, 1, radix as u64, limit, IntErrorKind::NegOverflow) {
            crate::result::Result::Ok(magnitude) => {
                if magnitude == limit {
                    crate::result::Result::Ok(min)
                } else {
                    crate::result::Result::Ok(0i64 - (magnitude as i64))
                }
            }
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    } else {
        let mut from: usize = 0;
        if first == 43 {
            from = 1;
        }
        match lrr_parse_digits(s, from, radix as u64, max as u64, IntErrorKind::PosOverflow) {
            crate::result::Result::Ok(value) => crate::result::Result::Ok(value as i64),
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    }
}
pub(crate) fn lrr_parse_u64(s: &str, max: u64) -> crate::result::Result<u64, ParseIntError> {
    lrr_parse_radix_u64(s, 10, max)
}
pub(crate) fn lrr_parse_i64(s: &str, min: i64, max: i64) -> crate::result::Result<i64, ParseIntError> {
    lrr_parse_radix_i64(s, 10, min, max)
}
pub struct ParseFloatError;
pub struct TryFromIntError(pub(crate) ());

pub struct Wrapping<T>(pub T);
pub struct Saturating<T>(pub T);

pub struct NonZero<T>(T);
pub type NonZeroU8 = NonZero<u8>;
pub type NonZeroU16 = NonZero<u16>;
pub type NonZeroU32 = NonZero<u32>;
pub type NonZeroU64 = NonZero<u64>;
pub type NonZeroUsize = NonZero<usize>;
pub type NonZeroI8 = NonZero<i8>;
pub type NonZeroI16 = NonZero<i16>;
pub type NonZeroI32 = NonZero<i32>;
pub type NonZeroI64 = NonZero<i64>;
pub type NonZeroIsize = NonZero<isize>;

pub enum FpCategory {
    Nan,
    Infinite,
    Zero,
    Subnormal,
    Normal,
}

impl crate::str::FromStr for i8 {
    type Err = ParseIntError;
    fn from_str(s: &str) -> crate::result::Result<i8, ParseIntError> {
        match lrr_parse_i64(s, crate::i8::MIN as i64, crate::i8::MAX as i64) {
            crate::result::Result::Ok(value) => crate::result::Result::Ok(value as i8),
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    }
}
impl crate::str::FromStr for i16 {
    type Err = ParseIntError;
    fn from_str(s: &str) -> crate::result::Result<i16, ParseIntError> {
        match lrr_parse_i64(s, crate::i16::MIN as i64, crate::i16::MAX as i64) {
            crate::result::Result::Ok(value) => crate::result::Result::Ok(value as i16),
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    }
}
impl crate::str::FromStr for i32 {
    type Err = ParseIntError;
    fn from_str(s: &str) -> crate::result::Result<i32, ParseIntError> {
        match lrr_parse_i64(s, crate::i32::MIN as i64, crate::i32::MAX as i64) {
            crate::result::Result::Ok(value) => crate::result::Result::Ok(value as i32),
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    }
}
impl crate::str::FromStr for i64 {
    type Err = ParseIntError;
    fn from_str(s: &str) -> crate::result::Result<i64, ParseIntError> {
        match lrr_parse_i64(s, crate::i64::MIN as i64, crate::i64::MAX as i64) {
            crate::result::Result::Ok(value) => crate::result::Result::Ok(value as i64),
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    }
}
impl crate::str::FromStr for isize {
    type Err = ParseIntError;
    fn from_str(s: &str) -> crate::result::Result<isize, ParseIntError> {
        match lrr_parse_i64(s, crate::i64::MIN as i64, crate::i64::MAX as i64) {
            crate::result::Result::Ok(value) => crate::result::Result::Ok(value as isize),
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    }
}
impl crate::str::FromStr for u8 {
    type Err = ParseIntError;
    fn from_str(s: &str) -> crate::result::Result<u8, ParseIntError> {
        match lrr_parse_u64(s, crate::u8::MAX as u64) {
            crate::result::Result::Ok(value) => crate::result::Result::Ok(value as u8),
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    }
}
impl crate::str::FromStr for u16 {
    type Err = ParseIntError;
    fn from_str(s: &str) -> crate::result::Result<u16, ParseIntError> {
        match lrr_parse_u64(s, crate::u16::MAX as u64) {
            crate::result::Result::Ok(value) => crate::result::Result::Ok(value as u16),
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    }
}
impl crate::str::FromStr for u32 {
    type Err = ParseIntError;
    fn from_str(s: &str) -> crate::result::Result<u32, ParseIntError> {
        match lrr_parse_u64(s, crate::u32::MAX as u64) {
            crate::result::Result::Ok(value) => crate::result::Result::Ok(value as u32),
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    }
}
impl crate::str::FromStr for u64 {
    type Err = ParseIntError;
    fn from_str(s: &str) -> crate::result::Result<u64, ParseIntError> {
        match lrr_parse_u64(s, crate::u64::MAX as u64) {
            crate::result::Result::Ok(value) => crate::result::Result::Ok(value as u64),
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    }
}
impl crate::str::FromStr for usize {
    type Err = ParseIntError;
    fn from_str(s: &str) -> crate::result::Result<usize, ParseIntError> {
        match lrr_parse_u64(s, crate::u64::MAX as u64) {
            crate::result::Result::Ok(value) => crate::result::Result::Ok(value as usize),
            crate::result::Result::Err(error) => crate::result::Result::Err(error),
        }
    }
}
