// `core::marker`.
use crate::clone::Clone;

// Pinned library/core/src/marker.rs:145-192 (MIT OR Apache-2.0).
// Compiler-role and semantic attributes are retained as declaration metadata;
// stability/diagnostic prose is supplied by the licensed original in vendor.
#[lang = "sized"]
#[fundamental]
#[rustc_specialization_trait]
#[rustc_deny_explicit_impl]
#[rustc_dyn_incompatible_trait]
#[rustc_coinductive]
pub trait Sized: MetaSized {}
#[lang = "meta_sized"]
#[fundamental]
#[rustc_specialization_trait]
#[rustc_deny_explicit_impl]
#[rustc_coinductive]
pub trait MetaSized: PointeeSized {}
#[lang = "pointee_sized"]
#[fundamental]
#[rustc_specialization_trait]
#[rustc_deny_explicit_impl]
#[rustc_coinductive]
pub trait PointeeSized {}
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
