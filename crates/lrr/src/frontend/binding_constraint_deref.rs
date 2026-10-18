//! Overloaded dereference in the source numeric constraint service.
//!
//! `*x` of a smart pointer (`Rc<T>`, `Arc<T>`, a user type with a `Deref`
//! impl) and the deref coercion of a reference argument (`&Vec<T>` where `&[T]`
//! is declared, `&Rc<T>` where `&T` is) take the value's `Deref::Target`.
//! That projection is normalized by the shared associated-type normalizer
//! (`associated_types::normalize`) over the run's impl member index, exactly
//! as a `for` head's `IntoIterator::Item` is (`iteration.rs`): the inference
//! holes of the value's static Type stand for its still-unknown parts, so a
//! numeric variable at some part of the value is at the matching part of the
//! target (`Vec<?v>` dereferences to `[?v]`).
//!
//! References and `Box` dereference by the language itself and keep their own
//! steps (`Step::Deref`, `box_contents`); this module is only reached for the
//! other nominal Types. Every walk is finite: a Type has finitely many
//! dereference steps to reach a declared head, bounded by rustc's autoderef
//! `recursion_limit`.
use super::{Component, ConstraintDemand, RECURSION_LIMIT, Step, Via, iteration};
use crate::{
    db::{memo::Database, poll::QueryPoll},
    ir::pc::NodeId,
    syntax::source_constraints::FactKind,
    traits::term::TyTerm,
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

/// Whether a value of Type `actual` already has the shape the declared Type
/// `declared` names, so that no dereference coercion applies: both are the
/// same nominal, the same family of sequence, tuple or reference, or the
/// declared Type is a parameter, an inference hole or an unsized coercion
/// target (a trait object).
pub(super) fn same_head(actual: &TyTerm, declared: &TyTerm) -> bool {
    match (actual, declared) {
        (
            _,
            TyTerm::Param(_)
            | TyTerm::Infer(_)
            | TyTerm::Dyn(_)
            | TyTerm::Opaque(..)
            | TyTerm::Projection(_)
            | TyTerm::Placeholder(_)
            | TyTerm::Bound { .. },
        ) => true,
        (TyTerm::Param(_) | TyTerm::Infer(_) | TyTerm::Never, _) => true,
        (TyTerm::Nominal(left, _), TyTerm::Nominal(right, _)) => left == right,
        (TyTerm::Array(..) | TyTerm::Slice(_), TyTerm::Array(..) | TyTerm::Slice(_)) => true,
        (TyTerm::Tuple(_), TyTerm::Tuple(_))
        | (TyTerm::Str, TyTerm::Str)
        | (TyTerm::Primitive(_), TyTerm::Primitive(_))
        | (TyTerm::Ref(..), TyTerm::Ref(..)) => true,
        _ => false,
    }
}

impl<'a> Component<'a> {
    /// The Type `ty` dereferences to by its overloaded `Deref` impl, and the
    /// projections of that Type that each of the member projections
    /// `candidates` (projections of `ty`) become. A member that is not at a
    /// part of the target (a closed target, another part of `ty`) maps to
    /// nothing. `None` when the target is not stated.
    pub(super) fn through_deref(
        &mut self,
        db: &mut Database,
        ty: &TyTerm,
        candidates: &[Vec<Step>],
    ) -> QueryPoll<Option<(TyTerm, Vec<Vec<Step>>)>> {
        self.through_target(db, ty, candidates, "Deref")
    }
    /// `through_deref` for the `Target` of another `core::ops` trait of the
    /// same shape (`Receiver`, which names an arbitrary-self-types step).
    pub(super) fn through_target(
        &mut self,
        db: &mut Database,
        ty: &TyTerm,
        candidates: &[Vec<Step>],
        trait_name: &str,
    ) -> QueryPoll<Option<(TyTerm, Vec<Vec<Step>>)>> {
        // A growable array's element is its type argument to the declared
        // Types that name it and `Element` to indexing: one position.
        let normalized: Vec<Vec<Step>>;
        let candidates = if ready!(self.vec_elements(db, ty)).is_some() {
            normalized = candidates
                .iter()
                .map(|steps| {
                    let mut steps = steps.clone();
                    if steps.first() == Some(&Step::Element) {
                        steps[0] = Step::TypeArgument(0);
                    }
                    steps
                })
                .collect();
            &normalized[..]
        } else {
            candidates
        };
        match ready!(self.associated_answer(db, ty, "ops", trait_name, "Target")) {
            iteration::ItemAnswer::Concrete(target) => {
                QueryPoll::Ready(Ok(Some((target, Vec::new()))))
            }
            iteration::ItemAnswer::Part(path) => {
                let Some(target) = ready!(self.project_term(db, ty, &path)) else {
                    return QueryPoll::Ready(Ok(None));
                };
                let mapped: Vec<Vec<Step>> = candidates
                    .iter()
                    .filter_map(|steps| steps.strip_prefix(path.as_slice()))
                    .map(<[Step]>::to_vec)
                    .collect();
                QueryPoll::Ready(Ok(Some((target, mapped))))
            }
            iteration::ItemAnswer::Structured { item, paths } => {
                let mut parts = Vec::with_capacity(paths.len());
                for path in &paths {
                    match ready!(self.project_term(db, ty, path)) {
                        Some(part) => parts.push(part),
                        None => return QueryPoll::Ready(Ok(None)),
                    }
                }
                let target = iteration::fill_holes(&item, &parts);
                let mut holes = Vec::new();
                iteration::item_holes(&item, &mut Vec::new(), &mut holes);
                let mut mapped = Vec::new();
                for steps in candidates {
                    for (hole, path) in paths.iter().enumerate() {
                        let Some(rest) = steps.strip_prefix(path.as_slice()) else {
                            continue;
                        };
                        for (occurrence, within) in &holes {
                            if *occurrence as usize == hole {
                                let mut steps = within.clone();
                                steps.extend_from_slice(rest);
                                mapped.push(steps);
                            }
                        }
                    }
                }
                QueryPoll::Ready(Ok(Some((target, mapped))))
            }
            iteration::ItemAnswer::Unknown => QueryPoll::Ready(Ok(None)),
        }
    }

    /// The type an arbitrary-self-types receiver `ty` names as its next probe
    /// step (`<ty as Receiver>::Target`).
    pub(super) fn receiver_target(
        &mut self,
        db: &mut Database,
        ty: &TyTerm,
    ) -> QueryPoll<Option<TyTerm>> {
        QueryPoll::Ready(Ok(
            ready!(self.through_target(db, ty, &[], "Receiver")).map(|(target, _)| target)
        ))
    }

    /// `*x` of the overloaded `Deref` Type `ty` (a projection step
    /// `Step::Deref` applied to a nominal): its `Target`.
    pub(super) fn deref_target(
        &mut self,
        db: &mut Database,
        ty: &TyTerm,
    ) -> QueryPoll<Option<TyTerm>> {
        QueryPoll::Ready(Ok(
            ready!(self.through_deref(db, ty, &[])).map(|(target, _)| target)
        ))
    }

    /// The current member is `node`, the operand of the dereference `parent`
    /// (`*node`): a part of `node`'s own Type that is at a part of its
    /// `Deref::Target` is at the matching part of `*node`. A `Box` dereferences
    /// by the language (its type argument is its content).
    pub(super) fn deref_outward(
        &mut self,
        db: &mut Database,
        node: NodeId,
        parent: NodeId,
    ) -> QueryPoll<()> {
        let steps = self.via.steps.clone();
        let Some(ty) = (if steps.is_empty() {
            None
        } else {
            ready!(self.expression_type(db, node))
        }) else {
            self.defer(parent, ConstraintDemand::ExpressionRelation);
            return QueryPoll::Ready(Ok(()));
        };
        if steps.first() == Some(&Step::TypeArgument(0))
            && ready!(self.box_contents(db, &ty)).is_some()
        {
            let via = self
                .via
                .unwrap(Step::TypeArgument(0))
                .expect("checked step");
            self.expression_via(Some(parent), via);
            return QueryPoll::Ready(Ok(()));
        }
        match ready!(self.through_deref(db, &ty, std::slice::from_ref(&steps))) {
            Some((_, mapped)) => {
                for steps in mapped {
                    self.expression_via(
                        Some(parent),
                        Via {
                            steps,
                            strict: false,
                        },
                    );
                }
            }
            None => self.defer(parent, ConstraintDemand::ExpressionRelation),
        }
        QueryPoll::Ready(Ok(()))
    }

    /// The method call `call` has the receiver projection `steps` (below the
    /// references the receiver expression is auto-dereferenced through) and
    /// the selected method declares the receiver Type `declared` (below its
    /// references): when the receiver's static Type is not the declared
    /// self type, the probe dereferenced it (`Vec<T>` to `[T]`, `Rc<T>` to
    /// `T`), and the projections become projections of the target. `None`
    /// when the receiver's Type is not stated or cannot be dereferenced to
    /// the declared head; an empty list when the member is not at a part of
    /// the target.
    pub(super) fn adjust_receiver(
        &mut self,
        db: &mut Database,
        call: NodeId,
        declared: &TyTerm,
        steps: Vec<Step>,
    ) -> QueryPoll<Option<Vec<Vec<Step>>>> {
        let receiver = match self.fact(call) {
            Ok(fact) => fact.operand(0),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(receiver) = receiver else {
            return QueryPoll::Ready(Ok(Some(vec![steps])));
        };
        let Some(mut actual) = ready!(self.expression_type(db, receiver)) else {
            return QueryPoll::Ready(Ok(Some(vec![steps])));
        };
        let mut frontier = vec![steps];
        for _ in 0..RECURSION_LIMIT {
            while let TyTerm::Ref(_, _, inner) = actual {
                actual = *inner;
            }
            if same_head(&actual, declared) {
                return QueryPoll::Ready(Ok(Some(frontier)));
            }
            if !matches!(actual, TyTerm::Nominal(..)) {
                return QueryPoll::Ready(Ok(None));
            }
            match ready!(self.through_deref(db, &actual, &frontier)) {
                Some((target, mapped)) => {
                    frontier = mapped;
                    actual = target;
                }
                None => return QueryPoll::Ready(Ok(None)),
            }
        }
        QueryPoll::Ready(Ok(None))
    }

    /// Argument `ordinal` of `call` is a reference that deref-coerces to the
    /// reference Type `declared` names (`&Vec<T>` to `&[T]`, `&Rc<T>` to
    /// `&T`): the current member's projection of the argument's Type, as a
    /// projection of the coerced Type. `None` when no coercion applies (the
    /// plain projection of `declared` is the relation) or the argument's Type
    /// is not stated; `Some(vias)` otherwise, empty when the member is not at
    /// a part of the coerced Type.
    pub(super) fn coerce_argument(
        &mut self,
        db: &mut Database,
        call: NodeId,
        ordinal: u32,
        declared: &TyTerm,
    ) -> QueryPoll<Option<Vec<Via>>> {
        if self.via.steps.first() != Some(&Step::Deref) {
            return QueryPoll::Ready(Ok(None));
        }
        let TyTerm::Ref(_, _, declared_inner) = declared else {
            return QueryPoll::Ready(Ok(None));
        };
        let (operands, offset) = match self.fact(call) {
            Ok(fact) => (
                fact.operands().to_vec(),
                usize::from(!matches!(
                    fact.kind(),
                    FactKind::MethodCall | FactKind::LangCall
                )),
            ),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(argument) = operands.get(ordinal as usize + offset).copied().flatten() else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(TyTerm::Ref(_, _, actual)) = ready!(self.expression_type(db, argument)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut actual: TyTerm = *actual;
        let mut frontier: Vec<Vec<Step>> = vec![self.via.steps[1..].to_vec()];
        let mut coerced = false;
        // rustc's autoderef is bounded by the recursion limit.
        for _ in 0..RECURSION_LIMIT {
            if same_head(&actual, declared_inner) {
                if !coerced {
                    return QueryPoll::Ready(Ok(None));
                }
                return QueryPoll::Ready(Ok(Some(
                    frontier
                        .into_iter()
                        .map(|rest| {
                            let mut steps = Vec::with_capacity(rest.len() + 1);
                            steps.push(Step::Deref);
                            steps.extend(rest);
                            Via {
                                steps,
                                strict: false,
                            }
                        })
                        .collect(),
                )));
            }
            match actual {
                // `&&A` coerces to `&A`: the language dereferences the
                // pointee reference.
                TyTerm::Ref(_, _, inner) => {
                    frontier = frontier
                        .into_iter()
                        .filter_map(|rest| match rest.split_first() {
                            Some((Step::Deref, rest)) => Some(rest.to_vec()),
                            _ => None,
                        })
                        .collect();
                    actual = *inner;
                }
                TyTerm::Nominal(..) => match ready!(self.through_deref(db, &actual, &frontier)) {
                    Some((target, mapped)) => {
                        frontier = mapped;
                        actual = target;
                    }
                    None => return QueryPoll::Ready(Ok(None)),
                },
                _ => return QueryPoll::Ready(Ok(None)),
            }
            coerced = true;
        }
        QueryPoll::Ready(Ok(None))
    }
}
