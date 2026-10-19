//! Pinned compiler type-pattern, existential and const-expression relations; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; unchanged originals/licenses: vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_relations.py.
use super::{
    constant_expression::ExprKind,
    context::Context,
    facts::SourceFacts,
    model::{Existentials, ExprConst, Pattern},
};
use rustc_type_ir::{
    self as ty,
    error::{ExpectedFound, TypeError},
    inherent::*,
    relate::{Relate, RelateResult, TypeRelation},
};
use std::iter;
impl<'cx, F: SourceFacts<'cx>> Relate<Context<'cx, F>> for Pattern<'cx, F> {
    #[inline]
    fn relate<R: TypeRelation<Context<'cx, F>>>(
        relation: &mut R,
        a: Self,
        b: Self,
    ) -> RelateResult<Context<'cx, F>, Self> {
        let tcx = relation.cx();
        match (&*a, &*b) {
            (
                &ty::PatternKind::Range {
                    start: start_a,
                    end: end_a,
                },
                &ty::PatternKind::Range {
                    start: start_b,
                    end: end_b,
                },
            ) => {
                let start = relation.relate(start_a, start_b)?;
                let end = relation.relate(end_a, end_b)?;
                Ok(tcx.pattern(ty::PatternKind::Range { start, end }))
            }
            (ty::PatternKind::NotNull, ty::PatternKind::NotNull) => Ok(a),
            (&ty::PatternKind::Or(a), &ty::PatternKind::Or(b)) => {
                if a.len() != b.len() {
                    return Err(TypeError::Mismatch);
                }
                let v = iter::zip(a, b).map(|(a, b)| relation.relate(a, b));
                let patterns = tcx.mk_patterns_from_iter(v)?;
                Ok(tcx.pattern(ty::PatternKind::Or(patterns)))
            }
            (
                ty::PatternKind::NotNull | ty::PatternKind::Range { .. } | ty::PatternKind::Or(_),
                _,
            ) => Err(TypeError::Mismatch),
        }
    }
}

impl<'cx, F: SourceFacts<'cx>> Relate<Context<'cx, F>> for Existentials<'cx, F> {
    fn relate<R: TypeRelation<Context<'cx, F>>>(
        relation: &mut R,
        a: Self,
        b: Self,
    ) -> RelateResult<Context<'cx, F>, Self> {
        let tcx = relation.cx();
        // Fast path for when the auto traits do not match, or if the principals
        // are from different traits and therefore the projections definitely don't
        // match up.
        if a.len() != b.len() {
            return Err(TypeError::ExistentialMismatch(ExpectedFound::new(a, b)));
        }
        let v =
            iter::zip(a, b).map(
                |(ep_a, ep_b)| match (ep_a.skip_binder(), ep_b.skip_binder()) {
                    (ty::ExistentialPredicate::Trait(a), ty::ExistentialPredicate::Trait(b)) => {
                        Ok(ep_a.rebind(ty::ExistentialPredicate::Trait(
                            relation
                                .relate(ep_a.rebind(a), ep_b.rebind(b))?
                                .skip_binder(),
                        )))
                    }
                    (
                        ty::ExistentialPredicate::Projection(a),
                        ty::ExistentialPredicate::Projection(b),
                    ) => Ok(ep_a.rebind(ty::ExistentialPredicate::Projection(
                        relation
                            .relate(ep_a.rebind(a), ep_b.rebind(b))?
                            .skip_binder(),
                    ))),
                    (
                        ty::ExistentialPredicate::AutoTrait(a),
                        ty::ExistentialPredicate::AutoTrait(b),
                    ) if a == b => Ok(ep_a.rebind(ty::ExistentialPredicate::AutoTrait(a))),
                    _ => Err(TypeError::ExistentialMismatch(ExpectedFound::new(a, b))),
                },
            );
        tcx.mk_poly_existential_predicates_from_iter(v)
    }
}

impl<'cx, F: SourceFacts<'cx>> Relate<Context<'cx, F>> for ExprConst<'cx, F> {
    fn relate<R: TypeRelation<Context<'cx, F>>>(
        relation: &mut R,
        ae: ExprConst<'cx, F>,
        be: ExprConst<'cx, F>,
    ) -> RelateResult<Context<'cx, F>, ExprConst<'cx, F>> {
        // FIXME(generic_const_exprs): is it possible to relate two consts which are not identical
        // exprs? Should we care about that?
        // FIXME(generic_const_exprs): relating the `ty()`s is a little weird since it is supposed to
        // ICE If they mismatch. Unfortunately `ConstKind::Expr` is a little special and can be thought
        // of as being generic over the argument types, however this is implicit so these types don't get
        // related when we relate the args of the item this const arg is for.
        match (ae.kind, be.kind) {
            (ExprKind::Binop(a_binop), ExprKind::Binop(b_binop)) if a_binop == b_binop => {}
            (ExprKind::UnOp(a_unop), ExprKind::UnOp(b_unop)) if a_unop == b_unop => {}
            (ExprKind::FunctionCall, ExprKind::FunctionCall) => {}
            (ExprKind::Cast(a_kind), ExprKind::Cast(b_kind)) if a_kind == b_kind => {}
            _ => return Err(TypeError::Mismatch),
        }

        let args = relation.relate(ae.args(), be.args())?;
        Ok(ExprConst::new(ae.kind, args))
    }
}
