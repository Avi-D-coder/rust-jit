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
use super::{Component, ConstraintDemand, Step};
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
    ) -> QueryPoll<Option<bool>> {
        let Some(mut term) = ready!(self.expression_type(db, operand)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let steps = self.via.steps.clone();
        for step in &steps {
            let mut head = &term;
            while let TyTerm::Ref(_, _, inner) = head {
                head = inner;
            }
            if let TyTerm::Nominal(definition, _) = head
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
        for operand in [Some(node), other].into_iter().flatten() {
            match ready!(self.self_rhs_projection(db, operand)) {
                Some(true) => verified = true,
                Some(false) => refused = true,
                None => {}
            }
        }
        if verified && !refused {
            self.expression(other);
        } else {
            self.defer(parent, ConstraintDemand::ExpressionRelation);
        }
        QueryPoll::Ready(Ok(()))
    }
}
