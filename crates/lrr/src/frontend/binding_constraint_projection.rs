//! Associated-type projections in the source constraint service.
//!
//! A declared Type may name `<T as Trait>::Name` (or `T::Name`, `Self::Name`)
//! of a selected function's own type parameter. Its value is decided by the
//! Types the call's operands state: the callee's parameters are bound from
//! the operands' static Types exactly as the call's result already is, and the
//! projection is then normalized through the same service the dispatcher uses
//! (`associated_types::normalize` over the completed environment's shared
//! impl member index). A projection whose self type is still open (a
//! parameter of the executing function, an inference variable) stays an
//! unanswered goal; nothing here guesses a Type.
//!
//! Everything is demand-driven: terms without a projection never reach this
//! module's work, and the member index is the environment's own retained
//! handle.
use super::{Component, FactKind, QueryPoll, not_closed};
use crate::{
    core::digest::Digest,
    frontend::associated_types::{contains_projection, normalize, substitute},
    ir::pc::NodeId,
    traits::term::{ParamKey, TyTerm},
};
use std::collections::BTreeMap;

macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}

/// Whether a refusal of the projection normalizer only leaves the projection
/// unanswered: an absent name or unsupported route, or several candidate
/// impls (rustc's selection by the trait's other arguments is not repeated
/// here, and a looser answer is never a refusal).
fn unanswered(error: &crate::core::diagnostic::Diagnostic) -> bool {
    not_closed(error) || error.code == "LRR-E-NAME-AMBIGUOUS"
}

impl<'a> Component<'a> {
    /// `term` with every projection whose self type is concrete normalized.
    /// A term that cannot be normalized completely is returned unchanged.
    pub(super) fn normalize_projections(
        &mut self,
        db: &mut crate::db::memo::Database,
        term: &TyTerm,
    ) -> QueryPoll<TyTerm> {
        if !contains_projection(term) {
            return QueryPoll::Ready(Ok(term.clone()));
        }
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(term.clone()));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        match normalize(db, environment.names(), &sources, &members, term) {
            QueryPoll::Ready(Ok(Some(normalized))) => {
                self.note_depth(super::term_depth(&normalized));
                QueryPoll::Ready(Ok(normalized))
            }
            QueryPoll::Ready(Ok(None)) => QueryPoll::Ready(Ok(term.clone())),
            QueryPoll::Ready(Err(error)) if unanswered(&error) => {
                QueryPoll::Ready(Ok(term.clone()))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }

    /// The selected callee's own type parameters (an impl's or a trait's
    /// `Self` included, which its signature carries) bound to the parts of
    /// the Types its call operands state. A parameter no operand states
    /// stays unbound.
    pub(super) fn bound_callee_parameters(
        &mut self,
        db: &mut crate::db::memo::Database,
        call: NodeId,
        definition: Digest,
        decide_numeric: bool,
    ) -> QueryPoll<BTreeMap<ParamKey, TyTerm>> {
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
        if !matches!(
            self.fact(call).map(|fact| fact.kind()),
            Ok(FactKind::Call | FactKind::MethodCall | FactKind::LangCall)
        ) {
            return QueryPoll::Ready(Ok(BTreeMap::new()));
        }
        let mut bound = BTreeMap::new();
        for (position, operand) in operands.iter().enumerate().skip(offset) {
            let (Some(operand), Ok(ordinal)) = (operand, u32::try_from(position - offset)) else {
                continue;
            };
            let Some(declared) = ready!(self.quiet_parameter_term(db, call, definition, ordinal))
            else {
                continue;
            };
            let Some(actual) = ready!(self.expression_type(db, *operand)) else {
                continue;
            };
            // An unsuffixed literal operand (or a local initialized with one)
            // is an inference variable of the call's parameter; rustc's
            // selection of the parameter's bound can decide it, and a
            // projection of it follows (`foo2(a)` with `a = 42` and one
            // integer `impl Foo`). Only a projection needs that decision.
            let actual = if decide_numeric {
                ready!(self.decide_open(db, *operand, actual))
            } else {
                actual
            };
            super::methods::bind_parameters(&declared, &actual, definition, &mut bound);
        }
        QueryPoll::Ready(Ok(bound))
    }

    /// `actual`, the static Type of `operand`, with its open numeric
    /// variable (through references) replaced by the primitive the numeric
    /// demand decides for the literal it came from. Anything else is
    /// returned as stated.
    fn decide_open(
        &mut self,
        db: &mut crate::db::memo::Database,
        operand: NodeId,
        actual: TyTerm,
    ) -> QueryPoll<TyTerm> {
        let mut references = Vec::new();
        let mut inner = &actual;
        while let TyTerm::Ref(region, mutable, next) = inner {
            references.push((region.clone(), *mutable));
            inner = next;
        }
        if *inner != super::methods::open() {
            return QueryPoll::Ready(Ok(actual));
        }
        let Some(primitive) = ready!(self.decided_numeric(db, operand)) else {
            return QueryPoll::Ready(Ok(actual));
        };
        let mut decided = TyTerm::Primitive(primitive);
        for (region, mutable) in references.into_iter().rev() {
            decided = TyTerm::Ref(region, mutable, Box::new(decided));
        }
        QueryPoll::Ready(Ok(decided))
    }

    /// The unsuffixed literal whose inference variable is the Type of
    /// `node`: the literal itself, or through parentheses, blocks, borrows,
    /// unary `-`/`!` and a local declared by a plain `let` with such an
    /// initializer. Each step moves to a strictly earlier declaration or a
    /// strictly smaller child, so the walk is finite.
    pub(super) fn literal_origin(
        &mut self,
        db: &mut crate::db::memo::Database,
        mut node: NodeId,
    ) -> QueryPoll<Option<(NodeId, bool)>> {
        let plan = self.plan;
        loop {
            let Some(fact) = plan.fact(node) else {
                return QueryPoll::Ready(Ok(None));
            };
            let next = match fact.kind() {
                FactKind::Integer => return QueryPoll::Ready(Ok(Some((node, false)))),
                FactKind::Float => return QueryPoll::Ready(Ok(Some((node, true)))),
                FactKind::Parenthesis | FactKind::Block | FactKind::Borrow => fact.operand(0),
                FactKind::Prefix if fact.numeric_prefix() => fact.operand(0),
                FactKind::Path => {
                    let observed = ready!(db.fetch(
                        &crate::db::provider::SyntaxProvider,
                        crate::db::key::QueryKey::EnteredValue(super::entry(self.key, node))
                    ));
                    let crate::db::value::QueryValue::EnteredValue(observed) =
                        observed.value.as_ref()
                    else {
                        return QueryPoll::Ready(Err(super::fault(
                            "projection literal query variant differs",
                        )));
                    };
                    let Some(identity) = super::selected_local(observed) else {
                        return QueryPoll::Ready(Ok(None));
                    };
                    let Some(pattern) = ready!(self.binding_pattern(db, node, identity)) else {
                        return QueryPoll::Ready(Ok(None));
                    };
                    plan.uses(pattern).iter().find_map(|edge| {
                        let statement = plan.fact(edge.parent)?;
                        (edge.ordinal == 0
                            && statement.kind() == FactKind::Let
                            && statement.annotation().is_none())
                        .then(|| statement.operand(1))
                        .flatten()
                    })
                }
                _ => None,
            };
            match next {
                Some(next) => node = next,
                None => return QueryPoll::Ready(Ok(None)),
            }
        }
    }

    /// The primitive the numeric demand decides for the literal `operand`'s
    /// Type comes from: the same component solver a literal's own goal runs,
    /// started at that literal (a concrete Type from its relations, else
    /// rustc's integer/float fallback of a closed component). A literal being
    /// decided further up has no answer, so the demands are finite: each
    /// nested demand adds a distinct literal to `deciding`.
    fn decided_numeric(
        &mut self,
        db: &mut crate::db::memo::Database,
        operand: NodeId,
    ) -> QueryPoll<Option<crate::ir::op::PrimitiveType>> {
        let Some((literal, floating)) = ready!(self.literal_origin(db, operand)) else {
            return QueryPoll::Ready(Ok(None));
        };
        if self.deciding.contains(&literal) {
            return QueryPoll::Ready(Ok(None));
        }
        let mut nested = Component::new(self.key, self.plan, floating);
        nested.literal_start = true;
        nested.deciding = self.deciding.clone();
        nested.deciding.push(literal);
        nested.expression(Some(literal));
        ready!(nested.solve(db));
        QueryPoll::Ready(Ok(
            match (nested.concrete, nested.pending, nested.unrecorded) {
                (Some((_, primitive)), _, _) => Some(primitive),
                (None, None, false) => Some(if floating {
                    crate::ir::op::PrimitiveType::F64
                } else {
                    crate::ir::op::PrimitiveType::I32
                }),
                _ => None,
            },
        ))
    }

    /// A selected function's declared parameter or result Type as the call
    /// states it: its own type parameters replaced by what the operands'
    /// Types state, then every projection normalized. Whatever stays open is
    /// returned as written.
    pub(super) fn instantiated_declared(
        &mut self,
        db: &mut crate::db::memo::Database,
        call: NodeId,
        definition: Digest,
        term: &TyTerm,
    ) -> QueryPoll<TyTerm> {
        let bound =
            ready!(self.bound_callee_parameters(db, call, definition, contains_projection(term)));
        // Written arguments fix parameters even when no value parameter
        // states them (a parameter can itself be `U::Error`). Reuse the
        // same explicit-Type query as declared results, before normalizing
        // parameter or result projections.
        let term = ready!(self.explicit_arguments(db, call, definition, term.clone()));
        let term = if bound.is_empty() {
            term
        } else {
            substitute(&term, &bound)
        };
        self.normalize_projections(db, &term)
    }
}
