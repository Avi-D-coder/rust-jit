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
    fn filter_map<B, F>(self, f: F) -> FilterMap<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> Option<B>,
    {
        FilterMap { iter: self, f }
    }
    fn inspect<F>(self, f: F) -> Inspect<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item),
    {
        Inspect { iter: self, f }
    }
    fn skip_while<P>(self, predicate: P) -> SkipWhile<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        SkipWhile {
            iter: self,
            flag: false,
            predicate,
        }
    }
    fn take_while<P>(self, predicate: P) -> TakeWhile<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        TakeWhile {
            iter: self,
            flag: false,
            predicate,
        }
    }
    fn map_while<B, P>(self, predicate: P) -> MapWhile<Self, P>
    where
        Self: Sized,
        P: FnMut(Self::Item) -> Option<B>,
    {
        MapWhile {
            iter: self,
            predicate,
        }
    }
    fn scan<St, B, F>(self, initial_state: St, f: F) -> Scan<Self, St, F>
    where
        Self: Sized,
        F: FnMut(&mut St, Self::Item) -> Option<B>,
    {
        Scan {
            iter: self,
            f,
            state: initial_state,
        }
    }
    fn fuse(self) -> Fuse<Self>
    where
        Self: Sized,
    {
        Fuse {
            iter: Option::Some(self),
        }
    }
    fn step_by(self, step: usize) -> StepBy<Self>
    where
        Self: Sized,
    {
        if step == 0 {
            crate::panicking::panic("assertion failed: step != 0");
        }
        StepBy {
            iter: self,
            step: step - 1,
            first_take: true,
        }
    }
    fn flat_map<U, F>(self, f: F) -> FlatMap<Self, U, F>
    where
        Self: Sized,
        U: IntoIterator,
        F: FnMut(Self::Item) -> U,
    {
        FlatMap {
            iter: self,
            f,
            front: Option::None,
            back: Option::None,
        }
    }
    fn flatten(self) -> Flatten<Self>
    where
        Self: Sized,
        Self::Item: IntoIterator,
    {
        Flatten {
            iter: self,
            front: Option::None,
            back: Option::None,
        }
    }
    fn cycle(self) -> Cycle<Self>
    where
        Self: Sized + crate::clone::Clone,
    {
        Cycle {
            orig: self.clone(),
            iter: self,
        }
    }
    fn find_map<B, F>(&mut self, f: F) -> Option<B>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> Option<B>,
    {
        let mut f = f;
        while let Option::Some(x) = self.next() {
            if let Option::Some(b) = f(x) {
                return Option::Some(b);
            }
        }
        Option::None
    }
    fn reduce<F>(self, f: F) -> Option<Self::Item>
    where
        Self: Sized,
        F: FnMut(Self::Item, Self::Item) -> Self::Item,
    {
        let mut iter = self;
        let mut f = f;
        let mut accum = match iter.next() {
            Option::Some(first) => first,
            Option::None => return Option::None,
        };
        while let Option::Some(x) = iter.next() {
            accum = f(accum, x);
        }
        Option::Some(accum)
    }
    fn max_by_key<B, F>(self, f: F) -> Option<Self::Item>
    where
        Self: Sized,
        B: crate::cmp::Ord,
        F: FnMut(&Self::Item) -> B,
    {
        let mut iter = self;
        let mut f = f;
        let mut best = match iter.next() {
            Option::Some(first) => first,
            Option::None => return Option::None,
        };
        let mut best_key = f(&best);
        while let Option::Some(x) = iter.next() {
            let key = f(&x);
            if crate::cmp::Ord::cmp(&key, &best_key) != crate::cmp::Ordering::Less {
                best = x;
                best_key = key;
            }
        }
        Option::Some(best)
    }
    fn min_by_key<B, F>(self, f: F) -> Option<Self::Item>
    where
        Self: Sized,
        B: crate::cmp::Ord,
        F: FnMut(&Self::Item) -> B,
    {
        let mut iter = self;
        let mut f = f;
        let mut best = match iter.next() {
            Option::Some(first) => first,
            Option::None => return Option::None,
        };
        let mut best_key = f(&best);
        while let Option::Some(x) = iter.next() {
            let key = f(&x);
            if crate::cmp::Ord::cmp(&key, &best_key) == crate::cmp::Ordering::Less {
                best = x;
                best_key = key;
            }
        }
        Option::Some(best)
    }
    fn max_by<F>(self, compare: F) -> Option<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> crate::cmp::Ordering,
    {
        let mut iter = self;
        let mut compare = compare;
        let mut best = match iter.next() {
            Option::Some(first) => first,
            Option::None => return Option::None,
        };
        while let Option::Some(x) = iter.next() {
            if compare(&best, &x) != crate::cmp::Ordering::Greater {
                best = x;
            }
        }
        Option::Some(best)
    }
    fn min_by<F>(self, compare: F) -> Option<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> crate::cmp::Ordering,
    {
        let mut iter = self;
        let mut compare = compare;
        let mut best = match iter.next() {
            Option::Some(first) => first,
            Option::None => return Option::None,
        };
        while let Option::Some(x) = iter.next() {
            if compare(&best, &x) == crate::cmp::Ordering::Greater {
                best = x;
            }
        }
        Option::Some(best)
    }
    fn partition<B, F>(self, f: F) -> (B, B)
    where
        Self: Sized,
        B: crate::default::Default + Extend<Self::Item>,
        F: FnMut(&Self::Item) -> bool,
    {
        let mut iter = self;
        let mut f = f;
        let mut left: B = crate::default::Default::default();
        let mut right: B = crate::default::Default::default();
        while let Option::Some(x) = iter.next() {
            if f(&x) {
                left.extend(once(x));
            } else {
                right.extend(once(x));
            }
        }
        (left, right)
    }
    fn unzip<A, B, FromA, FromB>(self) -> (FromA, FromB)
    where
        FromA: crate::default::Default + Extend<A>,
        FromB: crate::default::Default + Extend<B>,
        Self: Sized + Iterator<Item = (A, B)>,
    {
        let mut iter = self;
        let mut left: FromA = crate::default::Default::default();
        let mut right: FromB = crate::default::Default::default();
        while let Option::Some((a, b)) = iter.next() {
            left.extend(once(a));
            right.extend(once(b));
        }
        (left, right)
    }
    fn rposition<P>(&mut self, predicate: P) -> Option<usize>
    where
        P: FnMut(Self::Item) -> bool,
        Self: Sized + ExactSizeIterator + DoubleEndedIterator,
    {
        let mut predicate = predicate;
        let mut i = self.len();
        while let Option::Some(x) = self.next_back() {
            i -= 1;
            if predicate(x) {
                return Option::Some(i);
            }
        }
        Option::None
    }
}

pub trait DoubleEndedIterator: Iterator {
    fn next_back(&mut self) -> Option<Self::Item>;
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let mut n = n;
        while n > 0 {
            if let Option::None = self.next_back() {
                return Option::None;
            }
            n -= 1;
        }
        self.next_back()
    }
    fn rfold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let mut iter = self;
        let mut f = f;
        let mut accum = init;
        while let Option::Some(x) = iter.next_back() {
            accum = f(accum, x);
        }
        accum
    }
    fn rfind<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        let mut predicate = predicate;
        while let Option::Some(x) = self.next_back() {
            if predicate(&x) {
                return Option::Some(x);
            }
        }
        Option::None
    }
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.n == 0 {
            return (0, Option::Some(0));
        }
        let (lower, upper) = self.iter.size_hint();
        let lower = if lower < self.n { lower } else { self.n };
        let upper = match upper {
            Option::Some(x) if x < self.n => Option::Some(x),
            _ => Option::Some(self.n),
        };
        (lower, upper)
    }
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();
        let lower = if lower > self.n { lower - self.n } else { 0 };
        let upper = match upper {
            Option::Some(x) => Option::Some(if x > self.n { x - self.n } else { 0 }),
            Option::None => Option::None,
        };
        (lower, upper)
    }
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (a_lower, a_upper) = self.a.size_hint();
        let (b_lower, b_upper) = self.b.size_hint();
        let lower = if a_lower < b_lower { a_lower } else { b_lower };
        let upper = match (a_upper, b_upper) {
            (Option::Some(x), Option::Some(y)) => Option::Some(if x < y { x } else { y }),
            (Option::Some(x), Option::None) => Option::Some(x),
            (Option::None, Option::Some(y)) => Option::Some(y),
            (Option::None, Option::None) => Option::None,
        };
        (lower, upper)
    }
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (a_lower, a_upper) = match &self.a {
            Option::Some(a) => a.size_hint(),
            Option::None => (0, Option::Some(0)),
        };
        let (b_lower, b_upper) = match &self.b {
            Option::Some(b) => b.size_hint(),
            Option::None => (0, Option::Some(0)),
        };
        let upper = match (a_upper, b_upper) {
            (Option::Some(x), Option::Some(y)) => Option::Some(x + y),
            _ => Option::None,
        };
        (a_lower + b_lower, upper)
    }
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        let peek_len = match &self.peeked {
            Option::Some(Option::None) => return (0, Option::Some(0)),
            Option::Some(Option::Some(_)) => 1,
            Option::None => 0,
        };
        let (lower, upper) = self.iter.size_hint();
        let upper = match upper {
            Option::Some(x) => Option::Some(x + peek_len),
            Option::None => Option::None,
        };
        (lower + peek_len, upper)
    }
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
    fn next(&mut self) -> Option<I::Item> {
        self.iter.next_back()
    }
}

pub struct Cloned<I> {
    it: I,
}
impl<'a, I: Iterator<Item = &'a T>, T: 'a + crate::clone::Clone> Iterator for Cloned<I> {
    type Item = T;
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
    fn next(&mut self) -> Option<T> {
        match self.it.next() {
            Option::Some(&x) => Option::Some(x),
            Option::None => Option::None,
        }
    }
}

// Adapters beyond the first set. Each is lazy exactly as in core: nothing is
// pulled from the source until `next` (or `next_back`) is called.
pub struct FilterMap<I, F> {
    iter: I,
    f: F,
}
impl<B, I: Iterator, F> Iterator for FilterMap<I, F>
where
    F: FnMut(I::Item) -> Option<B>,
{
    type Item = B;
    fn next(&mut self) -> Option<B> {
        while let Option::Some(x) = self.iter.next() {
            if let Option::Some(y) = (self.f)(x) {
                return Option::Some(y);
            }
        }
        Option::None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
}
impl<B, I: DoubleEndedIterator, F> DoubleEndedIterator for FilterMap<I, F>
where
    F: FnMut(I::Item) -> Option<B>,
{
    fn next_back(&mut self) -> Option<B> {
        while let Option::Some(x) = self.iter.next_back() {
            if let Option::Some(y) = (self.f)(x) {
                return Option::Some(y);
            }
        }
        Option::None
    }
}

pub struct Inspect<I, F> {
    iter: I,
    f: F,
}
impl<I: Iterator, F> Iterator for Inspect<I, F>
where
    F: FnMut(&I::Item),
{
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        let next = self.iter.next();
        if let Option::Some(x) = &next {
            (self.f)(x);
        }
        next
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}
impl<I: DoubleEndedIterator, F> DoubleEndedIterator for Inspect<I, F>
where
    F: FnMut(&I::Item),
{
    fn next_back(&mut self) -> Option<I::Item> {
        let next = self.iter.next_back();
        if let Option::Some(x) = &next {
            (self.f)(x);
        }
        next
    }
}
impl<I: ExactSizeIterator, F> ExactSizeIterator for Inspect<I, F> where F: FnMut(&I::Item) {}

pub struct SkipWhile<I, P> {
    iter: I,
    flag: bool,
    predicate: P,
}
impl<I: Iterator, P> Iterator for SkipWhile<I, P>
where
    P: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        while let Option::Some(x) = self.iter.next() {
            if self.flag || !(self.predicate)(&x) {
                self.flag = true;
                return Option::Some(x);
            }
        }
        Option::None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
}

pub struct TakeWhile<I, P> {
    iter: I,
    flag: bool,
    predicate: P,
}
impl<I: Iterator, P> Iterator for TakeWhile<I, P>
where
    P: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        if self.flag {
            return Option::None;
        }
        match self.iter.next() {
            Option::Some(x) => {
                if (self.predicate)(&x) {
                    Option::Some(x)
                } else {
                    self.flag = true;
                    Option::None
                }
            }
            Option::None => Option::None,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.flag {
            (0, Option::Some(0))
        } else {
            let (_, upper) = self.iter.size_hint();
            (0, upper)
        }
    }
}

pub struct MapWhile<I, P> {
    iter: I,
    predicate: P,
}
impl<B, I: Iterator, P> Iterator for MapWhile<I, P>
where
    P: FnMut(I::Item) -> Option<B>,
{
    type Item = B;
    fn next(&mut self) -> Option<B> {
        match self.iter.next() {
            Option::Some(x) => (self.predicate)(x),
            Option::None => Option::None,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
}

pub struct Scan<I, St, F> {
    iter: I,
    f: F,
    state: St,
}
impl<B, I: Iterator, St, F> Iterator for Scan<I, St, F>
where
    F: FnMut(&mut St, I::Item) -> Option<B>,
{
    type Item = B;
    fn next(&mut self) -> Option<B> {
        match self.iter.next() {
            Option::Some(a) => (self.f)(&mut self.state, a),
            Option::None => Option::None,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
}

pub struct Fuse<I> {
    iter: Option<I>,
}
impl<I: Iterator> Iterator for Fuse<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        match &mut self.iter {
            Option::Some(iter) => match iter.next() {
                Option::None => {
                    self.iter = Option::None;
                    Option::None
                }
                item => item,
            },
            Option::None => Option::None,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.iter {
            Option::Some(iter) => iter.size_hint(),
            Option::None => (0, Option::Some(0)),
        }
    }
}
impl<I: DoubleEndedIterator> DoubleEndedIterator for Fuse<I> {
    fn next_back(&mut self) -> Option<I::Item> {
        match &mut self.iter {
            Option::Some(iter) => match iter.next_back() {
                Option::None => {
                    self.iter = Option::None;
                    Option::None
                }
                item => item,
            },
            Option::None => Option::None,
        }
    }
}
impl<I: ExactSizeIterator> ExactSizeIterator for Fuse<I> {}
impl<I: Iterator> FusedIterator for Fuse<I> {}

pub struct StepBy<I> {
    iter: I,
    step: usize,
    first_take: bool,
}
impl<I: Iterator> Iterator for StepBy<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        let step_size = if self.first_take { 0 } else { self.step };
        self.first_take = false;
        self.iter.nth(step_size)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (low, high) = self.iter.size_hint();
        let step = self.step + 1;
        if self.first_take {
            let scale = |n: usize| if n == 0 { 0 } else { 1 + (n - 1) / step };
            (scale(low), match high {
                Option::Some(n) => Option::Some(scale(n)),
                Option::None => Option::None,
            })
        } else {
            (low / step, match high {
                Option::Some(n) => Option::Some(n / step),
                Option::None => Option::None,
            })
        }
    }
}

impl<I: ExactSizeIterator> ExactSizeIterator for StepBy<I> {}

pub struct FlatMap<I, U: IntoIterator, F> {
    iter: I,
    f: F,
    front: Option<U::IntoIter>,
    back: Option<U::IntoIter>,
}
impl<I: Iterator, U: IntoIterator, F> Iterator for FlatMap<I, U, F>
where
    F: FnMut(I::Item) -> U,
{
    type Item = U::Item;
    fn next(&mut self) -> Option<U::Item> {
        loop {
            if let Option::Some(inner) = &mut self.front {
                match inner.next() {
                    Option::None => self.front = Option::None,
                    elt => return elt,
                }
            }
            match self.iter.next() {
                Option::Some(x) => {
                    self.front = Option::Some((self.f)(x).into_iter());
                }
                Option::None => {
                    return match &mut self.back {
                        Option::Some(inner) => match inner.next() {
                            Option::None => {
                                self.back = Option::None;
                                Option::None
                            }
                            elt => elt,
                        },
                        Option::None => Option::None,
                    };
                }
            }
        }
    }
}
impl<I: DoubleEndedIterator, U: IntoIterator, F> DoubleEndedIterator for FlatMap<I, U, F>
where
    F: FnMut(I::Item) -> U,
    U::IntoIter: DoubleEndedIterator,
{
    fn next_back(&mut self) -> Option<U::Item> {
        loop {
            if let Option::Some(inner) = &mut self.back {
                match inner.next_back() {
                    Option::None => self.back = Option::None,
                    elt => return elt,
                }
            }
            match self.iter.next_back() {
                Option::Some(x) => {
                    self.back = Option::Some((self.f)(x).into_iter());
                }
                Option::None => {
                    return match &mut self.front {
                        Option::Some(inner) => match inner.next_back() {
                            Option::None => {
                                self.front = Option::None;
                                Option::None
                            }
                            elt => elt,
                        },
                        Option::None => Option::None,
                    };
                }
            }
        }
    }
}

pub struct Flatten<I: Iterator>
where
    I::Item: IntoIterator,
{
    iter: I,
    front: Option<<I::Item as IntoIterator>::IntoIter>,
    back: Option<<I::Item as IntoIterator>::IntoIter>,
}
impl<I: Iterator> Iterator for Flatten<I>
where
    I::Item: IntoIterator,
{
    type Item = <I::Item as IntoIterator>::Item;
    fn next(&mut self) -> Option<<I::Item as IntoIterator>::Item> {
        loop {
            if let Option::Some(inner) = &mut self.front {
                match inner.next() {
                    Option::None => self.front = Option::None,
                    elt => return elt,
                }
            }
            match self.iter.next() {
                Option::Some(x) => {
                    self.front = Option::Some(x.into_iter());
                }
                Option::None => {
                    return match &mut self.back {
                        Option::Some(inner) => match inner.next() {
                            Option::None => {
                                self.back = Option::None;
                                Option::None
                            }
                            elt => elt,
                        },
                        Option::None => Option::None,
                    };
                }
            }
        }
    }
}
impl<I: DoubleEndedIterator> DoubleEndedIterator for Flatten<I>
where
    I::Item: IntoIterator,
    <I::Item as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    fn next_back(&mut self) -> Option<<I::Item as IntoIterator>::Item> {
        loop {
            if let Option::Some(inner) = &mut self.back {
                match inner.next_back() {
                    Option::None => self.back = Option::None,
                    elt => return elt,
                }
            }
            match self.iter.next_back() {
                Option::Some(x) => {
                    self.back = Option::Some(x.into_iter());
                }
                Option::None => {
                    return match &mut self.front {
                        Option::Some(inner) => match inner.next_back() {
                            Option::None => {
                                self.front = Option::None;
                                Option::None
                            }
                            elt => elt,
                        },
                        Option::None => Option::None,
                    };
                }
            }
        }
    }
}

pub struct Cycle<I> {
    orig: I,
    iter: I,
}
impl<I: crate::clone::Clone + Iterator> Iterator for Cycle<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<I::Item> {
        match self.iter.next() {
            Option::None => {
                self.iter = self.orig.clone();
                self.iter.next()
            }
            y => y,
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.orig.size_hint() {
            (0, Option::Some(0)) => (0, Option::Some(0)),
            (0, _) => (0, Option::None),
            _ => (usize::MAX, Option::None),
        }
    }
}

// Double-ended and exact-size forms of the first adapter set.
impl<B, I: DoubleEndedIterator, F> DoubleEndedIterator for Map<I, F>
where
    F: FnMut(I::Item) -> B,
{
    fn next_back(&mut self) -> Option<B> {
        match self.iter.next_back() {
            Option::Some(x) => Option::Some((self.f)(x)),
            Option::None => Option::None,
        }
    }
}
impl<B, I: ExactSizeIterator, F> ExactSizeIterator for Map<I, F> where F: FnMut(I::Item) -> B {}
impl<I: DoubleEndedIterator, P> DoubleEndedIterator for Filter<I, P>
where
    P: FnMut(&I::Item) -> bool,
{
    fn next_back(&mut self) -> Option<I::Item> {
        while let Option::Some(x) = self.iter.next_back() {
            if (self.predicate)(&x) {
                return Option::Some(x);
            }
        }
        Option::None
    }
}
impl<I: ExactSizeIterator> ExactSizeIterator for Enumerate<I> {}
impl<I: ExactSizeIterator + DoubleEndedIterator> DoubleEndedIterator for Enumerate<I> {
    fn next_back(&mut self) -> Option<(usize, I::Item)> {
        match self.iter.next_back() {
            Option::Some(a) => {
                let len = self.iter.len();
                Option::Some((self.count + len, a))
            }
            Option::None => Option::None,
        }
    }
}
impl<I: ExactSizeIterator> ExactSizeIterator for Take<I> {}
impl<I: ExactSizeIterator> ExactSizeIterator for Skip<I> {}
impl<A: ExactSizeIterator, B: ExactSizeIterator> ExactSizeIterator for Zip<A, B> {}
impl<A: ExactSizeIterator + DoubleEndedIterator, B: ExactSizeIterator + DoubleEndedIterator>
    DoubleEndedIterator for Zip<A, B>
{
    fn next_back(&mut self) -> Option<(A::Item, B::Item)> {
        let a_len = self.a.len();
        let b_len = self.b.len();
        if a_len > b_len {
            let mut extra = a_len - b_len;
            while extra > 0 {
                self.a.next_back();
                extra -= 1;
            }
        } else if b_len > a_len {
            let mut extra = b_len - a_len;
            while extra > 0 {
                self.b.next_back();
                extra -= 1;
            }
        }
        match self.a.next_back() {
            Option::Some(x) => match self.b.next_back() {
                Option::Some(y) => Option::Some((x, y)),
                Option::None => Option::None,
            },
            Option::None => Option::None,
        }
    }
}
impl<A: DoubleEndedIterator, B: DoubleEndedIterator<Item = A::Item>> DoubleEndedIterator
    for Chain<A, B>
{
    fn next_back(&mut self) -> Option<A::Item> {
        if let Option::Some(b) = &mut self.b {
            match b.next_back() {
                Option::None => self.b = Option::None,
                item => return item,
            }
        }
        match &mut self.a {
            Option::Some(a) => a.next_back(),
            Option::None => Option::None,
        }
    }
}
impl<I: DoubleEndedIterator> DoubleEndedIterator for Rev<I> {
    fn next_back(&mut self) -> Option<I::Item> {
        self.iter.next()
    }
}
impl<I: ExactSizeIterator + DoubleEndedIterator> ExactSizeIterator for Rev<I> {}
impl<'a, I: DoubleEndedIterator<Item = &'a T>, T: 'a + crate::clone::Clone> DoubleEndedIterator
    for Cloned<I>
{
    fn next_back(&mut self) -> Option<T> {
        match self.it.next_back() {
            Option::Some(x) => Option::Some(x.clone()),
            Option::None => Option::None,
        }
    }
}
impl<'a, I: DoubleEndedIterator<Item = &'a T>, T: 'a + crate::marker::Copy> DoubleEndedIterator
    for Copied<I>
{
    fn next_back(&mut self) -> Option<T> {
        match self.it.next_back() {
            Option::Some(&x) => Option::Some(x),
            Option::None => Option::None,
        }
    }
}
impl<'a, I: ExactSizeIterator<Item = &'a T>, T: 'a + crate::clone::Clone> ExactSizeIterator
    for Cloned<I>
{
}
impl<'a, I: ExactSizeIterator<Item = &'a T>, T: 'a + crate::marker::Copy> ExactSizeIterator
    for Copied<I>
{
}
impl<I: DoubleEndedIterator + ?Sized> DoubleEndedIterator for &mut I {
    fn next_back(&mut self) -> Option<I::Item> {
        (**self).next_back()
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
impl LrrToNum<i128> for i128 {
    fn lrr_to_num(self) -> i128 {
        self
    }
}
impl<'a> LrrToNum<i128> for &'a i128 {
    fn lrr_to_num(self) -> i128 {
        *self
    }
}
impl<A: LrrToNum<i128>> Sum<A> for i128 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> i128 {
        let mut total: i128 = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<i128>> Product<A> for i128 {
    fn product<I: Iterator<Item = A>>(iter: I) -> i128 {
        let mut total: i128 = 1;
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
impl LrrToNum<u128> for u128 {
    fn lrr_to_num(self) -> u128 {
        self
    }
}
impl<'a> LrrToNum<u128> for &'a u128 {
    fn lrr_to_num(self) -> u128 {
        *self
    }
}
impl<A: LrrToNum<u128>> Sum<A> for u128 {
    fn sum<I: Iterator<Item = A>>(iter: I) -> u128 {
        let mut total: u128 = 0;
        let mut it = iter;
        while let Option::Some(x) = it.next() {
            total += x.lrr_to_num();
        }
        total
    }
}
impl<A: LrrToNum<u128>> Product<A> for u128 {
    fn product<I: Iterator<Item = A>>(iter: I) -> u128 {
        let mut total: u128 = 1;
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
