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
