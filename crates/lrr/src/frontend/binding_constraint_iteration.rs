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
    /// A Type built around parts of `T` (`&T`'s `Item` for `&[T; N]`,
    /// `(&K, &V)` for `&HashMap<K, V>`): `item` with its numbered holes
    /// standing for the parts of `T` at `paths`.
    Structured {
        item: TyTerm,
        paths: Vec<Vec<Step>>,
    },
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
        // A callee's still-unfixed parameter is as unknown as an inference
        // variable: it stands for a part of the head.
        TyTerm::Infer(_) | TyTerm::Param(_) => {
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
        TyTerm::Ref(region, mutable, inner) => {
            at.push(Step::Deref);
            let inner = with_holes(inner, at, paths);
            at.pop();
            TyTerm::Ref(region.clone(), *mutable, Box::new(inner))
        }
        other => other.clone(),
    }
}
/// The numbered holes of an `Item` Type with the projection steps from the
/// `Item` to each occurrence.
pub(super) fn item_holes(term: &TyTerm, at: &mut Vec<Step>, out: &mut Vec<(u32, Vec<Step>)>) {
    match term {
        TyTerm::Infer(key) if key.transaction == hole_transaction() => {
            out.push((key.ordinal, at.clone()))
        }
        TyTerm::Ref(_, _, inner) => {
            at.push(Step::Deref);
            item_holes(inner, at, out);
            at.pop();
        }
        TyTerm::Slice(inner) | TyTerm::Array(inner, _) => {
            at.push(Step::Element);
            item_holes(inner, at, out);
            at.pop();
        }
        TyTerm::Tuple(items) => {
            for (position, item) in items.iter().enumerate() {
                let Ok(position) = u32::try_from(position) else {
                    return;
                };
                at.push(Step::Field(position));
                item_holes(item, at, out);
                at.pop();
            }
        }
        TyTerm::Nominal(_, args) => {
            let types = args.iter().filter_map(|arg| match arg {
                GenericArg::Type(inner) => Some(inner),
                _ => None,
            });
            for (position, inner) in types.enumerate() {
                let Ok(position) = u32::try_from(position) else {
                    return;
                };
                at.push(Step::TypeArgument(position));
                item_holes(inner, at, out);
                at.pop();
            }
        }
        _ => {}
    }
}
/// `term` with each numbered hole replaced by its Type.
pub(super) fn fill_holes(term: &TyTerm, parts: &[TyTerm]) -> TyTerm {
    match term {
        TyTerm::Infer(key) if key.transaction == hole_transaction() => parts
            .get(key.ordinal as usize)
            .cloned()
            .unwrap_or_else(|| term.clone()),
        TyTerm::Ref(region, mutable, inner) => {
            TyTerm::Ref(region.clone(), *mutable, Box::new(fill_holes(inner, parts)))
        }
        TyTerm::Slice(inner) => TyTerm::Slice(Box::new(fill_holes(inner, parts))),
        TyTerm::Array(inner, length) => {
            TyTerm::Array(Box::new(fill_holes(inner, parts)), length.clone())
        }
        TyTerm::Tuple(items) => {
            TyTerm::Tuple(items.iter().map(|item| fill_holes(item, parts)).collect())
        }
        TyTerm::Nominal(identity, args) => TyTerm::Nominal(
            *identity,
            args.iter()
                .map(|arg| match arg {
                    GenericArg::Type(inner) => GenericArg::Type(fill_holes(inner, parts)),
                    other => other.clone(),
                })
                .collect(),
        ),
        other => other.clone(),
    }
}
/// Where `steps` lead inside an `Item` Type that still has holes: a hole
/// (and the steps remaining below it) or a closed Type.
enum ItemProjection {
    Hole(u32, Vec<Step>),
    Concrete(TyTerm),
    Unknown,
}
fn project_item(item: &TyTerm, steps: &[Step]) -> ItemProjection {
    let mut term = item.clone();
    for (index, step) in steps.iter().enumerate() {
        if let TyTerm::Infer(key) = &term
            && key.transaction == hole_transaction()
        {
            return ItemProjection::Hole(key.ordinal, steps[index..].to_vec());
        }
        if *step == Step::AutoDeref {
            while let TyTerm::Ref(_, _, inner) = term {
                term = *inner;
            }
            continue;
        }
        term = match (step, term) {
            (Step::Deref, TyTerm::Ref(_, _, inner)) => *inner,
            // `*p` of a raw pointer is its pointee (E-R5-008).
            (Step::Deref, term)
                if crate::traits::pointer_terms::as_raw_pointer(&term).is_some() =>
            {
                match crate::traits::pointer_terms::as_raw_pointer(&term) {
                    Some((_, pointee)) => pointee.clone(),
                    None => return ItemProjection::Unknown,
                }
            }
            (Step::Field(position), TyTerm::Tuple(mut members)) => {
                if (*position as usize) < members.len() {
                    members.swap_remove(*position as usize)
                } else {
                    return ItemProjection::Unknown;
                }
            }
            (Step::Element, TyTerm::Array(inner, _) | TyTerm::Slice(inner)) => *inner,
            (Step::TypeArgument(position), TyTerm::Nominal(_, args)) => {
                match super::type_argument(&args, *position) {
                    Some(inner) => inner,
                    None => return ItemProjection::Unknown,
                }
            }
            _ => return ItemProjection::Unknown,
        };
    }
    match term {
        TyTerm::Infer(key) if key.transaction == hole_transaction() => {
            ItemProjection::Hole(key.ordinal, Vec::new())
        }
        term if !mentions_hole(&term) => ItemProjection::Concrete(term),
        _ => ItemProjection::Unknown,
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
    pub(super) fn core_trait(
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
        self.associated_answer(db, ty, "iter", "IntoIterator", "Item")
    }
    /// `<ty as Trait>::Name` for a core trait (`module::trait_name`),
    /// normalized by the shared projection normalizer over the run's impl
    /// member index. The holes of `ty` stand for its still-unknown parts, so
    /// the answer relates the projection to those parts.
    pub(super) fn associated_answer(
        &mut self,
        db: &mut Database,
        ty: &TyTerm,
        module: &str,
        trait_name: &str,
        name: &str,
    ) -> QueryPoll<ItemAnswer> {
        let Some(trait_key) = ready!(self.core_trait(db, module, trait_name)) else {
            return QueryPoll::Ready(Ok(ItemAnswer::Unknown));
        };
        self.associated_answer_of(db, ty, trait_key, name)
    }
    /// `<ty as Trait>::Name` for the trait `trait_key`, or for whichever trait
    /// the Type's unique impl declares `Name` in when `trait_key` is the zero
    /// digest (the shorthand `T::Name`).
    pub(super) fn associated_answer_of(
        &mut self,
        db: &mut Database,
        ty: &TyTerm,
        trait_key: Digest,
        name: &str,
    ) -> QueryPoll<ItemAnswer> {
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
                trait_key,
                self_ty: head,
                args: Vec::new(),
            },
            item: crate::frontend::member_index::associated_name_key(name),
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
            // Several candidate impls: rustc's selection by the trait's other
            // arguments is not repeated here, so the projection is unanswered.
            QueryPoll::Ready(Err(error))
                if super::not_closed(&error) || error.code == "LRR-E-NAME-AMBIGUOUS" =>
            {
                return QueryPoll::Ready(Ok(ItemAnswer::Unknown));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.note_depth(super::term_depth(ty).max(super::term_depth(&item)));
        QueryPoll::Ready(Ok(match item {
            TyTerm::Infer(key) if key.transaction == hole_transaction() => {
                match paths.get(key.ordinal as usize) {
                    Some(path) => ItemAnswer::Part(path.clone()),
                    None => ItemAnswer::Unknown,
                }
            }
            item if !mentions_hole(&item) => ItemAnswer::Concrete(item),
            // Not a bare hole, but only holes of the head are open: a Type
            // built around the head's parts.
            item if !paths.is_empty() => ItemAnswer::Structured { item, paths },
            _ => ItemAnswer::Unknown,
        }))
    }
    /// The static Type of the index expression `node` (`base[index]`) when
    /// its base (through references) is a nominal with its own `Index<Idx>`
    /// impl: `<Base as Index<Idx>>::Output`, `Idx` being the operand's static
    /// Type. `None` for a builtin array/slice/`Vec` base or when the
    /// selection is not stated (several impls the operand does not select).
    pub(super) fn user_index_output(
        &mut self,
        db: &mut Database,
        node: NodeId,
    ) -> QueryPoll<Option<TyTerm>> {
        use crate::traits::term::GenericArg;
        let fact = match self.fact(node) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let (Some(base), Some(operand)) = (fact.operand(0), fact.operand(1)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(mut base_ty) = ready!(self.expression_type(db, base)) else {
            return QueryPoll::Ready(Ok(None));
        };
        while let TyTerm::Ref(_, _, inner) = base_ty {
            base_ty = *inner;
        }
        if !matches!(base_ty, TyTerm::Nominal(..))
            || ready!(self.vec_elements(db, &base_ty)).is_some()
        {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(index_ty) = ready!(self.expression_type(db, operand)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(trait_key) = ready!(self.core_trait(db, "ops", "Index")) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let projection = TyTerm::Projection(Box::new(ProjectionKey {
            trait_ref: TraitRef {
                trait_key,
                self_ty: base_ty,
                args: vec![GenericArg::Type(index_ty)],
            },
            item: crate::frontend::member_index::associated_name_key("Output"),
            item_args: Vec::new(),
        }));
        let sources: Vec<_> = environment.sources().iter().collect();
        match crate::frontend::associated_types::normalize(
            db,
            environment.names(),
            &sources,
            &members,
            &projection,
        ) {
            QueryPoll::Ready(Ok(Some(output))) if !matches!(output, TyTerm::Projection(_)) => {
                QueryPoll::Ready(Ok(Some(output)))
            }
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error))
                if super::not_closed(&error) || error.code == "LRR-E-NAME-AMBIGUOUS" =>
            {
                QueryPoll::Ready(Ok(None))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
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
            // A Type built around the head's parts: the rest of the
            // projection either reaches one of those parts or a closed Type.
            ItemAnswer::Structured { item, paths } => match project_item(&item, &rest.steps) {
                ItemProjection::Hole(hole, remaining) => match paths.get(hole as usize) {
                    Some(path) => {
                        let mut steps = path.clone();
                        steps.extend(remaining);
                        self.expression_via(
                            Some(node),
                            Via {
                                steps,
                                strict: false,
                            },
                        );
                        true
                    }
                    None => false,
                },
                ItemProjection::Concrete(term) => {
                    let current = std::mem::replace(
                        &mut self.via,
                        Via {
                            steps: Vec::new(),
                            strict: rest.strict,
                        },
                    );
                    let result =
                        self.constrain_term(db, node, &term, ConstraintDemand::ResultType);
                    self.via = current;
                    ready!(result);
                    true
                }
                ItemProjection::Unknown => false,
            },
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
                    // The head's part that a Type built around the head's
                    // parts holds: the pattern's matching position.
                    ItemAnswer::Structured { item, paths } => {
                        let Some(some) = ready!(self.for_some_step(db, root)) else {
                            self.defer(looped, ConstraintDemand::Binding);
                            return QueryPoll::Ready(Ok(()));
                        };
                        let mut holes = Vec::new();
                        item_holes(&item, &mut Vec::new(), &mut holes);
                        for (hole, path) in paths.iter().enumerate() {
                            if path.is_empty() || !self.via.steps.starts_with(path) {
                                continue;
                            }
                            for (occurrence, within) in &holes {
                                if *occurrence as usize != hole {
                                    continue;
                                }
                                let mut steps = vec![some];
                                steps.extend(within.iter().copied());
                                steps.extend_from_slice(&self.via.steps[path.len()..]);
                                self.pattern_via(
                                    Some(root),
                                    Via {
                                        steps,
                                        strict: false,
                                    },
                                );
                            }
                        }
                        return QueryPoll::Ready(Ok(()));
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
