// `core::cmp`.
use crate::marker::Sized;
use crate::option::Option;

pub trait PartialEq<Rhs: ?Sized = Self> {
    fn eq(&self, other: &Rhs) -> bool;
    fn ne(&self, other: &Rhs) -> bool {
        !self.eq(other)
    }
}

pub trait Eq: PartialEq<Self> {}

pub enum Ordering {
    Less = -1,
    Equal = 0,
    Greater = 1,
}

impl Ordering {
    pub fn is_eq(self) -> bool {
        match self {
            Ordering::Equal => true,
            _ => false,
        }
    }
    pub fn is_ne(self) -> bool {
        match self {
            Ordering::Equal => false,
            _ => true,
        }
    }
    pub fn is_lt(self) -> bool {
        match self {
            Ordering::Less => true,
            _ => false,
        }
    }
    pub fn is_gt(self) -> bool {
        match self {
            Ordering::Greater => true,
            _ => false,
        }
    }
    pub fn is_le(self) -> bool {
        match self {
            Ordering::Greater => false,
            _ => true,
        }
    }
    pub fn is_ge(self) -> bool {
        match self {
            Ordering::Less => false,
            _ => true,
        }
    }
    pub fn reverse(self) -> Ordering {
        match self {
            Ordering::Less => Ordering::Greater,
            Ordering::Equal => Ordering::Equal,
            Ordering::Greater => Ordering::Less,
        }
    }
    pub fn then(self, other: Ordering) -> Ordering {
        match self {
            Ordering::Equal => other,
            _ => self,
        }
    }
}

impl crate::clone::Clone for Ordering {
    fn clone(&self) -> Ordering {
        *self
    }
}
impl crate::marker::Copy for Ordering {}
impl PartialEq for Ordering {
    fn eq(&self, other: &Ordering) -> bool {
        (*self as i32) == (*other as i32)
    }
}
impl Eq for Ordering {}
impl crate::fmt::Debug for Ordering {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        match self {
            Ordering::Less => f.write_str("Less"),
            Ordering::Equal => f.write_str("Equal"),
            Ordering::Greater => f.write_str("Greater"),
        }
    }
}

pub trait PartialOrd<Rhs: ?Sized = Self>: PartialEq<Rhs> {
    fn partial_cmp(&self, other: &Rhs) -> Option<Ordering>;
    fn lt(&self, other: &Rhs) -> bool {
        match self.partial_cmp(other) {
            Option::Some(Ordering::Less) => true,
            _ => false,
        }
    }
    fn le(&self, other: &Rhs) -> bool {
        match self.partial_cmp(other) {
            Option::Some(Ordering::Less) | Option::Some(Ordering::Equal) => true,
            _ => false,
        }
    }
    fn gt(&self, other: &Rhs) -> bool {
        match self.partial_cmp(other) {
            Option::Some(Ordering::Greater) => true,
            _ => false,
        }
    }
    fn ge(&self, other: &Rhs) -> bool {
        match self.partial_cmp(other) {
            Option::Some(Ordering::Greater) | Option::Some(Ordering::Equal) => true,
            _ => false,
        }
    }
}

pub trait Ord: Eq + PartialOrd<Self> {
    fn cmp(&self, other: &Self) -> Ordering;
    fn max(self, other: Self) -> Self
    where
        Self: Sized,
    {
        match other.cmp(&self) {
            Ordering::Less => self,
            _ => other,
        }
    }
    fn min(self, other: Self) -> Self
    where
        Self: Sized,
    {
        match other.cmp(&self) {
            Ordering::Less => other,
            _ => self,
        }
    }
    fn clamp(self, min: Self, max: Self) -> Self
    where
        Self: Sized,
    {
        if self.cmp(&min) == Ordering::Less {
            min
        } else if self.cmp(&max) == Ordering::Greater {
            max
        } else {
            self
        }
    }
}

pub fn max<T: Ord>(v1: T, v2: T) -> T {
    v1.max(v2)
}

pub fn min<T: Ord>(v1: T, v2: T) -> T {
    v1.min(v2)
}

// Reverses the ordering of the wrapped value.
pub struct Reverse<T>(pub T);

// References compare their referents (`&A == &B`, `&mut A == &mut B`),
// through the referents' impls as real core writes them.
impl<'a, 'b, A: ?Sized, B: ?Sized> PartialEq<&'b B> for &'a A
where
    A: PartialEq<B>,
{
    fn eq(&self, other: &&'b B) -> bool {
        PartialEq::eq(*self, *other)
    }
    fn ne(&self, other: &&'b B) -> bool {
        PartialEq::ne(*self, *other)
    }
}
impl<'a, 'b, A: ?Sized, B: ?Sized> PartialEq<&'b mut B> for &'a mut A
where
    A: PartialEq<B>,
{
    fn eq(&self, other: &&'b mut B) -> bool {
        **self == **other
    }
}
impl<'a, A: ?Sized + Eq> Eq for &'a A {}
impl<'a, 'b, A: ?Sized, B: ?Sized> PartialOrd<&'b B> for &'a A
where
    A: PartialOrd<B>,
{
    fn partial_cmp(&self, other: &&'b B) -> Option<Ordering> {
        PartialOrd::partial_cmp(*self, *other)
    }
    fn lt(&self, other: &&'b B) -> bool {
        PartialOrd::lt(*self, *other)
    }
    fn le(&self, other: &&'b B) -> bool {
        PartialOrd::le(*self, *other)
    }
    fn gt(&self, other: &&'b B) -> bool {
        PartialOrd::gt(*self, *other)
    }
    fn ge(&self, other: &&'b B) -> bool {
        PartialOrd::ge(*self, *other)
    }
}
impl<'a, A: ?Sized + Ord> Ord for &'a A {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(*self, *other)
    }
}

// Tuples compare lexicographically, element by element.
impl PartialEq for () {
    fn eq(&self, _other: &()) -> bool {
        true
    }
}
impl Eq for () {}
impl<A: PartialEq> PartialEq for (A,) {
    fn eq(&self, other: &(A,)) -> bool {
        self.0 == other.0
    }
}
impl<A: PartialEq, B: PartialEq> PartialEq for (A, B) {
    fn eq(&self, other: &(A, B)) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}
impl<A: PartialEq, B: PartialEq, C: PartialEq> PartialEq for (A, B, C) {
    fn eq(&self, other: &(A, B, C)) -> bool {
        self.0 == other.0 && self.1 == other.1 && self.2 == other.2
    }
}
impl<A: PartialEq, B: PartialEq, C: PartialEq, D: PartialEq> PartialEq for (A, B, C, D) {
    fn eq(&self, other: &(A, B, C, D)) -> bool {
        self.0 == other.0 && self.1 == other.1 && self.2 == other.2 && self.3 == other.3
    }
}
impl<A: Eq> Eq for (A,) {}
impl<A: Eq, B: Eq> Eq for (A, B) {}
impl<A: Eq, B: Eq, C: Eq> Eq for (A, B, C) {}
impl<A: Eq, B: Eq, C: Eq, D: Eq> Eq for (A, B, C, D) {}
impl<A: PartialOrd> PartialOrd for (A,) {
    fn partial_cmp(&self, other: &(A,)) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<A: PartialOrd, B: PartialOrd> PartialOrd for (A, B) {
    fn partial_cmp(&self, other: &(A, B)) -> Option<Ordering> {
        match self.0.partial_cmp(&other.0) {
            Option::Some(Ordering::Equal) => self.1.partial_cmp(&other.1),
            ordering => ordering,
        }
    }
}
impl<A: PartialOrd, B: PartialOrd, C: PartialOrd> PartialOrd for (A, B, C) {
    fn partial_cmp(&self, other: &(A, B, C)) -> Option<Ordering> {
        match self.0.partial_cmp(&other.0) {
            Option::Some(Ordering::Equal) => match self.1.partial_cmp(&other.1) {
                Option::Some(Ordering::Equal) => self.2.partial_cmp(&other.2),
                ordering => ordering,
            },
            ordering => ordering,
        }
    }
}
impl<A: Ord> Ord for (A,) {
    fn cmp(&self, other: &(A,)) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl<A: Ord, B: Ord> Ord for (A, B) {
    fn cmp(&self, other: &(A, B)) -> Ordering {
        match self.0.cmp(&other.0) {
            Ordering::Equal => self.1.cmp(&other.1),
            ordering => ordering,
        }
    }
}
