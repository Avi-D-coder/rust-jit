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
    Coerce,
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
#[derive(Debug)]
pub(crate) struct CallerSourceContext {
    pub(crate) body_definition: Digest,
    pub(crate) generic_definition: Option<Digest>,
    pub(crate) signature: Option<Digest>,
    pub(crate) application: Option<Digest>,
    pub(crate) substitutions: BTreeMap<ParamKey, TyTerm>,
}

/// Borrowed from the immutable reached-call packet, never from guest slots.
pub(crate) struct SourceRootCallInput<'a> {
    pub(crate) callee: Digest,
    pub(crate) signature: Digest,
    pub(crate) parameters: &'a BTreeMap<ParamKey, TyTerm>,
    pub(crate) variables: &'a BTreeMap<InferVarKey, SourceVariableFamily>,
    pub(crate) relations: &'a [SourceCallRelation],
    pub(crate) caller: &'a CallerSourceContext,
}
