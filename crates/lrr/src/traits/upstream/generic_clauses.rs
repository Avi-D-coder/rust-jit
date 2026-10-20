//! Pinned GenericClauses instantiation; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18.
//! Generated/audited by tools/upstream_generic_clauses.py.
use super::{
    identity::DefinitionId,
    model,
    prepared::{Cx, PreparedProvider},
};
use rustc_type_ir::{EarlyBinder, Unnormalized};
use std::marker::PhantomData;
use tracing::instrument;
type Clause<'cx> = model::Clause<'cx, PreparedProvider<'cx>>;
type Args<'cx> = model::Args<'cx, PreparedProvider<'cx>>;
type Span = model::Span;

/// Query identity is a type boundary. An explicit-source bundle cannot stand
/// in for compiler full predicates merely because their storage shapes match.
pub(super) trait ClauseQuery: Copy {}
#[derive(Clone, Copy, Debug)]
pub(super) struct ExplicitOwnClauses;
impl ClauseQuery for ExplicitOwnClauses {}
#[derive(Clone, Copy, Debug)]
pub(super) struct FullClauses;
impl ClauseQuery for FullClauses {}

/// The actual source bundle owns the authenticated parent receipts and encoded
/// clauses. This callback reads that immutable retained map, never live source
/// queries or a partially populated SourceFacts provider.
pub(super) trait ClauseParents<'data, 'cx>: Copy {
    type Query: ClauseQuery;
    fn clauses_of(self, definition: DefinitionId) -> GenericClauses<'data, 'cx, Self::Query>;
}

/// Same compiler query payload with a separate borrow lifetime for an owned
/// source bundle. No Default: absence must be established by its real producer.
#[derive(Clone, Copy, Debug)]
pub(super) struct GenericClauses<'data, 'cx, Q: ClauseQuery> {
    parent: Option<DefinitionId>,
    clauses: &'data [(Clause<'cx>, Span)],
    query: PhantomData<Q>,
}
impl<'data, 'cx, Q: ClauseQuery> GenericClauses<'data, 'cx, Q> {
    /// Only the actual prepared source-bundle factory supplies this payload.
    /// It authenticates own clauses, query kind and the exact source parent.
    pub(super) fn from_source(
        parent: Option<DefinitionId>,
        clauses: &'data [(Clause<'cx>, Span)],
    ) -> Self {
        Self {
            parent,
            clauses,
            query: PhantomData,
        }
    }
}

impl<'data, 'cx, Q: ClauseQuery> GenericClauses<'data, 'cx, Q> {
    pub(super) fn instantiate<P: ClauseParents<'data, 'cx, Query = Q>>(
        self,
        tcx: Cx<'cx>,
        args: Args<'cx>,
        parents: P,
    ) -> InstantiatedClauses<'cx> {
        let mut instantiated = InstantiatedClauses::empty();
        self.instantiate_into(tcx, &mut instantiated, args, parents);
        instantiated
    }

    pub(super) fn instantiate_own(
        self,
        tcx: Cx<'cx>,
        args: Args<'cx>,
    ) -> impl Iterator<Item = (Unnormalized<Cx<'cx>, Clause<'cx>>, Span)>
    + DoubleEndedIterator
    + ExactSizeIterator
    + Clone {
        EarlyBinder::bind_iter(self.clauses)
            .iter_instantiated_copied(tcx, args.items())
            .map(|u| {
                let (clause, span) = u.unzip();
                (clause, span.skip_normalization())
            })
    }

    pub(super) fn instantiate_own_identity(
        self,
    ) -> impl Iterator<Item = (Unnormalized<Cx<'cx>, Clause<'cx>>, Span)>
    + DoubleEndedIterator
    + ExactSizeIterator
    + Clone {
        EarlyBinder::bind_iter(self.clauses)
            .iter_identity_copied()
            .map(|u| {
                let (clause, span) = u.unzip();
                (clause, span.skip_normalization())
            })
    }

    #[instrument(level = "debug", skip(self, tcx, parents))]
    fn instantiate_into<P: ClauseParents<'data, 'cx, Query = Q>>(
        self,
        tcx: Cx<'cx>,
        instantiated: &mut InstantiatedClauses<'cx>,
        args: Args<'cx>,
        parents: P,
    ) {
        if let Some(def_id) = self.parent {
            parents
                .clauses_of(def_id)
                .instantiate_into(tcx, instantiated, args, parents);
        }
        instantiated.clauses.extend(
            self.clauses
                .iter()
                .map(|(p, _)| EarlyBinder::bind(tcx, *p).instantiate(tcx, args)),
        );
        instantiated
            .spans
            .extend(self.clauses.iter().map(|(_, sp)| *sp));
    }

    pub(super) fn instantiate_identity<P: ClauseParents<'data, 'cx, Query = Q>>(
        self,
        tcx: Cx<'cx>,
        parents: P,
    ) -> InstantiatedClauses<'cx> {
        let mut instantiated = InstantiatedClauses::empty();
        self.instantiate_identity_into(tcx, &mut instantiated, parents);
        instantiated
    }

    fn instantiate_identity_into<P: ClauseParents<'data, 'cx, Query = Q>>(
        self,
        tcx: Cx<'cx>,
        instantiated: &mut InstantiatedClauses<'cx>,
        parents: P,
    ) {
        if let Some(def_id) = self.parent {
            parents
                .clauses_of(def_id)
                .instantiate_identity_into(tcx, instantiated, parents);
        }
        instantiated
            .clauses
            .extend(self.clauses.iter().map(|(p, _)| Unnormalized::new(*p)));
        instantiated
            .spans
            .extend(self.clauses.iter().map(|(_, s)| s));
    }
}
#[derive(Clone, Debug)]
pub(super) struct InstantiatedClauses<'cx> {
    pub clauses: Vec<Unnormalized<Cx<'cx>, Clause<'cx>>>,
    pub spans: Vec<Span>,
}

impl<'cx> InstantiatedClauses<'cx> {
    pub(super) fn empty() -> InstantiatedClauses<'cx> {
        InstantiatedClauses {
            clauses: vec![],
            spans: vec![],
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.clauses.is_empty()
    }

    pub(super) fn iter(&self) -> <&Self as IntoIterator>::IntoIter {
        self.into_iter()
    }
}

impl<'cx> IntoIterator for InstantiatedClauses<'cx> {
    type Item = (Unnormalized<Cx<'cx>, Clause<'cx>>, Span);

    type IntoIter = std::iter::Zip<
        std::vec::IntoIter<Unnormalized<Cx<'cx>, Clause<'cx>>>,
        std::vec::IntoIter<Span>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        debug_assert_eq!(self.clauses.len(), self.spans.len());
        std::iter::zip(self.clauses, self.spans)
    }
}

impl<'a, 'cx> IntoIterator for &'a InstantiatedClauses<'cx> {
    type Item = (Unnormalized<Cx<'cx>, Clause<'cx>>, Span);

    type IntoIter = std::iter::Zip<
        std::iter::Copied<std::slice::Iter<'a, Unnormalized<Cx<'cx>, Clause<'cx>>>>,
        std::iter::Copied<std::slice::Iter<'a, Span>>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        debug_assert_eq!(self.clauses.len(), self.spans.len());
        std::iter::zip(self.clauses.iter().copied(), self.spans.iter().copied())
    }
}
