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
pub struct ParseFloatError;
pub struct TryFromIntError(());

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
