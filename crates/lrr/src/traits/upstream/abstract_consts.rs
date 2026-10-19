//! Pinned compiler abstract-constant expansion and region erasure; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; unchanged originals/licenses: vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_relations.py.
use super::{
    context::Context,
    facts::SourceFacts,
    model::{Clauses, Ty},
};
use rustc_type_ir::{self as ty, inherent::*, *};
use tracing::debug;
pub(super) fn erase_and_anonymize_regions_ty<'cx, F: SourceFacts<'cx>>(
    tcx: Context<'cx, F>,
    ty: Ty<'cx, F>,
) -> Ty<'cx, F> {
    // N.B., use `super_fold_with` here. If we used `fold_with`, it
    // could invoke the `erase_and_anonymize_regions_ty` query recursively.
    ty.super_fold_with(&mut RegionEraserAndAnonymizerVisitor { tcx })
}

impl<'cx, F: SourceFacts<'cx>> Context<'cx, F> {
    /// Returns an equivalent value with all free regions removed and
    /// bound regions anonymized. (note that bound regions are important
    /// for subtyping and generally type equality so *cannot* be removed)
    pub(crate) fn erase_and_anonymize_regions<T>(self, value: T) -> T
    where
        T: TypeFoldable<Context<'cx, F>>,
    {
        // If there's nothing to erase or anonymize, avoid performing the query at all
        if !value.has_type_flags(TypeFlags::HAS_BINDER_VARS | TypeFlags::HAS_FREE_REGIONS) {
            return value;
        }
        debug!("erase_and_anonymize_regions({:?})", value);
        let value1 = value.fold_with(&mut RegionEraserAndAnonymizerVisitor { tcx: self });
        debug!("erase_and_anonymize_regions = {:?}", value1);
        value1
    }
}

struct RegionEraserAndAnonymizerVisitor<'cx, F: SourceFacts<'cx>> {
    tcx: Context<'cx, F>,
}

impl<'cx, F: SourceFacts<'cx>> TypeFolder<Context<'cx, F>>
    for RegionEraserAndAnonymizerVisitor<'cx, F>
{
    fn cx(&self) -> Context<'cx, F> {
        self.tcx
    }

    fn fold_ty(&mut self, ty: Ty<'cx, F>) -> Ty<'cx, F> {
        if !ty.has_type_flags(TypeFlags::HAS_BINDER_VARS | TypeFlags::HAS_FREE_REGIONS) {
            ty
        } else if ty.has_infer() {
            ty.super_fold_with(self)
        } else {
            self.tcx.erase_and_anonymize_regions_ty(ty)
        }
    }

    fn fold_binder<T>(
        &mut self,
        t: ty::Binder<Context<'cx, F>, T>,
    ) -> ty::Binder<Context<'cx, F>, T>
    where
        T: TypeFoldable<Context<'cx, F>>,
    {
        let u = self.tcx.anonymize_bound_vars(t);
        u.super_fold_with(self)
    }

    fn fold_region(&mut self, r: ty::Region<Context<'cx, F>>) -> ty::Region<Context<'cx, F>> {
        // We must not erase bound regions. `for<'a> fn(&'a ())` and
        // `fn(&'free ())` are different types: they may implement different
        // traits and have a different `TypeId`.
        match r.kind() {
            ty::ReBound(..) => r,
            _ => self.tcx.intern_region(ty::ReErased),
        }
    }

    fn fold_const(&mut self, ct: ty::Const<Context<'cx, F>>) -> ty::Const<Context<'cx, F>> {
        if ct.has_type_flags(TypeFlags::HAS_BINDER_VARS | TypeFlags::HAS_FREE_REGIONS) {
            ct.super_fold_with(self)
        } else {
            ct
        }
    }

    fn fold_predicate<P: PredicateProxy<Context<'cx, F>>>(&mut self, p: P) -> P {
        if p.has_type_flags(TypeFlags::HAS_BINDER_VARS | TypeFlags::HAS_FREE_REGIONS) {
            p.super_fold_with(self)
        } else {
            p
        }
    }

    fn fold_clauses(&mut self, c: Clauses<'cx, F>) -> Clauses<'cx, F> {
        if c.has_type_flags(TypeFlags::HAS_BINDER_VARS | TypeFlags::HAS_FREE_REGIONS) {
            c.super_fold_with(self)
        } else {
            c
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> Context<'cx, F> {
    pub(crate) fn expand_abstract_consts<T: TypeFoldable<Context<'cx, F>>>(self, ac: T) -> T {
        struct Expander<'cx, F: SourceFacts<'cx>> {
            tcx: Context<'cx, F>,
        }

        impl<'cx, F: SourceFacts<'cx>> TypeFolder<Context<'cx, F>> for Expander<'cx, F> {
            fn cx(&self) -> Context<'cx, F> {
                self.tcx
            }
            fn fold_ty(&mut self, ty: Ty<'cx, F>) -> Ty<'cx, F> {
                if ty.has_type_flags(ty::TypeFlags::HAS_CONST_ALIAS) {
                    ty.super_fold_with(self)
                } else {
                    ty
                }
            }
            fn fold_const(&mut self, c: ty::Const<Context<'cx, F>>) -> ty::Const<Context<'cx, F>> {
                let ct = match c.kind() {
                    ty::ConstKind::Alias(_, alias_const) => {
                        let def_id = match alias_const.kind {
                            ty::AliasConstKind::Projection { def_id }
                            | ty::AliasConstKind::InherentSelf { def_id }
                            | ty::AliasConstKind::InherentImpl { def_id }
                            | ty::AliasConstKind::Free { def_id }
                            | ty::AliasConstKind::Anon { def_id } => def_id,
                        };
                        match self.tcx.services().abstract_const(self.tcx, def_id) {
                            Err(e) => ty::Const::new_error(self.tcx, e),
                            Ok(Some(bac)) => {
                                let args = self.tcx.erase_and_anonymize_regions(alias_const.args);
                                let bac = bac.instantiate(self.tcx, args).skip_norm_wip();
                                return bac.fold_with(self);
                            }
                            Ok(None) => c,
                        }
                    }
                    _ => c,
                };
                ct.super_fold_with(self)
            }
        }
        ac.fold_with(&mut Expander { tcx: self })
    }
}
