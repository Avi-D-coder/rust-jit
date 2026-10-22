//! Owned source call data; inference and caller ParamEnv construction are separate.
//! Engine values/slots and their commit guards stay with the reached call owner.
use crate::{
    core::digest::Digest,
    traits::term::{InferVarKey, ParamKey, TyTerm},
};
use std::collections::BTreeMap;

pub(crate) use super::model::Span as SourceCallSite;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceVariableFamily {
    Type,
    Integer,
    Float,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceCallRelationKind {
    Equal,
    /// Argument value (actual) coerces to the declared parameter (formal).
    Coerce,
    /// Declared callee return (formal) coerces to the caller's expectation
    /// (actual). Keeping direction explicit avoids reversing result inference.
    CoerceResult,
}

/// Transport the actual relation; capturing it never performs unification.
#[derive(Debug, Clone)]
pub(crate) struct SourceCallRelation {
    pub(crate) formal: TyTerm,
    pub(crate) actual: TyTerm,
    pub(crate) kind: SourceCallRelationKind,
    pub(crate) site: SourceCallSite,
}

/// These source substitutions do not certify the caller's clauses or ParamEnv.
#[derive(Debug, Clone)]
pub(crate) struct CallerSourceContext {
    pub(crate) body_definition: Digest,
    pub(crate) generic_definition: Option<Digest>,
    pub(crate) signature: Option<Digest>,
    pub(crate) application: Option<Digest>,
    pub(crate) substitutions: BTreeMap<ParamKey, TyTerm>,
}

/// One selected generic function call inside a source component: its
/// callee and a variable for every generic Type parameter. The callee's
/// numeric-connected where-clauses are instantiated by exactly these.
#[derive(Debug, Clone)]
pub(crate) struct ComponentSiteInput {
    pub(crate) callee: Digest,
    pub(crate) signature: Digest,
    pub(crate) parameters: BTreeMap<ParamKey, TyTerm>,
    /// The callee's const generic arguments: component const variables.
    pub(crate) constants: BTreeMap<ParamKey, crate::traits::term::ConstTerm>,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum DeferredRelationKind {
    /// `result = *operand` of a builtin reference.
    Deref,
    /// Builtin arithmetic: both operands and the result share one Type.
    Arithmetic,
    /// Builtin comparison: both operands share one Type.
    Comparison,
    /// Builtin compound assignment: both operands share one Type.
    Compound,
    /// Builtin shift: the result has the left operand's Type.
    Shift,
}
/// A structural relation that fires only after earlier rows resolved the
/// operand Types (rustc's `structurally_resolve_type` points).
#[derive(Debug, Clone)]
pub(crate) struct DeferredRelation {
    pub(crate) kind: DeferredRelationKind,
    pub(crate) first: TyTerm,
    pub(crate) second: TyTerm,
    pub(crate) result: TyTerm,
    /// For a dereference: `<first as Deref>::Target`, the overloaded step a
    /// resolved non-reference operand takes.
    pub(crate) projection: Option<TyTerm>,
    pub(crate) site: SourceCallSite,
}
/// Source-component extras of a root packet. The owner (`ReachedSubject::
/// Component`) is the caller body itself; the relations are its source rows.
pub(crate) struct ComponentInput<'a> {
    pub(crate) deferred: &'a [DeferredRelation],
    pub(crate) sites: &'a [ComponentSiteInput],
    /// The component's unsuffixed numeric literal variables.
    pub(crate) literals: &'a [InferVarKey],
    /// The component's const inference variables (array lengths).
    pub(crate) const_variables: &'a [InferVarKey],
    /// The executing body's own const generic parameters (rigid).
    pub(crate) constants: &'a BTreeMap<ParamKey, crate::traits::term::ConstTerm>,
    /// No typed hole: the variables are closed, so rustc's fallback applies.
    pub(crate) closed: bool,
    /// `core::ops::Deref`: the trait whose impls an overloaded dereference
    /// reads (demanded before any row fires).
    pub(crate) deref_trait: Option<Digest>,
    /// `core::cmp::PartialEq`: a comparison of operands that are not both
    /// builtin scalars is the obligation `lhs: PartialEq<rhs>`.
    pub(crate) eq_trait: Option<Digest>,
    /// `core::iter::IntoIterator`: a `for` loop's pattern takes `<head as
    /// IntoIterator>::Item`.
    pub(crate) into_iter_trait: Option<Digest>,
    /// `core::ops::Index`: an index expression takes `<base as Index<idx>>::Output`.
    pub(crate) index_trait: Option<Digest>,
}

/// Borrowed from the immutable reached-call packet, never from guest slots.
pub(crate) struct SourceRootCallInput<'a> {
    pub(crate) callee: Digest,
    pub(crate) signature: Digest,
    pub(crate) parameters: &'a BTreeMap<ParamKey, TyTerm>,
    pub(crate) variables: &'a BTreeMap<InferVarKey, SourceVariableFamily>,
    pub(crate) relations: &'a [SourceCallRelation],
    pub(crate) caller: &'a CallerSourceContext,
    pub(crate) component: Option<&'a ComponentInput<'a>>,
}
