// `alloc::borrow`.
pub use core::borrow::{Borrow, BorrowMut};

pub trait ToOwned {
    type Owned: Borrow<Self>;
    fn to_owned(&self) -> Self::Owned;
    fn clone_into(&self, target: &mut Self::Owned) {
        *target = self.to_owned();
    }
}

impl<T: Clone> ToOwned for T {
    type Owned = T;
    fn to_owned(&self) -> T {
        self.clone()
    }
}

pub enum Cow<'a, B: ?Sized + 'a + ToOwned> {
    Borrowed(&'a B),
    Owned(<B as ToOwned>::Owned),
}
