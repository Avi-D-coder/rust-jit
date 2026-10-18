//! Comparison operands that hold a numeric variable inside an aggregate.
//!
//! `x == Some(243)`, `y == (217, true)` and `a == [1, 2, 3, 4]` compare
//! aggregates whose `PartialEq`/`PartialOrd` impls (tuples, arrays, slices,
//! references and the standard containers, derived impls) take the other
//! operand of the same Type: `Rhs = Self`. A numeric variable at any depth of
//! one operand then has the Type at the same depth of the other, exactly as
//! the scalar comparison relates its operands. The relation is stated only
//! where the compared operand's static Type is known and no Type the
//! projection descends through has an impl with an explicit other `Rhs`
//! (`impl PartialEq<Other> for T`), in which case selection by the other
//! operand's Type is the dispatcher's and the relation stays unrecorded.
use super::{Component, ConstraintDemand, Step, Via};
use crate::{
    core::digest::Digest,
    db::{memo::Database, poll::QueryPoll},
    ir::pc::NodeId,
    syntax::ast_adapter::TokenRecipe,
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

/// Whether the impl header (`impl<..> PartialEq<Rhs> for Self`) names a
/// comparison trait with an explicit `Rhs` other than its own self type.
fn explicit_other_rhs(tokens: &[TokenRecipe]) -> bool {
    let text: Vec<&str> = tokens.iter().map(|token| token.text.as_str()).collect();
    let Some(trait_at) = text
        .iter()
        .position(|token| matches!(*token, "PartialEq" | "PartialOrd"))
    else {
        return false;
    };
    if text.get(trait_at + 1) != Some(&"<") {
        return false;
    }
    let mut depth = 0i32;
    let mut close = None;
    for (offset, token) in text[trait_at + 1..].iter().enumerate() {
        match *token {
            "<" => depth += 1,
            ">" => {
                depth -= 1;
                if depth == 0 {
                    close = Some(trait_at + 1 + offset);
                    break;
                }
            }
            ">>" => {
                depth -= 2;
                if depth <= 0 {
                    close = Some(trait_at + 1 + offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(close) = close else {
        return true;
    };
    let rhs = &text[trait_at + 2..close];
    let Some(for_at) = text[close..].iter().position(|token| *token == "for") else {
        return true;
    };
    let rest = &text[close + for_at + 1..];
    let end = rest
        .iter()
        .position(|token| matches!(*token, "where" | "{"))
        .unwrap_or(rest.len());
    rhs != &rest[..end]
}

impl<'a> Component<'a> {
    /// Whether every nominal Type the member's projection descends through
    /// (starting from the static Type of `operand`, one of the compared
    /// operands) has only `Rhs = Self` comparison impls: `None` when the
    /// operand's Type is not stated.
    fn self_rhs_projection(
        &mut self,
        db: &mut Database,
        operand: NodeId,
        steps: &[Step],
    ) -> QueryPoll<Option<bool>> {
        let Some(mut term) = ready!(self.expression_type(db, operand)) else {
            return QueryPoll::Ready(Ok(None));
        };
        for step in steps {
            let mut head = &term;
            while let TyTerm::Ref(_, _, inner) = head {
                head = inner;
            }
            // A growable array compares element-wise with every sequence
            // (`Vec<T>: PartialEq<[U; N]>`, `PartialEq<[U]>`,
            // `PartialEq<Vec<U>>`): those impls' `T: PartialEq<U>` bounds
            // relate elements, which hold primitive numbers of one Type.
            if let TyTerm::Nominal(definition, _) = head
                && ready!(self.vec_elements(db, head)).is_none()
                && !ready!(self.self_rhs_only(db, *definition))
            {
                return QueryPoll::Ready(Ok(Some(false)));
            }
            let step: &Step = step;
            term = match ready!(self.project_term(db, &term, std::slice::from_ref(step))) {
                Some(next) => next,
                None => return QueryPoll::Ready(Ok(Some(false))),
            };
        }
        QueryPoll::Ready(Ok(Some(true)))
    }

    /// Whether the nominal `definition` has no comparison impl with an
    /// explicit `Rhs` other than itself. An impl the sources do not outline
    /// (a derive's generated impl) takes the default `Rhs = Self`.
    fn self_rhs_only(&mut self, db: &mut Database, definition: Digest) -> QueryPoll<bool> {
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(false));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        for imp in members.impls_of(definition) {
            let compares = imp.trait_path.as_ref().is_some_and(|path| {
                matches!(
                    path.last(),
                    Some(crate::syntax::imports::ImportSegment::Name(name))
                        if name == "PartialEq" || name == "PartialOrd"
                )
            });
            if !compares {
                continue;
            }
            // The impl item is the one whose owner is the impl's identity.
            let item = environment.sources().iter().find_map(|source| {
                source.outline().items.iter().find(|item| {
                    item.def_key.item_kind == crate::source::manifest::ItemKind::Impl
                        && item.def_key.enclosing_impl == Some(imp.identity)
                })
            });
            if item.is_some_and(|item| explicit_other_rhs(&item.header.tokens)) {
                return QueryPoll::Ready(Ok(false));
            }
        }
        QueryPoll::Ready(Ok(true))
    }

    /// The current member's projection as the other comparison operand
    /// states it: a growable array's element is `TypeArgument(0)` to the
    /// declared Types that name it and `Element` to the arrays and slices it
    /// compares with (and to its own element projection), so the position
    /// of a `Vec<T>` operand is rewritten to the `Element` step.
    fn mirrored_via(
        &mut self,
        db: &mut Database,
        operand: NodeId,
        other: Option<NodeId>,
    ) -> QueryPoll<Via> {
        let mut via = self.via.clone();
        // The sequence impls compare through any references (`&[T] ==
        // [U; N]`): a leading dereference the other operand's Type has no
        // reference for does not apply to it.
        if let Some(other) = other
            && let Some(mut other_term) = ready!(self.expression_type(db, other))
        {
            let mut kept = Vec::with_capacity(via.steps.len());
            let mut leading = true;
            for step in &via.steps {
                if leading && *step == Step::Deref {
                    if let TyTerm::Ref(_, _, inner) = other_term {
                        other_term = *inner;
                        kept.push(*step);
                    }
                    continue;
                }
                leading = false;
                kept.push(*step);
            }
            via.steps = kept;
        }
        let Some(mut term) = ready!(self.expression_type(db, operand)) else {
            return QueryPoll::Ready(Ok(via));
        };
        let source = self.via.steps.clone();
        let mut position = 0usize;
        for step in &source {
            let mut head = &term;
            while let TyTerm::Ref(_, _, inner) = head {
                head = inner;
            }
            // The mirrored list dropped some leading dereferences: find this
            // step's place in it.
            let in_mirror = via.steps.len() - (source.len() - position).min(via.steps.len());
            if *step == Step::TypeArgument(0)
                && ready!(self.vec_elements(db, head)).is_some()
                && let Some(slot) = via.steps.get_mut(in_mirror)
            {
                *slot = Step::Element;
            }
            position += 1;
            term = match ready!(self.project_term(db, &term, std::slice::from_ref(step))) {
                Some(next) => next,
                None => break,
            };
        }
        QueryPoll::Ready(Ok(via))
    }

    /// The comparison `parent` of the current member `node` when the member
    /// sits inside an aggregate: both operands hold the same Type at that
    /// depth, provided no operand whose Type is stated has an impl naming
    /// another `Rhs` (the impl may belong to either operand's Type) and at
    /// least one operand's Type is stated.
    pub(super) fn aggregate_comparison(
        &mut self,
        db: &mut Database,
        node: NodeId,
        parent: NodeId,
        other: Option<NodeId>,
    ) -> QueryPoll<()> {
        let mut verified = false;
        let mut refused = false;
        // The other operand holds the member at the mirrored projection.
        let mirrored = ready!(self.mirrored_via(db, node, other));
        let own = self.via.steps.clone();
        for (operand, steps) in [(Some(node), own), (other, mirrored.steps.clone())] {
            let Some(operand) = operand else {
                continue;
            };
            match ready!(self.self_rhs_projection(db, operand, &steps)) {
                Some(true) => verified = true,
                Some(false) => refused = true,
                None => {}
            }
        }
        if verified && !refused {
            self.expression_via(other, mirrored);
        } else {
            self.defer(parent, ConstraintDemand::ExpressionRelation);
        }
        QueryPoll::Ready(Ok(()))
    }
}
