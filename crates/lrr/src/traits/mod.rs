//! Symbolic source foundations and the pinned upstream solver bridge.
//! Concrete source/inference delegate integration remains incomplete.
pub(crate) mod upstream;
pub mod term;
pub mod pointer_terms;
pub mod unify;

pub mod subst;

pub mod header;
pub mod index;

pub mod header_syntax;

pub mod predicate;

pub(crate) mod trait_catalog;
