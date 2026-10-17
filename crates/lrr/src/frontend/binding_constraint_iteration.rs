//! `for` loops in the source numeric constraint service.
//!
//! rustc desugars `for PAT in HEAD BODY` into `IntoIterator::into_iter(HEAD)`
//! and per iteration `Iterator::next(&mut iter)` matched against `Some(PAT)`
//! (the shared pattern plan's `ForPlan` and `PatternTree::build_for`). The
//! pattern therefore matches `Option<<HEAD as IntoIterator>::Item>`. That
//! projection is normalized by the shared associated-type normalizer
//! (`associated_types::normalize`) over the run's impl member index: the
//! head Type's own `IntoIterator` impl, else the core blanket
//! `impl<I: Iterator> IntoIterator for I` whose `type Item = I::Item` then
//! selects the Type's `Iterator` impl. Inference holes of the head's static
//! Type stand for its still-unknown parts: an `Item` that is one of them
//! (`Range<A>` -> `A`, `[T; N]` -> `T`) relates the pattern to that part of
//! the head. Any other head stays unknown, never a fallback.
use super::{Component, ConstraintDemand, Step, Via};
use crate::{
    core::digest::Digest,
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        poll::QueryPoll,
    },
    ir::pc::NodeId,
    source::manifest::Namespace,
    syntax::pattern_tree::PatternNode,
    traits::term::{GenericArg, InferVarKey, ProjectionKey, TraitRef, TyTerm},
};

macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}

/// What `<T as IntoIterator>::Item` is for a static Type `T`.
#[derive(Debug, Clone)]
pub(super) enum ItemAnswer {
    Concrete(TyTerm),
    /// The part of `T` these projection steps select.
    Part(Vec<Step>),
    Unknown,
}

/// The transaction of the holes standing for a head Type's unknown parts.
fn hole_transaction() -> Digest {
    Digest::of_bytes(b"lrr/numeric-constraint/iteration-hole")
}
/// `ty` with each inference hole replaced by a numbered hole whose path
/// (projection steps from `ty`) is recorded in `paths`.
fn with_holes(ty: &TyTerm, at: &mut Vec<Step>, paths: &mut Vec<Vec<Step>>) -> TyTerm {
    match ty {
        TyTerm::Infer(_) => {
            let ordinal = u32::try_from(paths.len()).unwrap_or(u32::MAX);
            paths.push(at.clone());
            TyTerm::Infer(InferVarKey {
                transaction: hole_transaction(),
                ordinal,
            })
        }
        TyTerm::Nominal(identity, args) => {
            let mut position = 0u32;
            let args = args
                .iter()
                .map(|arg| match arg {
                    GenericArg::Type(inner) => {
                        at.push(Step::TypeArgument(position));
                        position += 1;
                        let inner = with_holes(inner, at, paths);
                        at.pop();
                        GenericArg::Type(inner)
                    }
                    other => other.clone(),
                })
                .collect();
            TyTerm::Nominal(*identity, args)
        }
        TyTerm::Array(inner, length) => {
            at.push(Step::Element);
            let inner = with_holes(inner, at, paths);
            at.pop();
            TyTerm::Array(Box::new(inner), length.clone())
        }
        TyTerm::Slice(inner) => {
            at.push(Step::Element);
            let inner = with_holes(inner, at, paths);
            at.pop();
            TyTerm::Slice(Box::new(inner))
        }
        TyTerm::Tuple(items) => TyTerm::Tuple(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    at.push(Step::Field(u32::try_from(index).unwrap_or(u32::MAX)));
                    let item = with_holes(item, at, paths);
                    at.pop();
                    item
                })
                .collect(),
        ),
        // References are not part of a for head's own selection here.
        other => other.clone(),
    }
}
/// Whether `term` still mentions an inference hole.
fn mentions_hole(term: &TyTerm) -> bool {
    match term {
        TyTerm::Infer(_) => true,
        TyTerm::Ref(_, _, inner) | TyTerm::Slice(inner) | TyTerm::Array(inner, _) => {
            mentions_hole(inner)
        }
        TyTerm::Tuple(items) => items.iter().any(mentions_hole),
        TyTerm::Nominal(_, args) => args.iter().any(|arg| match arg {
            GenericArg::Type(inner) => mentions_hole(inner),
            _ => false,
        }),
        TyTerm::Projection(key) => mentions_hole(&key.trait_ref.self_ty),
        _ => false,
    }
}

impl<'a> Component<'a> {
    /// A trait of the core library facade, through the run's own name graph.
    fn core_trait(
        &mut self,
        db: &mut Database,
        module: &str,
        name: &str,
    ) -> QueryPoll<Option<Digest>> {
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let profile = match db
            .input(&InputKey::DefinitionHeader(self.key.body.definition))
            .as_deref()
        {
            Some(InputValue::DefinitionHeader(header)) => header.profile.clone(),
            _ => return QueryPoll::Ready(Ok(None)),
        };
        let Some(caller) = self.caller_scope(db) else {
            return QueryPoll::Ready(Ok(None));
        };
        let core = crate::facade::unit(&profile, "core");
        let path = match crate::frontend::names::NameNamespaces::path(
            core,
            &[module, name],
            profile.edition,
        ) {
            Ok(path) => path,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // Without the library facade no such trait is declared.
        QueryPoll::Ready(
            match environment.names().lookup(Namespace::Type, &path, &caller) {
                Ok(crate::frontend::names::NameLookup::Defined(definition)) => {
                    Ok(Some(definition.identity))
                }
                Ok(_) => Ok(None),
                Err(error) => Err(error),
            },
        )
    }
    /// `<ty as IntoIterator>::Item`, normalized by the shared projection
    /// normalizer over the run's impl member index.
    pub(super) fn iteration_item(
        &mut self,
        db: &mut Database,
        ty: &TyTerm,
    ) -> QueryPoll<ItemAnswer> {
        let Some(into_iterator) = ready!(self.core_trait(db, "iter", "IntoIterator")) else {
            return QueryPoll::Ready(Ok(ItemAnswer::Unknown));
        };
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(ItemAnswer::Unknown));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut paths = Vec::new();
        let head = with_holes(ty, &mut Vec::new(), &mut paths);
        let projection = TyTerm::Projection(Box::new(ProjectionKey {
            trait_ref: TraitRef {
                trait_key: into_iterator,
                self_ty: head,
                args: Vec::new(),
            },
            item: crate::frontend::member_index::associated_name_key("Item"),
            item_args: Vec::new(),
        }));
        let sources: Vec<_> = environment.sources().iter().collect();
        let item = match crate::frontend::associated_types::normalize(
            db,
            environment.names(),
            &sources,
            &members,
            &projection,
        ) {
            QueryPoll::Ready(Ok(Some(item))) => item,
            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(ItemAnswer::Unknown)),
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                return QueryPoll::Ready(Ok(ItemAnswer::Unknown));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready(Ok(match item {
            TyTerm::Infer(key) if key.transaction == hole_transaction() => {
                match paths.get(key.ordinal as usize) {
                    Some(path) => ItemAnswer::Part(path.clone()),
                    None => ItemAnswer::Unknown,
                }
            }
            item if !mentions_hole(&item) => ItemAnswer::Concrete(item),
            _ => ItemAnswer::Unknown,
        }))
    }
    /// The current member (`node`) holds `<T as IntoIterator>::Item` of its
    /// own static Type `T`, projected by the rest of the steps.
    pub(super) fn item_inward(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<bool> {
        let Some(ty) = ready!(self.expression_type(db, node)) else {
            return QueryPoll::Ready(Ok(false));
        };
        let rest = self.via.unwrap(Step::Item).expect("checked item step");
        QueryPoll::Ready(Ok(match ready!(self.iteration_item(db, &ty)) {
            ItemAnswer::Part(mut path) => {
                path.extend_from_slice(&rest.steps);
                self.expression_via(
                    Some(node),
                    Via {
                        steps: path,
                        strict: false,
                    },
                );
                true
            }
            ItemAnswer::Concrete(term) => {
                let current = std::mem::replace(&mut self.via, rest);
                let result = self.constrain_term(db, node, &term, ConstraintDemand::ResultType);
                self.via = current;
                ready!(result);
                true
            }
            ItemAnswer::Unknown => false,
        }))
    }
    /// The `Some` field step of a `for` pattern root's retained tree.
    fn for_some_step(&mut self, db: &mut Database, root: NodeId) -> QueryPoll<Option<Step>> {
        let plan = self.plan;
        let Some(PatternNode::Constructor { slot, .. }) = plan.tree(root).map(|tree| tree.root())
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let site = super::nominal::ConstructorSite::Pattern { root, slot: *slot };
        let Some(constructed) = ready!(self.constructor(db, site)) else {
            return QueryPoll::Ready(Ok(None));
        };
        QueryPoll::Ready(Ok(self.field_step(&constructed, 0)))
    }
    /// The `for` pattern root matched under the current member's projection:
    /// its `Some` field is the head's `Item`.
    pub(super) fn for_pattern(
        &mut self,
        db: &mut Database,
        root: NodeId,
        looped: NodeId,
    ) -> QueryPoll<()> {
        let head = match self.fact(looped) {
            Ok(fact) => fact.operand(0),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let some = ready!(self.for_some_step(db, root));
        match (some, self.via.steps.first()) {
            (Some(some), Some(first)) if *first == some => {
                let rest = self.via.unwrap(some).expect("checked some step");
                self.expression_via(head, rest.wrap(Step::Item));
            }
            _ => self.defer(root, ConstraintDemand::Binding),
        }
        QueryPoll::Ready(Ok(()))
    }
    /// The `for` head under the current member's projection: when that is
    /// the head's `Item`, the pattern root's `Some` field holds it.
    pub(super) fn for_head(&mut self, db: &mut Database, looped: NodeId) -> QueryPoll<()> {
        let (head, root) = match self.fact(looped) {
            Ok(fact) => (fact.operand(0), fact.operand(1)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let (Some(head), Some(root)) = (head, root) else {
            self.defer(looped, ConstraintDemand::ExpressionRelation);
            return QueryPoll::Ready(Ok(()));
        };
        let Some(first) = self.via.steps.first().copied() else {
            // The head itself is an iterable, never a numeric value.
            self.defer(looped, ConstraintDemand::NumericFamily);
            return QueryPoll::Ready(Ok(()));
        };
        // The head's `Item` itself, or the part of the head's Type that its
        // selected `Item` is.
        let after = match first {
            Step::Item => Some(1),
            _ => match ready!(self.expression_type(db, head)) {
                Some(ty) => match ready!(self.iteration_item(db, &ty)) {
                    ItemAnswer::Part(path)
                        if !path.is_empty() && self.via.steps.starts_with(&path) =>
                    {
                        Some(path.len())
                    }
                    // Another part of the head's selected Type relates to
                    // nothing through the loop.
                    ItemAnswer::Part(_) | ItemAnswer::Concrete(_) => {
                        return QueryPoll::Ready(Ok(()));
                    }
                    ItemAnswer::Unknown => None,
                },
                None => None,
            },
        };
        let Some(after) = after else {
            self.defer(looped, ConstraintDemand::ExpressionRelation);
            return QueryPoll::Ready(Ok(()));
        };
        let Some(some) = ready!(self.for_some_step(db, root)) else {
            self.defer(looped, ConstraintDemand::Binding);
            return QueryPoll::Ready(Ok(()));
        };
        let rest = Via {
            steps: self.via.steps[after..].to_vec(),
            strict: false,
        };
        self.pattern_via(Some(root), rest.wrap(some));
        QueryPoll::Ready(Ok(()))
    }
}
