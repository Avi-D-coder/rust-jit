// `core::iter`. Iteration over built-in ranges, slices and containers is
// driven by the interpreter; these are the trait identities it selects.
use crate::marker::Sized;
use crate::ops::FnMut;
use crate::option::Option;

pub trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Option::None)
    }
    fn count(self) -> usize
    where
        Self: Sized,
    {
        let mut iter = self;
        let mut count = 0;
        while let Option::Some(_) = iter.next() {
            count += 1;
        }
        count
    }
    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        let mut iter = self;
        let mut last = Option::None;
        while let Option::Some(x) = iter.next() {
            last = Option::Some(x);
        }
        last
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let mut n = n;
        while n > 0 {
            if let Option::None = self.next() {
                return Option::None;
            }
            n -= 1;
        }
        self.next()
    }
    fn for_each<F>(self, f: F)
    where
        Self: Sized,
        F: FnMut(Self::Item),
    {
        let mut iter = self;
        let mut f = f;
        while let Option::Some(x) = iter.next() {
            f(x);
        }
    }
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let mut iter = self;
        let mut f = f;
        let mut accum = init;
        while let Option::Some(x) = iter.next() {
            accum = f(accum, x);
        }
        accum
    }
    fn all<F>(&mut self, f: F) -> bool
    where
        Self: Sized,
        F: FnMut(Self::Item) -> bool,
    {
        let mut f = f;
        while let Option::Some(x) = self.next() {
            if !f(x) {
                return false;
            }
        }
        true
    }
    fn any<F>(&mut self, f: F) -> bool
    where
        Self: Sized,
        F: FnMut(Self::Item) -> bool,
    {
        let mut f = f;
        while let Option::Some(x) = self.next() {
            if f(x) {
                return true;
            }
        }
        false
    }
    fn find<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        let mut predicate = predicate;
        while let Option::Some(x) = self.next() {
            if predicate(&x) {
                return Option::Some(x);
            }
        }
        Option::None
    }
    fn position<P>(&mut self, predicate: P) -> Option<usize>
    where
        Self: Sized,
        P: FnMut(Self::Item) -> bool,
    {
        let mut predicate = predicate;
        let mut i = 0;
        while let Option::Some(x) = self.next() {
            if predicate(x) {
                return Option::Some(i);
            }
            i += 1;
        }
        Option::None
    }
    fn map<B, F>(self, f: F) -> Map<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> B,
    {
        Map { iter: self, f }
    }
    fn filter<P>(self, predicate: P) -> Filter<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        Filter {
            iter: self,
            predicate,
        }
    }
    fn enumerate(self) -> Enumerate<Self>
    where
        Self: Sized,
    {
        Enumerate { iter: self, count: 0 }
    }
    fn take(self, n: usize) -> Take<Self>
    where
        Self: Sized,
    {
        Take { iter: self, n }
    }
    fn skip(self, n: usize) -> Skip<Self>
    where
        Self: Sized,
    {
        Skip { iter: self, n }
    }
    fn zip<U>(self, other: U) -> Zip<Self, U::IntoIter>
    where
        Self: Sized,
        U: IntoIterator,
    {
        Zip {
            a: self,
            b: other.into_iter(),
        }
    }
    fn chain<U>(self, other: U) -> Chain<Self, U::IntoIter>
    where
        Self: Sized,
        U: IntoIterator<Item = Self::Item>,
    {
        Chain {
            a: Option::Some(self),
            b: Option::Some(other.into_iter()),
        }
    }
    fn peekable(self) -> Peekable<Self>
    where
        Self: Sized,
    {
        Peekable {
            iter: self,
            peeked: Option::None,
        }
    }
    fn rev(self) -> Rev<Self>
    where
        Self: Sized + DoubleEndedIterator,
    {
        Rev { iter: self }
    }
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }
    fn collect<B: FromIterator<Self::Item>>(self) -> B
    where
        Self: Sized,
    {
        FromIterator::from_iter(self)
    }
    fn sum<S>(self) -> S
    where
        Self: Sized,
        S: Sum<Self::Item>,
    {
        Sum::sum(self)
    }
    fn product<P>(self) -> P
    where
        Self: Sized,
        P: Product<Self::Item>,
    {
        Product::product(self)
    }
    fn max(self) -> Option<Self::Item>
    where
        Self: Sized,
        Self::Item: crate::cmp::Ord,
    {
        let mut iter = self;
        let mut best = match iter.next() {
            Option::Some(first) => first,
            Option::None => return Option::None,
        };
        while let Option::Some(x) = iter.next() {
            if crate::cmp::Ord::cmp(&x, &best) != crate::cmp::Ordering::Less {
                best = x;
            }
        }
        Option::Some(best)
    }
    fn min(self) -> Option<Self::Item>
    where
        Self: Sized,
        Self::Item: crate::cmp::Ord,
    {
        let mut iter = self;
        let mut best = match iter.next() {
            Option::Some(first) => first,
            Option::None => return Option::None,
        };
        while let Option::Some(x) = iter.next() {
            if crate::cmp::Ord::cmp(&x, &best) == crate::cmp::Ordering::Less {
                best = x;
            }
        }
        Option::Some(best)
    }
    fn cloned<'a, T>(self) -> Cloned<Self>
    where
        T: 'a + crate::clone::Clone,
        Self: Sized + Iterator<Item = &'a T>,
    {
        Cloned { it: self }
    }
    fn copied<'a, T>(self) -> Copied<Self>
    where
        T: 'a + crate::marker::Copy,
        Self: Sized + Iterator<Item = &'a T>,
    {
        Copied { it: self }
    }
}

pub trait DoubleEndedIterator: Iterator {
    fn next_back(&mut self) -> Option<Self::Item>;
}
pub trait ExactSizeIterator: Iterator {
    fn len(&self) -> usize {
        let (lower, _) = self.size_hint();
        lower
    }
}
pub trait FusedIterator: Iterator {}
pub unsafe trait TrustedLen: Iterator {}

pub trait IntoIterator {
    type Item;
    type IntoIter: Iterator<Item = Self::Item>;
    fn into_iter(self) -> Self::IntoIter;
}
impl<I: Iterator> IntoIterator for I {
    type Item = I::Item;
    type IntoIter = I;
    fn into_iter(self) -> I {
        self
    }
}

pub trait FromIterator<A>: Sized {
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self;
}
pub trait Extend<A> {
    fn extend<T: IntoIterator<Item = A>>(&mut self, iter: T);
}
pub trait Sum<A = Self>: Sized {
    fn sum<I: Iterator<Item = A>>(iter: I) -> Self;
}
pub trait Product<A = Self>: Sized {
    fn product<I: Iterator<Item = A>>(iter: I) -> Self;
}

impl<I: Iterator + ?Sized> Iterator for &mut I {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        (**self).next()
    }
}

pub struct Map<I, F> {
    iter: I,
    f: F,
}
impl<B, I: Iterator, F> Iterator for Map<I, F>
where
    F: FnMut(I::Item) -> B,
{
    type Item = B;
    fn next(&mut self) -> Option<B> {
        match self.iter.next() {
            Option::Some(x) => Option::Some((self.f)(x)),
            Option::None => Option::None,
        }
    }
}

pub struct Filter<I, P> {
    iter: I,
    predicate: P,
}
impl<I: Iterator, P> Iterator for Filter<I, P>
where
    P: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        while let Option::Some(x) = self.iter.next() {
            if (self.predicate)(&x) {
                return Option::Some(x);
            }
        }
        Option::None
    }
}

pub struct Enumerate<I> {
    iter: I,
    count: usize,
}
impl<I: Iterator> Iterator for Enumerate<I> {
    type Item = (usize, I::Item);
    fn next(&mut self) -> Option<(usize, I::Item)> {
        match self.iter.next() {
            Option::Some(a) => {
                let i = self.count;
                self.count += 1;
                Option::Some((i, a))
            }
            Option::None => Option::None,
        }
    }
}

pub struct Take<I> {
    iter: I,
    n: usize,
}
impl<I: Iterator> Iterator for Take<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        if self.n != 0 {
            self.n -= 1;
            self.iter.next()
        } else {
            Option::None
        }
    }
}

pub struct Skip<I> {
    iter: I,
    n: usize,
}
impl<I: Iterator> Iterator for Skip<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        while self.n > 0 {
            self.n -= 1;
            if let Option::None = self.iter.next() {
                return Option::None;
            }
        }
        self.iter.next()
    }
}

pub struct Zip<A, B> {
    a: A,
    b: B,
}
impl<A: Iterator, B: Iterator> Iterator for Zip<A, B> {
    type Item = (A::Item, B::Item);
    fn next(&mut self) -> Option<(A::Item, B::Item)> {
        let x = match self.a.next() {
            Option::Some(x) => x,
            Option::None => return Option::None,
        };
        let y = match self.b.next() {
            Option::Some(y) => y,
            Option::None => return Option::None,
        };
        Option::Some((x, y))
    }
}

pub struct Chain<A, B> {
    a: Option<A>,
    b: Option<B>,
}
impl<A: Iterator, B: Iterator<Item = A::Item>> Iterator for Chain<A, B> {
    type Item = A::Item;
    fn next(&mut self) -> Option<A::Item> {
        if let Option::Some(a) = &mut self.a {
            match a.next() {
                Option::None => self.a = Option::None,
                item => return item,
            }
        }
        match &mut self.b {
            Option::Some(b) => b.next(),
            Option::None => Option::None,
        }
    }
}

pub struct Peekable<I: Iterator> {
    iter: I,
    peeked: Option<Option<I::Item>>,
}
impl<I: Iterator> Iterator for Peekable<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        match self.peeked.take() {
            Option::Some(v) => v,
            Option::None => self.iter.next(),
        }
    }
}
impl<I: Iterator> Peekable<I> {
    pub fn peek(&mut self) -> Option<&I::Item> {
        if let Option::None = self.peeked {
            self.peeked = Option::Some(self.iter.next());
        }
        match &self.peeked {
            Option::Some(Option::Some(value)) => Option::Some(value),
            _ => Option::None,
        }
    }
}

pub struct Rev<I> {
    iter: I,
}
impl<I: DoubleEndedIterator> Iterator for Rev<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        self.iter.next_back()
    }
}

pub struct Cloned<I> {
    it: I,
}
impl<'a, I: Iterator<Item = &'a T>, T: 'a + crate::clone::Clone> Iterator for Cloned<I> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        match self.it.next() {
            Option::Some(x) => Option::Some(x.clone()),
            Option::None => Option::None,
        }
    }
}

pub struct Copied<I> {
    it: I,
}
impl<'a, I: Iterator<Item = &'a T>, T: 'a + crate::marker::Copy> Iterator for Copied<I> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        match self.it.next() {
            Option::Some(&x) => Option::Some(x),
            Option::None => Option::None,
        }
    }
}

pub struct Empty<T> {
    marker: crate::marker::PhantomData<T>,
}
pub fn empty<T>() -> Empty<T> {
    Empty {
        marker: crate::marker::PhantomData,
    }
}
impl<T> Iterator for Empty<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        Option::None
    }
}

pub struct Once<T> {
    inner: Option<T>,
}
pub fn once<T>(value: T) -> Once<T> {
    Once {
        inner: Option::Some(value),
    }
}
impl<T> Iterator for Once<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.inner.take()
    }
}

pub struct Repeat<A> {
    element: A,
}
pub fn repeat<T: crate::clone::Clone>(elt: T) -> Repeat<T> {
    Repeat { element: elt }
}
impl<A: crate::clone::Clone> Iterator for Repeat<A> {
    type Item = A;
    fn next(&mut self) -> Option<A> {
        Option::Some(self.element.clone())
    }
}

pub struct FromFn<F> {
    f: F,
}
pub fn from_fn<T, F>(f: F) -> FromFn<F>
where
    F: FnMut() -> Option<T>,
{
    FromFn { f }
}
impl<T, F> Iterator for FromFn<F>
where
    F: FnMut() -> Option<T>,
{
    type Item = T;
    fn next(&mut self) -> Option<T> {
        (self.f)()
    }
}

// Objects that have a notion of successor and predecessor (real core:
// iter/range.rs); the integer impls are in `step_impls.rs`.
pub trait Step: crate::clone::Clone + crate::cmp::PartialOrd + Sized {
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>);
    fn forward_checked(start: Self, count: usize) -> Option<Self>;
    fn backward_checked(start: Self, count: usize) -> Option<Self>;
    fn forward(start: Self, count: usize) -> Self {
        match Step::forward_checked(start, count) {
            Option::Some(next) => next,
            Option::None => core::panicking::panic("overflow in `Step::forward`"),
        }
    }
    unsafe fn forward_unchecked(start: Self, count: usize) -> Self {
        Step::forward(start, count)
    }
    fn backward(start: Self, count: usize) -> Self {
        match Step::backward_checked(start, count) {
            Option::Some(next) => next,
            Option::None => core::panicking::panic("overflow in `Step::backward`"),
        }
    }
    unsafe fn backward_unchecked(start: Self, count: usize) -> Self {
        Step::backward(start, count)
    }
}

impl<A: Step> Iterator for crate::ops::Range<A> {
    type Item = A;
    fn next(&mut self) -> Option<A> {
        if self.start < self.end {
            let current = self.start.clone();
            self.start = unsafe { Step::forward_unchecked(current.clone(), 1) };
            Option::Some(current)
        } else {
            Option::None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.start < self.end {
            Step::steps_between(&self.start, &self.end)
        } else {
            (0, Option::Some(0))
        }
    }
}
impl<A: Step> DoubleEndedIterator for crate::ops::Range<A> {
    fn next_back(&mut self) -> Option<A> {
        if self.start < self.end {
            self.end = unsafe { Step::backward_unchecked(self.end.clone(), 1) };
            Option::Some(self.end.clone())
        } else {
            Option::None
        }
    }
}
impl<A: Step> ExactSizeIterator for crate::ops::Range<A> {}
impl<A: Step> FusedIterator for crate::ops::Range<A> {}

impl<A: Step> Iterator for crate::ops::RangeFrom<A> {
    type Item = A;
    fn next(&mut self) -> Option<A> {
        let current = self.start.clone();
        self.start = Step::forward(current.clone(), 1);
        Option::Some(current)
    }
}
impl<A: Step> FusedIterator for crate::ops::RangeFrom<A> {}

impl<A: Step> Iterator for crate::ops::RangeInclusive<A> {
    type Item = A;
    fn next(&mut self) -> Option<A> {
        if self.exhausted || !(self.start <= self.end) {
            return Option::None;
        }
        if self.start < self.end {
            let current = self.start.clone();
            self.start = unsafe { Step::forward_unchecked(current.clone(), 1) };
            Option::Some(current)
        } else {
            self.exhausted = true;
            Option::Some(self.start.clone())
        }
    }
}
impl<A: Step> DoubleEndedIterator for crate::ops::RangeInclusive<A> {
    fn next_back(&mut self) -> Option<A> {
        if self.exhausted || !(self.start <= self.end) {
            return Option::None;
        }
        if self.start < self.end {
            let current = self.end.clone();
            self.end = unsafe { Step::backward_unchecked(current.clone(), 1) };
            Option::Some(current)
        } else {
            self.exhausted = true;
            Option::Some(self.end.clone())
        }
    }
}
impl<A: Step> FusedIterator for crate::ops::RangeInclusive<A> {}

// `Sum`/`Product` of the numeric types over items taken by value or by
// reference: one impl per numeric type, the item converted by `LrrToNum`.
pub trait LrrToNum<N> {
    fn lrr_to_num(self) -> N;
}
impl LrrToNum<i8> for i8 {
    fn lrr_to_num(self) -> i8 {
        self
    }
}
impl<'a> LrrToNum<i8> for &'a i8 {
    fn lrr_to_num(self) -> i8 {
        *self
    }
}
impl<A: LrrToNum<i8>> Sum<A> for i8 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> i8 {
        let mut total: i8 = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<i8>> Product<A> for i8 {
    fn product<I: Iterator<Item = A>>(iter: I) -> i8 {
        let mut total: i8 = 1;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
impl LrrToNum<i16> for i16 {
    fn lrr_to_num(self) -> i16 {
        self
    }
}
impl<'a> LrrToNum<i16> for &'a i16 {
    fn lrr_to_num(self) -> i16 {
        *self
    }
}
impl<A: LrrToNum<i16>> Sum<A> for i16 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> i16 {
        let mut total: i16 = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<i16>> Product<A> for i16 {
    fn product<I: Iterator<Item = A>>(iter: I) -> i16 {
        let mut total: i16 = 1;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
impl LrrToNum<i32> for i32 {
    fn lrr_to_num(self) -> i32 {
        self
    }
}
impl<'a> LrrToNum<i32> for &'a i32 {
    fn lrr_to_num(self) -> i32 {
        *self
    }
}
impl<A: LrrToNum<i32>> Sum<A> for i32 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> i32 {
        let mut total: i32 = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<i32>> Product<A> for i32 {
    fn product<I: Iterator<Item = A>>(iter: I) -> i32 {
        let mut total: i32 = 1;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
impl LrrToNum<i64> for i64 {
    fn lrr_to_num(self) -> i64 {
        self
    }
}
impl<'a> LrrToNum<i64> for &'a i64 {
    fn lrr_to_num(self) -> i64 {
        *self
    }
}
impl<A: LrrToNum<i64>> Sum<A> for i64 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> i64 {
        let mut total: i64 = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<i64>> Product<A> for i64 {
    fn product<I: Iterator<Item = A>>(iter: I) -> i64 {
        let mut total: i64 = 1;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
impl LrrToNum<isize> for isize {
    fn lrr_to_num(self) -> isize {
        self
    }
}
impl<'a> LrrToNum<isize> for &'a isize {
    fn lrr_to_num(self) -> isize {
        *self
    }
}
impl<A: LrrToNum<isize>> Sum<A> for isize {
    fn sum<I: Iterator<Item = A>>(iter: I) -> isize {
        let mut total: isize = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<isize>> Product<A> for isize {
    fn product<I: Iterator<Item = A>>(iter: I) -> isize {
        let mut total: isize = 1;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
impl LrrToNum<u8> for u8 {
    fn lrr_to_num(self) -> u8 {
        self
    }
}
impl<'a> LrrToNum<u8> for &'a u8 {
    fn lrr_to_num(self) -> u8 {
        *self
    }
}
impl<A: LrrToNum<u8>> Sum<A> for u8 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> u8 {
        let mut total: u8 = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<u8>> Product<A> for u8 {
    fn product<I: Iterator<Item = A>>(iter: I) -> u8 {
        let mut total: u8 = 1;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
impl LrrToNum<u16> for u16 {
    fn lrr_to_num(self) -> u16 {
        self
    }
}
impl<'a> LrrToNum<u16> for &'a u16 {
    fn lrr_to_num(self) -> u16 {
        *self
    }
}
impl<A: LrrToNum<u16>> Sum<A> for u16 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> u16 {
        let mut total: u16 = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<u16>> Product<A> for u16 {
    fn product<I: Iterator<Item = A>>(iter: I) -> u16 {
        let mut total: u16 = 1;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
impl LrrToNum<u32> for u32 {
    fn lrr_to_num(self) -> u32 {
        self
    }
}
impl<'a> LrrToNum<u32> for &'a u32 {
    fn lrr_to_num(self) -> u32 {
        *self
    }
}
impl<A: LrrToNum<u32>> Sum<A> for u32 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> u32 {
        let mut total: u32 = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<u32>> Product<A> for u32 {
    fn product<I: Iterator<Item = A>>(iter: I) -> u32 {
        let mut total: u32 = 1;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
impl LrrToNum<u64> for u64 {
    fn lrr_to_num(self) -> u64 {
        self
    }
}
impl<'a> LrrToNum<u64> for &'a u64 {
    fn lrr_to_num(self) -> u64 {
        *self
    }
}
impl<A: LrrToNum<u64>> Sum<A> for u64 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> u64 {
        let mut total: u64 = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<u64>> Product<A> for u64 {
    fn product<I: Iterator<Item = A>>(iter: I) -> u64 {
        let mut total: u64 = 1;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
impl LrrToNum<usize> for usize {
    fn lrr_to_num(self) -> usize {
        self
    }
}
impl<'a> LrrToNum<usize> for &'a usize {
    fn lrr_to_num(self) -> usize {
        *self
    }
}
impl<A: LrrToNum<usize>> Sum<A> for usize {
    fn sum<I: Iterator<Item = A>>(iter: I) -> usize {
        let mut total: usize = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<usize>> Product<A> for usize {
    fn product<I: Iterator<Item = A>>(iter: I) -> usize {
        let mut total: usize = 1;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
impl LrrToNum<f32> for f32 {
    fn lrr_to_num(self) -> f32 {
        self
    }
}
impl<'a> LrrToNum<f32> for &'a f32 {
    fn lrr_to_num(self) -> f32 {
        *self
    }
}
impl<A: LrrToNum<f32>> Sum<A> for f32 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> f32 {
        let mut total: f32 = -0.0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<f32>> Product<A> for f32 {
    fn product<I: Iterator<Item = A>>(iter: I) -> f32 {
        let mut total: f32 = 1.0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
impl LrrToNum<f64> for f64 {
    fn lrr_to_num(self) -> f64 {
        self
    }
}
impl<'a> LrrToNum<f64> for &'a f64 {
    fn lrr_to_num(self) -> f64 {
        *self
    }
}
impl<A: LrrToNum<f64>> Sum<A> for f64 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> f64 {
        let mut total: f64 = -0.0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<f64>> Product<A> for f64 {
    fn product<I: Iterator<Item = A>>(iter: I) -> f64 {
        let mut total: f64 = 1.0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total *= x.lrr_to_num();
        }
        total
    }
}
