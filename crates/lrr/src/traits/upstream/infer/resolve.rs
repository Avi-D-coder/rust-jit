//! Pinned inference variable/resolution client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference_context.py.
use super::{Const, DbInterner, InferenceContext, PreparedProvider, Ty};
use rustc_type_ir::{
    self as ty, PredicateProxy, TypeFolder, TypeSuperFoldable, TypeVisitableExt,
    data_structures::DelayedMap,
};
type Clauses<'db> = crate::traits::upstream::model::Clauses<'db, PreparedProvider<'db>>;
pub struct DeepResolverIgnoringRegions<'a, 'db> {
    infcx: &'a InferenceContext<'db>,
    /// We're able to use a cache here as the folder does
    /// not have any mutable state.
    cache: DelayedMap<Ty<'db>, Ty<'db>>,
}

impl<'a, 'db> DeepResolverIgnoringRegions<'a, 'db> {
    #[inline]
    pub fn new(infcx: &'a InferenceContext<'db>) -> Self {
        DeepResolverIgnoringRegions {
            infcx,
            cache: Default::default(),
        }
    }
}

impl<'a, 'db> TypeFolder<DbInterner<'db>> for DeepResolverIgnoringRegions<'a, 'db> {
    fn cx(&self) -> DbInterner<'db> {
        self.infcx.interner
    }

    #[inline]
    fn fold_ty(&mut self, t: Ty<'db>) -> Ty<'db> {
        if !t.has_non_region_infer() {
            t // micro-optimize -- if there is nothing in this type that this fold affects...
        } else if let Some(&ty) = self.cache.get(&t) {
            ty
        } else {
            let shallow = self.infcx.shallow_resolve(t);
            let res = shallow.super_fold_with(self);
            assert!(self.cache.insert(t, res));
            res
        }
    }

    fn fold_const(&mut self, ct: Const<'db>) -> Const<'db> {
        if !ct.has_non_region_infer() {
            ct // micro-optimize -- if there is nothing in this const that this fold affects...
        } else {
            let ct = self.infcx.shallow_resolve_const(ct);
            ct.super_fold_with(self)
        }
    }

    fn fold_predicate<P: PredicateProxy<DbInterner<'db>>>(&mut self, p: P) -> P {
        if !p.has_non_region_infer() {
            p
        } else {
            p.super_fold_with(self)
        }
    }

    fn fold_clauses(&mut self, c: Clauses<'db>) -> Clauses<'db> {
        if !c.has_non_region_infer() {
            c
        } else {
            c.super_fold_with(self)
        }
    }
}
