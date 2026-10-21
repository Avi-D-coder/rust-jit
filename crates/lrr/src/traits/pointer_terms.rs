//! Source terms of raw pointers and fn pointers (E-R5-008).
//!
//! `TyTerm` has no raw-pointer variant. `*const T` / `*mut T` is the
//! reserved-identity nominal `RawPtr<T>` (the way a closure's callable
//! identity is a nominal-like term), so substitution, unification, impl
//! matching and open type arguments reach it through the ordinary `Nominal`
//! arms. `fn(A) -> R` is `TyTerm::Function`. Actual source terms can retain
//! qualifiers; legacy runtime construction below carries no qualifier evidence.
use super::term::{Binder, FunctionSignature, GenericArg, TyTerm};
use crate::core::digest::Digest;
use std::sync::OnceLock;

/// The reserved identity of `*const T`.
pub fn const_pointer_identity() -> Digest {
    static IDENTITY: OnceLock<Digest> = OnceLock::new();
    *IDENTITY.get_or_init(|| Digest::of_bytes(b"lrr/v1/builtin-raw-pointer/const"))
}
/// The reserved identity of `*mut T`.
pub fn mut_pointer_identity() -> Digest {
    static IDENTITY: OnceLock<Digest> = OnceLock::new();
    *IDENTITY.get_or_init(|| Digest::of_bytes(b"lrr/v1/builtin-raw-pointer/mut"))
}
/// The reserved impl-self identity of every fn pointer type (`impl .. for
/// fn(A) -> R`); fn pointers are `TyTerm::Function`, not nominal.
pub fn fn_pointer_identity() -> Digest {
    static IDENTITY: OnceLock<Digest> = OnceLock::new();
    *IDENTITY.get_or_init(|| Digest::of_bytes(b"lrr/v1/builtin-fn-pointer"))
}
/// `*const pointee` or `*mut pointee`.
pub fn raw_pointer(mutable: bool, pointee: TyTerm) -> TyTerm {
    TyTerm::Nominal(
        if mutable {
            mut_pointer_identity()
        } else {
            const_pointer_identity()
        },
        vec![GenericArg::Type(pointee)],
    )
}
/// Whether both identities are raw-pointer identities (`*const` or `*mut`).
pub fn both_pointer_identities(a: &Digest, b: &Digest) -> bool {
    let is_pointer = |identity: &Digest| {
        *identity == const_pointer_identity() || *identity == mut_pointer_identity()
    };
    is_pointer(a) && is_pointer(b)
}
/// The mutability and pointee of a raw pointer term.
pub fn as_raw_pointer(term: &TyTerm) -> Option<(bool, &TyTerm)> {
    let TyTerm::Nominal(identity, arguments) = term else {
        return None;
    };
    let mutable = if *identity == mut_pointer_identity() {
        true
    } else if *identity == const_pointer_identity() {
        false
    } else {
        return None;
    };
    match arguments.as_slice() {
        [GenericArg::Type(pointee)] => Some((mutable, pointee)),
        _ => None,
    }
}
/// Whether two source types are both raw pointers or both fn pointers. A
/// pointer result or location accepts any pointer of its family: rustc has
/// already proven the types agree up to coercion (E-R5-008).
pub fn same_pointer_family(a: &TyTerm, b: &TyTerm) -> bool {
    (as_raw_pointer(a).is_some() && as_raw_pointer(b).is_some())
        || (matches!(a, TyTerm::Function(_)) && matches!(b, TyTerm::Function(_)))
}
/// Whether both are references to fn pointers of one mutability: an operator
/// impl for `fn()` serves every fn-pointer signature (E-R5-008).
pub fn same_function_pointer_reference(a: &TyTerm, b: &TyTerm) -> bool {
    matches!(
        (a, b),
        (TyTerm::Ref(_, m, x), TyTerm::Ref(_, n, y))
            if m == n && matches!(x.as_ref(), TyTerm::Function(_)) && matches!(y.as_ref(), TyTerm::Function(_))
    )
}
/// `fn(parameters) -> result`.
pub fn fn_pointer(parameters: Vec<TyTerm>, result: TyTerm) -> TyTerm {
    TyTerm::Function(Box::new(Binder {
        variables: Vec::new(),
        value: FunctionSignature {
            parameters,
            result: Box::new(result),
            source_header: None,
        },
    }))
}
