//! Pinned compiler binder client algorithms; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_binders.py.
use super::{
    context::Context,
    facts::SourceFacts,
    model::{Clause, Clauses, Ty},
};
use rustc_type_ir::inherent::Ty as _;
use rustc_type_ir::{
    self as ty,
    data_structures::{DelayedMap, IndexMap as FxIndexMap},
    inherent::*,
    *,
};
/// A delegate used when instantiating bound vars.
///
/// Any implementation must make sure that each bound variable always
/// gets mapped to the same result. `BoundVarReplacer` caches by using
/// a `DelayedMap` which does not cache the first few types it encounters.
pub(crate) trait BoundVarReplacerDelegate<'cx, F: SourceFacts<'cx>> {
    fn replace_region(
        &mut self,
        br: ty::BoundRegion<Context<'cx, F>>,
    ) -> ty::Region<Context<'cx, F>>;
    fn replace_ty(&mut self, bt: ty::BoundTy<Context<'cx, F>>) -> Ty<'cx, F>;
    fn replace_const(&mut self, bc: ty::BoundConst<Context<'cx, F>>) -> ty::Const<Context<'cx, F>>;
}

/// A simple delegate taking 3 mutable functions. The used functions must
/// always return the same result for each bound variable, no matter how
/// frequently they are called.
pub(crate) struct FnMutDelegate<'a, 'cx, F: SourceFacts<'cx>> {
    pub regions:
        &'a mut (dyn FnMut(ty::BoundRegion<Context<'cx, F>>) -> ty::Region<Context<'cx, F>> + 'a),
    pub types: &'a mut (dyn FnMut(ty::BoundTy<Context<'cx, F>>) -> Ty<'cx, F> + 'a),
    pub consts:
        &'a mut (dyn FnMut(ty::BoundConst<Context<'cx, F>>) -> ty::Const<Context<'cx, F>> + 'a),
}

impl<'a, 'cx, F: SourceFacts<'cx>> BoundVarReplacerDelegate<'cx, F> for FnMutDelegate<'a, 'cx, F> {
    fn replace_region(
        &mut self,
        br: ty::BoundRegion<Context<'cx, F>>,
    ) -> ty::Region<Context<'cx, F>> {
        (self.regions)(br)
    }
    fn replace_ty(&mut self, bt: ty::BoundTy<Context<'cx, F>>) -> Ty<'cx, F> {
        (self.types)(bt)
    }
    fn replace_const(&mut self, bc: ty::BoundConst<Context<'cx, F>>) -> ty::Const<Context<'cx, F>> {
        (self.consts)(bc)
    }
}

/// Replaces the escaping bound vars (late bound regions or bound types) in a type.
struct BoundVarReplacer<'cx, F: SourceFacts<'cx>, D> {
    tcx: Context<'cx, F>,

    /// As with `RegionFolder`, represents the index of a binder *just outside*
    /// the ones we have visited.
    current_index: ty::DebruijnIndex,

    delegate: D,

    /// This cache only tracks the `DebruijnIndex` and assumes that it does not matter
    /// for the delegate how often its methods get used.
    cache: DelayedMap<(ty::DebruijnIndex, Ty<'cx, F>), Ty<'cx, F>>,
}

impl<'cx, F: SourceFacts<'cx>, D: BoundVarReplacerDelegate<'cx, F>> BoundVarReplacer<'cx, F, D> {
    fn new(tcx: Context<'cx, F>, delegate: D) -> Self {
        BoundVarReplacer {
            tcx,
            current_index: ty::INNERMOST,
            delegate,
            cache: Default::default(),
        }
    }
}

impl<'cx, F: SourceFacts<'cx>, D> TypeFolder<Context<'cx, F>> for BoundVarReplacer<'cx, F, D>
where
    D: BoundVarReplacerDelegate<'cx, F>,
{
    fn cx(&self) -> Context<'cx, F> {
        self.tcx
    }

    fn fold_binder<T: TypeFoldable<Context<'cx, F>>>(
        &mut self,
        t: ty::Binder<Context<'cx, F>, T>,
    ) -> ty::Binder<Context<'cx, F>, T> {
        self.current_index.shift_in(1);
        let t = t.super_fold_with(self);
        self.current_index.shift_out(1);
        t
    }

    fn fold_ty(&mut self, t: Ty<'cx, F>) -> Ty<'cx, F> {
        match t.kind() {
            ty::Bound(ty::BoundVarIndexKind::Bound(debruijn), bound_ty)
                if debruijn == self.current_index =>
            {
                let ty = self.delegate.replace_ty(bound_ty);
                debug_assert!(!ty.has_vars_bound_above(ty::INNERMOST));
                ty::shift_vars(self.tcx, ty, self.current_index.as_u32())
            }
            _ => {
                if !t.has_vars_bound_at_or_above(self.current_index) {
                    t
                } else if let Some(&t) = self.cache.get(&(self.current_index, t)) {
                    t
                } else {
                    let res = t.super_fold_with(self);
                    assert!(self.cache.insert((self.current_index, t), res));
                    res
                }
            }
        }
    }

    fn fold_region(&mut self, r: ty::Region<Context<'cx, F>>) -> ty::Region<Context<'cx, F>> {
        match r.kind() {
            ty::ReBound(ty::BoundVarIndexKind::Bound(debruijn), br)
                if debruijn == self.current_index =>
            {
                let region = self.delegate.replace_region(br);
                if let ty::ReBound(ty::BoundVarIndexKind::Bound(debruijn1), br) = region.kind() {
                    // If the callback returns a bound region,
                    // that region should always use the INNERMOST
                    // debruijn index. Then we adjust it to the
                    // correct depth.
                    assert_eq!(debruijn1, ty::INNERMOST);
                    ty::Region::new_bound(self.tcx, debruijn, br)
                } else {
                    region
                }
            }
            _ => r,
        }
    }

    fn fold_const(&mut self, ct: ty::Const<Context<'cx, F>>) -> ty::Const<Context<'cx, F>> {
        match ct.kind() {
            ty::ConstKind::Bound(ty::BoundVarIndexKind::Bound(debruijn), bound_const)
                if debruijn == self.current_index =>
            {
                let ct = self.delegate.replace_const(bound_const);
                debug_assert!(!ct.has_vars_bound_above(ty::INNERMOST));
                ty::shift_vars(self.tcx, ct, self.current_index.as_u32())
            }
            _ => ct.super_fold_with(self),
        }
    }

    fn fold_predicate<P: PredicateProxy<Context<'cx, F>>>(&mut self, p: P) -> P {
        if p.has_vars_bound_at_or_above(self.current_index) {
            p.super_fold_with(self)
        } else {
            p
        }
    }

    fn fold_clauses(&mut self, c: Clauses<'cx, F>) -> Clauses<'cx, F> {
        if c.has_vars_bound_at_or_above(self.current_index) {
            c.super_fold_with(self)
        } else {
            c
        }
    }
}

impl<'cx, F: SourceFacts<'cx>> Context<'cx, F> {
    pub(crate) fn replace_escaping_bound_vars_uncached<T: TypeFoldable<Context<'cx, F>>>(
        self,
        value: T,
        delegate: impl BoundVarReplacerDelegate<'cx, F>,
    ) -> T {
        if !value.has_escaping_bound_vars() {
            value
        } else {
            let mut replacer = BoundVarReplacer::new(self, delegate);
            value.fold_with(&mut replacer)
        }
    }

    pub(crate) fn replace_bound_vars_uncached<T: TypeFoldable<Context<'cx, F>>>(
        self,
        value: Binder<Context<'cx, F>, T>,
        delegate: impl BoundVarReplacerDelegate<'cx, F>,
    ) -> T {
        self.replace_escaping_bound_vars_uncached(value.skip_binder(), delegate)
    }

    pub(crate) fn shift_bound_var_indices<T>(self, bound_vars: usize, value: T) -> T
    where
        T: TypeFoldable<Context<'cx, F>>,
    {
        let shift_bv = |bv: ty::BoundVar| bv + bound_vars;
        self.replace_escaping_bound_vars_uncached(
            value,
            FnMutDelegate {
                regions: &mut |r: ty::BoundRegion<Context<'cx, F>>| {
                    ty::Region::new_bound(
                        self,
                        ty::INNERMOST,
                        ty::BoundRegion {
                            var: shift_bv(r.var),
                            kind: r.kind,
                        },
                    )
                },
                types: &mut |t: ty::BoundTy<Context<'cx, F>>| {
                    Ty::new_bound(
                        self,
                        ty::INNERMOST,
                        ty::BoundTy {
                            var: shift_bv(t.var),
                            kind: t.kind,
                        },
                    )
                },
                consts: &mut |c| {
                    ty::Const::new_bound(self, ty::INNERMOST, ty::BoundConst::new(shift_bv(c.var)))
                },
            },
        )
    }

    pub(crate) fn anonymize_bound_vars<T>(
        self,
        value: Binder<Context<'cx, F>, T>,
    ) -> Binder<Context<'cx, F>, T>
    where
        T: TypeFoldable<Context<'cx, F>>,
    {
        struct Anonymize<'a, 'cx, F: SourceFacts<'cx>> {
            tcx: Context<'cx, F>,
            map: &'a mut FxIndexMap<ty::BoundVar, ty::BoundVariableKind<Context<'cx, F>>>,
        }
        impl<'cx, F: SourceFacts<'cx>> BoundVarReplacerDelegate<'cx, F> for Anonymize<'_, 'cx, F> {
            fn replace_region(
                &mut self,
                br: ty::BoundRegion<Context<'cx, F>>,
            ) -> ty::Region<Context<'cx, F>> {
                let entry = self.map.entry(br.var);
                let index = entry.index();
                let var = ty::BoundVar::from_usize(index);
                let kind = entry
                    .or_insert_with(|| ty::BoundVariableKind::Region(ty::BoundRegionKind::Anon))
                    .expect_region();
                let br = ty::BoundRegion { var, kind };
                ty::Region::new_bound(self.tcx, ty::INNERMOST, br)
            }
            fn replace_ty(&mut self, bt: ty::BoundTy<Context<'cx, F>>) -> Ty<'cx, F> {
                let entry = self.map.entry(bt.var);
                let index = entry.index();
                let var = ty::BoundVar::from_usize(index);
                let kind = entry
                    .or_insert_with(|| ty::BoundVariableKind::Ty(ty::BoundTyKind::Anon))
                    .expect_ty();
                Ty::new_bound(self.tcx, ty::INNERMOST, BoundTy { var, kind })
            }
            fn replace_const(
                &mut self,
                bc: ty::BoundConst<Context<'cx, F>>,
            ) -> ty::Const<Context<'cx, F>> {
                let entry = self.map.entry(bc.var);
                let index = entry.index();
                let var = ty::BoundVar::from_usize(index);
                let () = entry
                    .or_insert_with(|| ty::BoundVariableKind::Const)
                    .expect_const();
                ty::Const::new_bound(self.tcx, ty::INNERMOST, ty::BoundConst::new(var))
            }
        }

        let mut map = Default::default();
        let delegate = Anonymize {
            tcx: self,
            map: &mut map,
        };
        let inner = self.replace_escaping_bound_vars_uncached(value.skip_binder(), delegate);
        let bound_vars = self.mk_bound_variable_kinds_from_iter(map.into_values());
        Binder::bind_with_vars(inner, bound_vars)
    }
}
impl<'cx, F: SourceFacts<'cx>> Clause<'cx, F> {
    pub(crate) fn instantiate_supertrait_upstream(
        self,
        tcx: Context<'cx, F>,
        trait_ref: ty::Binder<Context<'cx, F>, ty::TraitRef<Context<'cx, F>>>,
    ) -> Clause<'cx, F> {
        // The interaction between HRTB and supertraits is not entirely
        // obvious. Let me walk you (and myself) through an example.
        //
        // Let's start with an easy case. Consider two traits:
        //
        //     trait Foo<'a>: Bar<'a,'a> { }
        //     trait Bar<'b,'c> { }
        //
        // Now, if we have a trait reference `for<'x> T: Foo<'x>`, then
        // we can deduce that `for<'x> T: Bar<'x,'x>`. Basically, if we
        // knew that `Foo<'x>` (for any 'x) then we also know that
        // `Bar<'x,'x>` (for any 'x). This more-or-less falls out from
        // normal instantiation.
        //
        // In terms of why this is sound, the idea is that whenever there
        // is an impl of `T:Foo<'a>`, it must show that `T:Bar<'a,'a>`
        // holds. So if there is an impl of `T:Foo<'a>` that applies to
        // all `'a`, then we must know that `T:Bar<'a,'a>` holds for all
        // `'a`.
        //
        // Another example to be careful of is this:
        //
        //     trait Foo1<'a>: for<'b> Bar1<'a,'b> { }
        //     trait Bar1<'b,'c> { }
        //
        // Here, if we have `for<'x> T: Foo1<'x>`, then what do we know?
        // The answer is that we know `for<'x,'b> T: Bar1<'x,'b>`. The
        // reason is similar to the previous example: any impl of
        // `T:Foo1<'x>` must show that `for<'b> T: Bar1<'x, 'b>`. So
        // basically we would want to collapse the bound lifetimes from
        // the input (`trait_ref`) and the supertraits.
        //
        // To achieve this in practice is fairly straightforward. Let's
        // consider the more complicated scenario:
        //
        // - We start out with `for<'x> T: Foo1<'x>`. In this case, `'x`
        //   has a De Bruijn index of 1. We want to produce `for<'x,'b> T: Bar1<'x,'b>`,
        //   where both `'x` and `'b` would have a DB index of 1.
        //   The instantiation from the input trait-ref is therefore going to be
        //   `'a => 'x` (where `'x` has a DB index of 1).
        // - The supertrait-ref is `for<'b> Bar1<'a,'b>`, where `'a` is an
        //   early-bound parameter and `'b` is a late-bound parameter with a
        //   DB index of 1.
        // - If we replace `'a` with `'x` from the input, it too will have
        //   a DB index of 1, and thus we'll have `for<'x,'b> Bar1<'x,'b>`
        //   just as we wanted.
        //
        // There is only one catch. If we just apply the instantiation `'a
        // => 'x` to `for<'b> Bar1<'a,'b>`, the instantiation code will
        // adjust the DB index because we instantiating into a binder (it
        // tries to be so smart...) resulting in `for<'x> for<'b>
        // Bar1<'x,'b>` (we have no syntax for this, so use your
        // imagination). Basically the 'x will have DB index of 2 and 'b
        // will have DB index of 1. Not quite what we want. So we apply
        // the instantiation to the *contents* of the trait reference,
        // rather than the trait reference itself (put another way, the
        // instantiation code expects equal binding levels in the values
        // from the instantiation and the value being instantiated into, and
        // this trick achieves that).

        // Working through the second example:
        // trait_ref: for<'x> T: Foo1<'^0.0>; args: [T, '^0.0]
        // predicate: for<'b> Self: Bar1<'a, '^0.0>; args: [Self, 'a, '^0.0]
        // We want to end up with:
        //     for<'x, 'b> T: Bar1<'^0.0, '^0.1>
        // To do this:
        // 1) We must shift all bound vars in predicate by the length
        //    of trait ref's bound vars. So, we would end up with predicate like
        //    Self: Bar1<'a, '^0.1>
        // 2) We can then apply the trait args to this, ending up with
        //    T: Bar1<'^0.0, '^0.1>
        // 3) Finally, to create the final bound vars, we concatenate the bound
        //    vars of the trait ref with those of the predicate:
        //    ['x, 'b]
        let bound_pred = self.kind();
        let pred_bound_vars = bound_pred.bound_vars();
        let trait_bound_vars = trait_ref.bound_vars();
        // 1) Self: Bar1<'a, '^0.0> -> Self: Bar1<'a, '^0.1>
        let shifted_pred =
            tcx.shift_bound_var_indices(trait_bound_vars.len(), bound_pred.skip_binder());
        // 2) Self: Bar1<'a, '^0.1> -> T: Bar1<'^0.0, '^0.1>
        let new = EarlyBinder::bind(tcx, shifted_pred)
            .instantiate(tcx, trait_ref.skip_binder().args)
            .skip_norm_wip();
        // 3) ['x] + ['b] -> ['x, 'b]
        let bound_vars =
            tcx.mk_bound_variable_kinds_from_iter(trait_bound_vars.iter().chain(pred_bound_vars));

        // FIXME: Is it really perf sensitive to use reuse_or_mk_predicate here?
        tcx.predicate(ty::Binder::bind_with_vars(
            PredicateKind::Clause(new),
            bound_vars,
        ))
        .as_clause()
        .expect("upstream substitution constructs a clause")
    }
}
