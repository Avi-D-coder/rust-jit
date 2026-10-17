// `core::marker`.
use crate::clone::Clone;

pub trait Sized {}
pub trait Unsize<T: ?Sized> {}
pub unsafe trait Send {}
pub unsafe trait Sync {}
pub trait Unpin {}
pub trait Copy: Clone {}
pub trait StructuralPartialEq {}
pub trait Tuple {}
pub trait Freeze {}
pub trait ConstParamTy {}
pub trait PointerLike {}
pub trait Destruct {}
pub trait FnPtr: Copy + Clone {}
pub trait DiscriminantKind {
    type Discriminant;
}

// Zero-sized marker owning a `T` for the type system.
pub struct PhantomData<T: ?Sized>;

impl<T: ?Sized> Clone for PhantomData<T> {
    fn clone(&self) -> Self {
        PhantomData
    }
}
impl<T: ?Sized> Copy for PhantomData<T> {}

pub struct PhantomPinned;
