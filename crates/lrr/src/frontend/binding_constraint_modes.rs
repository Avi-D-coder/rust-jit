//! Where a binding sits in the value its root pattern matches, with rustc's
//! default binding modes.
//!
//! A pattern that is not a reference pattern or a binding, matched against a
//! reference, matches the referent and binds its names by reference. The
//! retained pattern engine's occurrence steps are the written structure; the
//! references between them come from the matched value's static Type, which
//! this module reads from the common services. A Type the services do not
//! state leaves the occurrence unplaced (an unanswered goal), never guessed.
use super::{Component, Step, nominal::ConstructorSite};
use crate::{
    db::{memo::Database, poll::QueryPoll},
    ir::pc::NodeId,
    syntax::source_constraints::{FactKind, PatternOccurrence, PatternStep},
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

/// `annotation` with each written hole (`_`, or a leaf the Type service
/// does not state) filled by the matching part of the initializer's Type.
fn fill_from(annotation: &TyTerm, initializer: &TyTerm) -> TyTerm {
    use crate::traits::term::GenericArg;
    match (annotation, initializer) {
        (TyTerm::Infer(_), initializer) => initializer.clone(),
        (TyTerm::Ref(region, mutable, left), TyTerm::Ref(_, _, right)) => {
            TyTerm::Ref(region.clone(), *mutable, Box::new(fill_from(left, right)))
        }
        (TyTerm::Nominal(left, left_args), TyTerm::Nominal(right, right_args))
            if left == right && left_args.len() == right_args.len() =>
        {
            TyTerm::Nominal(
                *left,
                left_args
                    .iter()
                    .zip(right_args)
                    .map(|pair| match pair {
                        (GenericArg::Type(left), GenericArg::Type(right)) => {
                            GenericArg::Type(fill_from(left, right))
                        }
                        (left, _) => left.clone(),
                    })
                    .collect(),
            )
        }
        (TyTerm::Tuple(left), TyTerm::Tuple(right)) if left.len() == right.len() => TyTerm::Tuple(
            left.iter()
                .zip(right)
                .map(|(left, right)| fill_from(left, right))
                .collect(),
        ),
        (TyTerm::Array(left, length), TyTerm::Array(right, _)) => {
            TyTerm::Array(Box::new(fill_from(left, right)), length.clone())
        }
        (TyTerm::Slice(left), TyTerm::Slice(right) | TyTerm::Array(right, _)) => {
            TyTerm::Slice(Box::new(fill_from(left, right)))
        }
        (annotation, _) => annotation.clone(),
    }
}

impl<'a> Component<'a> {
    /// The static Type the root pattern `root` matches (a `let` annotation
    /// or initializer, a `match` scrutinee, or a `for` head's `Item`) and how
    /// many leading occurrence steps select the loop's `Some` instead.
    pub(super) fn root_matched_type(
        &mut self,
        db: &mut Database,
        root: NodeId,
    ) -> QueryPoll<(Option<TyTerm>, usize)> {
        let plan = self.plan;
        // A pattern function's parameter pattern matches its argument: the
        // declared Type of that signature parameter (E-R5-010).
        if let Some(ordinal) = plan.fact(root).and_then(|fact| fact.parameter()) {
            let definition = self.key.body.definition;
            return QueryPoll::Ready(Ok((
                ready!(self.parameter_term(db, root, definition, ordinal)),
                0,
            )));
        }
        let Some(edge) = plan.uses(root).first() else {
            return QueryPoll::Ready(Ok((None, 0)));
        };
        let Some(parent) = plan.fact(edge.parent) else {
            return QueryPoll::Ready(Ok((None, 0)));
        };
        QueryPoll::Ready(Ok(match parent.kind() {
            // A `for` pattern is the loop's `Some(PAT)` test: its first step
            // selects the `Some` field of the head's `Item`.
            FactKind::For if edge.ordinal == 1 => {
                let matched = match parent.operand(0) {
                    Some(head) => match ready!(self.expression_type(db, head)) {
                        Some(head) => ready!(self.project_term(db, &head, &[Step::Item])),
                        None => None,
                    },
                    None => None,
                };
                (matched, 1)
            }
            FactKind::Let if edge.ordinal == 0 => {
                let initializer = match parent.operand(1) {
                    Some(initializer) => ready!(self.expression_type(db, initializer)),
                    None => None,
                };
                // A written hole (`Box<_>`) is filled by the initializer.
                let matched = match (ready!(self.annotation_term(db, edge.parent)), initializer) {
                    (Some(annotation), Some(initializer)) => {
                        Some(fill_from(&annotation, &initializer))
                    }
                    (Some(annotation), None) => Some(annotation),
                    (None, _) if parent.annotation().is_some() => None,
                    (None, initializer) => initializer,
                };
                (matched, 0)
            }
            FactKind::Match if edge.ordinal % 2 == 1 => {
                let matched = match parent.operand(0) {
                    Some(scrutinee) => ready!(self.expression_type(db, scrutinee)),
                    None => None,
                };
                (matched, 0)
            }
            // A closure parameter pattern matches the call argument: its
            // written Type, else the `Fn*(A, ..)` bound it is passed for.
            FactKind::Closure if edge.ordinal >= 1 => {
                (ready!(self.closure_parameter_type(db, root)), 0)
            }
            _ => (None, 0),
        }))
    }

    /// The occurrence's steps as steps of the matched value (the first
    /// `skipped` are the loop's `Some` selection), with the references the
    /// default binding mode dereferences between them, and whether the name
    /// binds by reference. `None` when a step cannot be placed.
    pub(super) fn occurrence_steps(
        &mut self,
        db: &mut Database,
        occurrence: &PatternOccurrence,
        skipped: usize,
        matched: Option<TyTerm>,
    ) -> QueryPoll<Option<(Vec<Step>, bool)>> {
        let mut steps = Vec::with_capacity(occurrence.steps.len());
        let mut term = matched;
        let mut implicit = false;
        for (index, written) in occurrence.steps.iter().enumerate() {
            let counted_from_end = |term: &Option<TyTerm>, from_end: u32| match term {
                Some(TyTerm::Tuple(items)) => items
                    .len()
                    .checked_sub(1 + from_end as usize)
                    .and_then(|position| u32::try_from(position).ok()),
                Some(TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit)) => None,
                _ => None,
            };
            if index >= skipped && !matches!(written, PatternStep::Deref) {
                // A non-reference pattern against a reference matches the
                // referent (the default binding mode becomes `ref`).
                while let Some(TyTerm::Ref(_, _, inner)) = term {
                    steps.push(Step::Deref);
                    implicit = true;
                    term = Some(*inner);
                }
            }
            let step = match *written {
                PatternStep::Deref => Step::Deref,
                PatternStep::Field(position) => Step::Field(position),
                PatternStep::Element => Step::Element,
                PatternStep::Constructor { slot, written } => {
                    match ready!(self.occurrence_constructor_step(
                        db,
                        occurrence.root,
                        slot,
                        written
                    )) {
                        Some(step) => step,
                        None => return QueryPoll::Ready(Ok(None)),
                    }
                }
                PatternStep::TupleFromEnd(from_end) => match counted_from_end(&term, from_end) {
                    Some(position) => Step::Field(position),
                    None => return QueryPoll::Ready(Ok(None)),
                },
                PatternStep::ConstructorFromEnd { slot, from_end } => {
                    let site = ConstructorSite::Pattern {
                        root: occurrence.root,
                        slot,
                    };
                    let Some(constructed) = ready!(self.constructor(db, site)) else {
                        return QueryPoll::Ready(Ok(None));
                    };
                    let Some(field) = constructed
                        .arity()
                        .checked_sub(1 + from_end as usize)
                        .and_then(|field| u32::try_from(field).ok())
                    else {
                        return QueryPoll::Ready(Ok(None));
                    };
                    match self.field_step(&constructed, field) {
                        Some(step) => step,
                        None => return QueryPoll::Ready(Ok(None)),
                    }
                }
            };
            steps.push(step);
            if index >= skipped {
                term = match term {
                    Some(inner) => ready!(self.project_term(db, &inner, &[step])),
                    None => None,
                };
            }
        }
        QueryPoll::Ready(Ok(Some((
            steps,
            occurrence.by_reference || (implicit && !occurrence.mutable),
        ))))
    }
}
