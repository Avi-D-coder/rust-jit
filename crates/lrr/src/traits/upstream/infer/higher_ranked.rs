//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::{DbInterner, InferenceContext, Ty};
use crate::traits::upstream::binders::FnMutDelegate;
use rustc_type_ir::{self as ty, TypeFoldable, TypeVisitableExt, inherent::Ty as _};
use tracing::debug;
impl<'db> InferenceContext<'db> {
    pub fn enter_forall_and_leak_universe<T>(&self, binder: ty::Binder<DbInterner<'db>, T>) -> T
    where
        T: TypeFoldable<DbInterner<'db>>,
    {
        // Inlined `no_bound_vars`.
        if !binder.as_ref().skip_binder().has_escaping_bound_vars() {
            return binder.skip_binder();
        }

        let next_universe = self.create_next_universe();

        let delegate = FnMutDelegate {
            regions: &mut |br: ty::BoundRegion<DbInterner<'db>>| {
                ty::Region::new_placeholder(
                    self.interner,
                    ty::PlaceholderRegion::new(next_universe, br),
                )
            },
            types: &mut |bound_ty: ty::BoundTy<DbInterner<'db>>| {
                Ty::new_placeholder(
                    self.interner,
                    ty::PlaceholderType::new(next_universe, bound_ty),
                )
            },
            consts: &mut |bound_const: ty::BoundConst<DbInterner<'db>>| {
                ty::Const::new_placeholder(
                    self.interner,
                    ty::PlaceholderConst::new(next_universe, bound_const),
                )
            },
        };

        debug!(?next_universe);
        self.interner.replace_bound_vars_uncached(binder, delegate)
    }
    pub fn enter_forall<T, U>(
        &self,
        forall: ty::Binder<DbInterner<'db>, T>,
        f: impl FnOnce(T) -> U,
    ) -> U
    where
        T: TypeFoldable<DbInterner<'db>>,
    {
        // FIXME: currently we do nothing to prevent placeholders with the new universe being
        // used after exiting `f`. For example region subtyping can result in outlives constraints
        // that name placeholders created in this function. Nested goals from type relations can
        // also contain placeholders created by this function.
        let value = self.enter_forall_and_leak_universe(forall);
        debug!(?value);
        f(value)
    }
}
