// `core::marker`.
use crate::clone::Clone;
use crate::cmp::{Eq, PartialEq};
use crate::fmt::Debug;
use crate::hash::Hash;

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
// Pinned marker.rs:92/657/1014: these are ordinary auto declarations.
#[rustc_diagnostic_item = "Send"]
pub unsafe auto trait Send {}
#[rustc_diagnostic_item = "Sync"]
#[lang = "sync"]
pub unsafe auto trait Sync {}
#[lang = "unpin"]
pub auto trait Unpin {}
pub trait Copy: Clone {}
pub trait StructuralPartialEq {}
pub trait Tuple {}
pub trait Freeze {}
pub trait ConstParamTy {}
pub trait PointerLike {}
pub trait Destruct {}
pub trait FnPtr: Copy + Clone {}
// Pinned library/core/src/marker.rs:880-887 (MIT OR Apache-2.0).
// Actual source obligations; builtin normalization remains the pinned solver.
#[lang = "discriminant_kind"]
#[rustc_deny_explicit_impl]
#[rustc_dyn_incompatible_trait]
pub trait DiscriminantKind {
    #[lang = "discriminant_type"]
    type Discriminant: Clone + Copy + Debug + Eq + PartialEq + Hash + Send + Sync + Unpin;
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
