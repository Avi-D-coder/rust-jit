// `core::ops`. Operators on primitive operands are built into the
// interpreter; these traits are the identities user and facade
// implementations select through.
use crate::marker::{Sized, Tuple};

pub trait Add<Rhs = Self> {
    type Output;
    fn add(self, rhs: Rhs) -> Self::Output;
}
pub trait Sub<Rhs = Self> {
    type Output;
    fn sub(self, rhs: Rhs) -> Self::Output;
}
pub trait Mul<Rhs = Self> {
    type Output;
    fn mul(self, rhs: Rhs) -> Self::Output;
}
pub trait Div<Rhs = Self> {
    type Output;
    fn div(self, rhs: Rhs) -> Self::Output;
}
pub trait Rem<Rhs = Self> {
    type Output;
    fn rem(self, rhs: Rhs) -> Self::Output;
}
pub trait Neg {
    type Output;
    fn neg(self) -> Self::Output;
}
pub trait Not {
    type Output;
    fn not(self) -> Self::Output;
}
pub trait BitAnd<Rhs = Self> {
    type Output;
    fn bitand(self, rhs: Rhs) -> Self::Output;
}
pub trait BitOr<Rhs = Self> {
    type Output;
    fn bitor(self, rhs: Rhs) -> Self::Output;
}
pub trait BitXor<Rhs = Self> {
    type Output;
    fn bitxor(self, rhs: Rhs) -> Self::Output;
}
pub trait Shl<Rhs = Self> {
    type Output;
    fn shl(self, rhs: Rhs) -> Self::Output;
}
pub trait Shr<Rhs = Self> {
    type Output;
    fn shr(self, rhs: Rhs) -> Self::Output;
}
pub trait AddAssign<Rhs = Self> {
    fn add_assign(&mut self, rhs: Rhs);
}
pub trait SubAssign<Rhs = Self> {
    fn sub_assign(&mut self, rhs: Rhs);
}
pub trait MulAssign<Rhs = Self> {
    fn mul_assign(&mut self, rhs: Rhs);
}
pub trait DivAssign<Rhs = Self> {
    fn div_assign(&mut self, rhs: Rhs);
}
pub trait RemAssign<Rhs = Self> {
    fn rem_assign(&mut self, rhs: Rhs);
}
pub trait BitAndAssign<Rhs = Self> {
    fn bitand_assign(&mut self, rhs: Rhs);
}
pub trait BitOrAssign<Rhs = Self> {
    fn bitor_assign(&mut self, rhs: Rhs);
}
pub trait BitXorAssign<Rhs = Self> {
    fn bitxor_assign(&mut self, rhs: Rhs);
}
pub trait ShlAssign<Rhs = Self> {
    fn shl_assign(&mut self, rhs: Rhs);
}
pub trait ShrAssign<Rhs = Self> {
    fn shr_assign(&mut self, rhs: Rhs);
}

pub trait Index<Idx: ?Sized> {
    type Output: ?Sized;
    fn index(&self, index: Idx) -> &Self::Output;
}
pub trait IndexMut<Idx: ?Sized>: Index<Idx> {
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output;
}

pub trait Deref {
    type Target: ?Sized;
    fn deref(&self) -> &Self::Target;
}
pub trait DerefMut: Deref {
    fn deref_mut(&mut self) -> &mut Self::Target;
}
pub trait Receiver {
    type Target: ?Sized;
}

pub trait Drop {
    fn drop(&mut self);
}

// Closure traits. Calls through them are built into the interpreter.
pub trait FnOnce<Args: Tuple> {
    type Output;
    fn call_once(self, args: Args) -> Self::Output;
}
pub trait FnMut<Args: Tuple>: FnOnce<Args> {
    fn call_mut(&mut self, args: Args) -> Self::Output;
}
pub trait Fn<Args: Tuple>: FnMut<Args> {
    fn call(&self, args: Args) -> Self::Output;
}

// Async closure traits: names resolve; their execution is outside the
// interpreter's current profile.
pub trait AsyncFnOnce<Args: Tuple> {
    type CallOnceFuture;
    type Output;
}
pub trait AsyncFnMut<Args: Tuple>: AsyncFnOnce<Args> {
    type CallRefFuture<'a>
    where
        Self: 'a;
}
pub trait AsyncFn<Args: Tuple>: AsyncFnMut<Args> {}

pub enum ControlFlow<B, C = ()> {
    Continue(C),
    Break(B),
}

pub trait Try: FromResidual<Self::Residual> {
    type Output;
    type Residual;
    fn from_output(output: Self::Output) -> Self;
    fn branch(self) -> ControlFlow<Self::Residual, Self::Output>;
}
pub trait FromResidual<R> {
    fn from_residual(residual: R) -> Self;
}

// `Residual<O>`: the `Try` type whose residual is `Self` and whose output is `O`
// (`Option<Infallible>` with output `T` is `Option<T>`).
pub trait Residual<O> {
    type TryType: Try<Output = O, Residual = Self>;
}
impl<T> Residual<T> for crate::option::Option<crate::convert::Infallible> {
    type TryType = crate::option::Option<T>;
}
impl<T, E> Residual<T> for crate::result::Result<crate::convert::Infallible, E> {
    type TryType = crate::result::Result<T, E>;
}
impl<B, C> Residual<C> for ControlFlow<B, crate::convert::Infallible> {
    type TryType = ControlFlow<B, C>;
}

impl<B, C> ControlFlow<B, C> {
    pub fn is_break(&self) -> bool {
        match self {
            ControlFlow::Break(_) => true,
            ControlFlow::Continue(_) => false,
        }
    }
    pub fn is_continue(&self) -> bool {
        match self {
            ControlFlow::Break(_) => false,
            ControlFlow::Continue(_) => true,
        }
    }
    pub fn break_value(self) -> crate::option::Option<B> {
        match self {
            ControlFlow::Break(b) => crate::option::Option::Some(b),
            ControlFlow::Continue(_) => crate::option::Option::None,
        }
    }
    pub fn continue_value(self) -> crate::option::Option<C> {
        match self {
            ControlFlow::Break(_) => crate::option::Option::None,
            ControlFlow::Continue(c) => crate::option::Option::Some(c),
        }
    }
    // `<ControlFlow<B, C> as Try>::Output`, stated for the interpreter's static
    // `expr?` typing (never called). Not part of the real `ControlFlow`.
    #[doc(hidden)]
    pub fn __try_output(self) -> C {
        crate::intrinsics::lrr_unsupported("ControlFlow::__try_output")
    }
    // The residual of `expr?`'s `Break(residual)` arm, which the lowering has
    // already matched. Not part of the real `core::ops::ControlFlow`.
    #[doc(hidden)]
    pub fn __try_break_residual(self) -> B {
        match self {
            ControlFlow::Break(b) => b,
            ControlFlow::Continue(_) => loop {},
        }
    }
}
// `?` on a `ControlFlow`: the residual keeps the break value.
impl<B, C> Try for ControlFlow<B, C> {
    type Output = C;
    type Residual = ControlFlow<B, crate::convert::Infallible>;
    fn from_output(output: C) -> Self {
        ControlFlow::Continue(output)
    }
    fn branch(self) -> ControlFlow<ControlFlow<B, crate::convert::Infallible>, C> {
        match self {
            ControlFlow::Continue(c) => ControlFlow::Continue(c),
            ControlFlow::Break(b) => ControlFlow::Break(ControlFlow::Break(b)),
        }
    }
}
impl<B, C> FromResidual<ControlFlow<B, crate::convert::Infallible>> for ControlFlow<B, C> {
    fn from_residual(residual: ControlFlow<B, crate::convert::Infallible>) -> Self {
        match residual {
            ControlFlow::Break(b) => ControlFlow::Break(b),
        }
    }
}

pub enum CoroutineState<Y, R> {
    Yielded(Y),
    Complete(R),
}
pub trait Coroutine<R = ()> {
    type Yield;
    type Return;
}

// `..`
pub struct RangeFull;
// `start..end`
pub struct Range<Idx> {
    pub start: Idx,
    pub end: Idx,
}
// `start..`
pub struct RangeFrom<Idx> {
    pub start: Idx,
}
// `..end`
pub struct RangeTo<Idx> {
    pub end: Idx,
}
// `start..=end`
pub struct RangeInclusive<Idx> {
    // `pub(crate)` as in core: `core::iter`'s `Iterator` impl reads them.
    pub(crate) start: Idx,
    pub(crate) end: Idx,
    pub(crate) exhausted: bool,
}
// `..=end`
pub struct RangeToInclusive<Idx> {
    pub end: Idx,
}

impl<Idx> RangeInclusive<Idx> {
    pub const fn new(start: Idx, end: Idx) -> Self {
        RangeInclusive {
            start,
            end,
            exhausted: false,
        }
    }
    pub const fn start(&self) -> &Idx {
        &self.start
    }
    pub const fn end(&self) -> &Idx {
        &self.end
    }
    pub fn into_inner(self) -> (Idx, Idx) {
        (self.start, self.end)
    }
}

pub enum Bound<T> {
    Included(T),
    Excluded(T),
    Unbounded,
}

pub trait RangeBounds<T: ?Sized> {
    fn start_bound(&self) -> Bound<&T>;
    fn end_bound(&self) -> Bound<&T>;
    fn contains<U>(&self, item: &U) -> bool
    where
        T: crate::cmp::PartialOrd<U>,
        U: ?Sized + crate::cmp::PartialOrd<T>,
    {
        core::intrinsics::lrr_unsupported("core::ops::RangeBounds::contains")
    }
}

impl<T> RangeBounds<T> for Range<T> {
    fn start_bound(&self) -> Bound<&T> {
        Bound::Included(&self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Bound::Excluded(&self.end)
    }
}
impl<T> RangeBounds<T> for RangeFrom<T> {
    fn start_bound(&self) -> Bound<&T> {
        Bound::Included(&self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Bound::Unbounded
    }
}
impl<T> RangeBounds<T> for RangeTo<T> {
    fn start_bound(&self) -> Bound<&T> {
        Bound::Unbounded
    }
    fn end_bound(&self) -> Bound<&T> {
        Bound::Excluded(&self.end)
    }
}
impl<T> RangeBounds<T> for RangeInclusive<T> {
    fn start_bound(&self) -> Bound<&T> {
        Bound::Included(&self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Bound::Included(&self.end)
    }
}
impl<T> RangeBounds<T> for RangeToInclusive<T> {
    fn start_bound(&self) -> Bound<&T> {
        Bound::Unbounded
    }
    fn end_bound(&self) -> Bound<&T> {
        Bound::Included(&self.end)
    }
}
impl<T: ?Sized> RangeBounds<T> for RangeFull {
    fn start_bound(&self) -> Bound<&T> {
        Bound::Unbounded
    }
    fn end_bound(&self) -> Bound<&T> {
        Bound::Unbounded
    }
}

// The bounds of a range of indices into a sequence of `len` elements
// (`Vec::drain`'s `RangeBounds<usize>`).
pub trait LrrIndexRange {
    fn lrr_start(&self) -> usize;
    fn lrr_end(&self, len: usize) -> usize;
}
// The integer types an unsuffixed range literal may take: one blanket impl
// per range shape, so the literal's fallback type meets a single impl.
pub trait LrrIndex {
    fn lrr_index(&self) -> usize;
}
impl LrrIndex for usize {
    fn lrr_index(&self) -> usize {
        *self as usize
    }
}
impl LrrIndex for u8 {
    fn lrr_index(&self) -> usize {
        *self as usize
    }
}
impl LrrIndex for u16 {
    fn lrr_index(&self) -> usize {
        *self as usize
    }
}
impl LrrIndex for u32 {
    fn lrr_index(&self) -> usize {
        *self as usize
    }
}
impl LrrIndex for u64 {
    fn lrr_index(&self) -> usize {
        *self as usize
    }
}
impl LrrIndex for i8 {
    fn lrr_index(&self) -> usize {
        *self as usize
    }
}
impl LrrIndex for i16 {
    fn lrr_index(&self) -> usize {
        *self as usize
    }
}
impl LrrIndex for i32 {
    fn lrr_index(&self) -> usize {
        *self as usize
    }
}
impl LrrIndex for i64 {
    fn lrr_index(&self) -> usize {
        *self as usize
    }
}
impl LrrIndex for isize {
    fn lrr_index(&self) -> usize {
        *self as usize
    }
}
impl LrrIndexRange for RangeFull {
    fn lrr_start(&self) -> usize {
        0
    }
    fn lrr_end(&self, len: usize) -> usize {
        len
    }
}
impl<I: LrrIndex> LrrIndexRange for Range<I> {
    fn lrr_start(&self) -> usize {
        self.start.lrr_index()
    }
    fn lrr_end(&self, len: usize) -> usize {
        self.end.lrr_index()
    }
}
impl<I: LrrIndex> LrrIndexRange for RangeFrom<I> {
    fn lrr_start(&self) -> usize {
        self.start.lrr_index()
    }
    fn lrr_end(&self, len: usize) -> usize {
        len
    }
}
impl<I: LrrIndex> LrrIndexRange for RangeTo<I> {
    fn lrr_start(&self) -> usize {
        0
    }
    fn lrr_end(&self, len: usize) -> usize {
        self.end.lrr_index()
    }
}
impl<I: LrrIndex> LrrIndexRange for RangeInclusive<I> {
    fn lrr_start(&self) -> usize {
        self.start.lrr_index()
    }
    fn lrr_end(&self, len: usize) -> usize {
        self.end.lrr_index() + 1
    }
}
impl<I: LrrIndex> LrrIndexRange for RangeToInclusive<I> {
    fn lrr_start(&self) -> usize {
        0
    }
    fn lrr_end(&self, len: usize) -> usize {
        self.end.lrr_index() + 1
    }
}

impl<Idx: crate::cmp::PartialOrd<Idx>> Range<Idx> {
    pub fn contains<U>(&self, item: &U) -> bool
    where
        Idx: crate::cmp::PartialOrd<U>,
        U: ?Sized + crate::cmp::PartialOrd<Idx>,
    {
        *item >= self.start && *item < self.end
    }
    pub fn is_empty(&self) -> bool {
        !(self.start < self.end)
    }
}

impl<Idx: crate::cmp::PartialOrd<Idx>> RangeInclusive<Idx> {
    pub fn contains<U>(&self, item: &U) -> bool
    where
        Idx: crate::cmp::PartialOrd<U>,
        U: ?Sized + crate::cmp::PartialOrd<Idx>,
    {
        *item >= self.start && *item <= self.end
    }
    pub fn is_empty(&self) -> bool {
        self.exhausted || !(self.start <= self.end)
    }
}

// Pinned library/core/src/ops/unsize.rs: marker traits through which a
// smart-pointer type opts in to unsized coercion (`CoerceUnsized`) and to
// dynamic-receiver dispatch (`DispatchFromDyn`). The interpreter's built-in
// reference/`Box` unsizing does not consult them; they are the identities
// user `impl CoerceUnsized<Ptr<U>> for Ptr<T>` declarations select.
pub trait CoerceUnsized<T: ?Sized> {}
pub trait DispatchFromDyn<T> {}
