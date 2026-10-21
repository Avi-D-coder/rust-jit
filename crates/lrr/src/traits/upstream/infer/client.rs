//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::opaque_types::{OpaqueTypeStorageEntries, ProvisionalHiddenType};
use super::variables::BoundRegionConversionTime;
use super::{
    Const, DbInterner, InferenceContext, OpaqueTypeKey, PreparedProvider, Region, Span, Ty,
};
use super::{type_variable::TypeVariableValue, unify_key::ConstVariableValue};
use crate::traits::upstream::{
    identity::DefinitionId as DefId, model::ErrorToken as ErrorGuaranteed,
};
use rustc_type_ir::data_structures::SsoHashMap;
use rustc_type_ir::inherent::{Term as _, Ty as _};
use rustc_type_ir::{
    self as ty, InferCtxtLike, Interner, TypeFoldable, TypeSuperFoldable, TypeVisitableExt,
    inherent::*, relate::combine::PredicateEmittingRelation, solve::TyOrConstInferVar,
};
use std::range::RangeInclusive;
type RelateResult<'db, T> = ty::relate::RelateResult<DbInterner<'db>, T>;
const DUMMY_SP: Span = Span {
    origin: crate::core::digest::Digest([0; 32]),
    start: 0,
    end: 0,
};
impl<'db> rustc_type_ir::InferCtxtLike for InferenceContext<'db> {
    type Interner = DbInterner<'db>;

    fn cx(&self) -> DbInterner<'db> {
        self.interner
    }

    fn next_trait_solver(&self) -> bool {
        // Shared InferCtxtLike default: noncompiler clients use the new solver.
        true
    }

    fn enable_next_solver_overflow_fcw(&self) -> bool {
        self.enable_next_solver_overflow_fcw.get()
    }

    fn disable_trait_solver_fast_paths(&self) -> bool {
        self.require_source_root();
        self.interner.services().disable_trait_solver_fast_paths()
    }

    fn typing_mode_raw(&self) -> ty::TypingMode<DbInterner<'db>> {
        self.typing_mode
    }

    fn universe(&self) -> ty::UniverseIndex {
        super::InferenceStorage::universe(self)
    }

    fn create_next_universe(&self) -> ty::UniverseIndex {
        super::InferenceStorage::create_next_universe(self)
    }

    fn insert_placeholder_assumptions(
        &self,
        u: ty::UniverseIndex,
        assumptions: rustc_type_ir::region_constraint::Assumptions<DbInterner<'db>>,
    ) {
        self.insert_placeholder_assumptions(u, assumptions);
    }

    fn get_placeholder_assumptions(
        &self,
        u: ty::UniverseIndex,
    ) -> rustc_type_ir::region_constraint::Assumptions<DbInterner<'db>> {
        self.get_placeholder_assumptions(u)
    }

    fn get_solver_region_constraint(
        &self,
    ) -> rustc_type_ir::region_constraint::RegionConstraint<DbInterner<'db>> {
        self.get_solver_region_constraint().without_spans()
    }

    fn has_placeholder_assumptions(&self, range: RangeInclusive<ty::UniverseIndex>) -> bool {
        self.has_placeholder_assumptions(range)
    }

    fn overwrite_solver_region_constraint(
        &self,
        constraint: rustc_type_ir::region_constraint::RegionConstraint<DbInterner<'db>>,
        span: Span,
    ) {
        self.overwrite_solver_region_constraint(constraint.with_spans(span));
    }

    fn universe_of_ty(&self, vid: ty::TyVid) -> Option<ty::UniverseIndex> {
        match self.try_resolve_ty_var(vid) {
            Err(universe) => Some(universe),
            Ok(_) => None,
        }
    }

    fn universe_of_region(&self, lt: ty::RegionVid) -> Option<ty::UniverseIndex> {
        match self
            .inner
            .borrow_mut()
            .unwrap_region_constraints()
            .try_resolve_region_var(lt)
        {
            Err(universe) => Some(universe),
            Ok(_) => None,
        }
    }

    fn universe_of_const(&self, ct: ty::ConstVid) -> Option<ty::UniverseIndex> {
        match self.try_resolve_const_var(ct) {
            Err(universe) => Some(universe),
            Ok(_) => None,
        }
    }

    fn root_ty_var(&self, var: ty::TyVid) -> ty::TyVid {
        self.root_var(var)
    }

    fn sub_unification_table_root_var(&self, var: ty::TyVid) -> ty::TyVid {
        self.sub_unification_table_root_var(var)
    }

    #[inline]
    fn is_sub_unification_table_root_var(&self, vid: ty::TyVid) -> bool {
        self.inner
            .borrow()
            .type_variable_storage
            .sub_unification_table_ref()
            .try_probe_value(vid)
            .is_some()
    }

    fn root_const_var(&self, var: ty::ConstVid) -> ty::ConstVid {
        self.root_const_var(var)
    }

    fn shallow_resolve_ty_var(&self, vid: ty::TyVid) -> Ty<'db> {
        self.shallow_resolve_ty_var(vid)
    }

    fn shallow_resolve_int_var(&self, vid: ty::IntVid) -> Ty<'db> {
        self.shallow_resolve_int_var(vid)
    }

    fn shallow_resolve_float_var(&self, vid: ty::FloatVid) -> Ty<'db> {
        self.shallow_resolve_float_var(vid)
    }

    fn shallow_resolve_const_var(&self, vid: ty::ConstVid) -> Const<'db> {
        self.shallow_resolve_const_var(vid)
    }

    fn shallow_resolve_region_var(&self, vid: ty::RegionVid) -> ty::Region<DbInterner<'db>> {
        self.inner
            .borrow_mut()
            .unwrap_region_constraints()
            .shallow_resolve_region_var(self.interner, vid)
    }

    fn ty_or_const_infer_var_changed(&self, var: TyOrConstInferVar) -> bool {
        self.ty_or_const_infer_var_changed(var)
    }

    fn next_region_infer(&self) -> ty::Region<DbInterner<'db>> {
        self.next_region_var(DUMMY_SP)
    }

    fn next_ty_infer(&self) -> Ty<'db> {
        self.next_ty_var(DUMMY_SP)
    }

    fn next_const_infer(&self) -> Const<'db> {
        self.next_const_var(DUMMY_SP)
    }

    fn fresh_args_for_item(
        &self,
        def_id: DefId,
    ) -> crate::traits::upstream::model::Args<'db, PreparedProvider<'db>> {
        self.fresh_args_for_item(DUMMY_SP, def_id)
    }

    fn instantiate_binder_with_infer<T: TypeFoldable<DbInterner<'db>> + Copy>(
        &self,
        value: ty::Binder<DbInterner<'db>, T>,
    ) -> T {
        self.instantiate_binder_with_fresh_vars(
            DUMMY_SP,
            BoundRegionConversionTime::HigherRankedType,
            value,
        )
    }

    fn enter_forall_without_assumptions<T: TypeFoldable<DbInterner<'db>>, U>(
        &self,
        value: ty::Binder<DbInterner<'db>, T>,
        f: impl FnOnce(T) -> U,
    ) -> U {
        self.enter_forall(value, f)
    }

    fn enter_forall_with_empty_assumptions<T: TypeFoldable<DbInterner<'db>>, U>(
        &self,
        value: ty::Binder<DbInterner<'db>, T>,
        f: impl FnOnce(T) -> U,
    ) -> U {
        self.enter_forall(value, |value| {
            let u = self.universe();
            self.placeholder_assumptions_for_next_solver
                .borrow_mut()
                .insert(u, rustc_type_ir::region_constraint::Assumptions::empty());
            f(value)
        })
    }

    fn equate_ty_vids_raw(&self, a: ty::TyVid, b: ty::TyVid) {
        self.inner.borrow_mut().type_variables().equate(a, b);
    }

    fn sub_unify_ty_vids_raw(&self, a: ty::TyVid, b: ty::TyVid) {
        self.sub_unify_ty_vids_raw(a, b);
    }

    fn equate_int_vids_raw(&self, a: ty::IntVid, b: ty::IntVid) {
        self.inner.borrow_mut().int_unification_table().union(a, b);
    }

    fn equate_float_vids_raw(&self, a: ty::FloatVid, b: ty::FloatVid) {
        self.inner
            .borrow_mut()
            .float_unification_table()
            .union(a, b);
    }

    fn equate_const_vids_raw(&self, a: ty::ConstVid, b: ty::ConstVid) {
        self.inner
            .borrow_mut()
            .const_unification_table()
            .union(a, b);
    }

    fn instantiate_ty_var_raw(&self, vid: ty::TyVid, ty: Ty<'db>) {
        let ty = lower_universe(self, self.try_resolve_ty_var(vid).unwrap_err(), ty);

        self.inner
            .borrow_mut()
            .type_variables()
            .instantiate(vid, ty);
    }

    fn instantiate_const_var_raw(&self, vid: ty::ConstVid, ct: Const<'db>) {
        let ct = lower_universe(self, self.try_resolve_const_var(vid).unwrap_err(), ct);

        self.inner
            .borrow_mut()
            .const_unification_table()
            .union_value(vid, ConstVariableValue::Known { value: ct });
    }

    fn instantiate_ty_var<R: PredicateEmittingRelation<Self>>(
        &self,
        relation: &mut R,
        target_is_expected: bool,
        target_vid: ty::TyVid,
        instantiation_variance: ty::Variance,
        source_ty: Ty<'db>,
    ) -> RelateResult<'db, ()> {
        self.instantiate_ty_var(
            relation,
            target_is_expected,
            target_vid,
            instantiation_variance,
            source_ty,
        )
    }

    fn instantiate_int_var_raw(&self, vid: ty::IntVid, value: ty::IntVarValue) {
        self.inner
            .borrow_mut()
            .int_unification_table()
            .union_value(vid, value);
    }

    fn instantiate_float_var_raw(&self, vid: ty::FloatVid, value: ty::FloatVarValue) {
        self.inner
            .borrow_mut()
            .float_unification_table()
            .union_value(vid, value);
    }

    fn instantiate_const_var<R: PredicateEmittingRelation<Self>>(
        &self,
        relation: &mut R,
        target_is_expected: bool,
        target_vid: ty::ConstVid,
        source_ct: Const<'db>,
    ) -> RelateResult<'db, ()> {
        self.instantiate_const_var(relation, target_is_expected, target_vid, source_ct)
    }

    fn set_tainted_by_errors(&self, e: ErrorGuaranteed) {
        self.set_tainted_by_errors(e)
    }

    fn shallow_resolve(&self, ty: Ty<'db>) -> Ty<'db> {
        self.shallow_resolve(ty)
    }
    fn shallow_resolve_const(&self, ct: Const<'db>) -> Const<'db> {
        self.shallow_resolve_const(ct)
    }

    fn deeply_resolve_ignoring_regions<T>(&self, value: T) -> T
    where
        T: TypeFoldable<DbInterner<'db>>,
    {
        self.deeply_resolve_ignoring_regions(value)
    }

    fn probe<T>(&self, probe: impl FnOnce() -> T) -> T {
        super::InferenceStorage::probe(self, |_| probe())
    }

    fn commit_if_ok<T, E>(&self, f: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
        super::InferenceStorage::commit_if_ok(self, |_| f())
    }

    fn sub_regions(
        &self,
        sub: ty::Region<DbInterner<'db>>,
        sup: ty::Region<DbInterner<'db>>,
        vis: ty::solve::VisibleForLeakCheck,
        span: Span,
    ) {
        self.inner
            .borrow_mut()
            .unwrap_region_constraints()
            .make_subregion(span, sub, sup, vis);
    }

    fn equate_regions(
        &self,
        a: ty::Region<DbInterner<'db>>,
        b: ty::Region<DbInterner<'db>>,
        vis: ty::solve::VisibleForLeakCheck,
        span: Span,
    ) {
        self.inner
            .borrow_mut()
            .unwrap_region_constraints()
            .make_eqregion(span, a, b, vis);
    }

    fn register_solver_region_constraint(
        &self,
        c: rustc_type_ir::region_constraint::RegionConstraint<DbInterner<'db>>,
        span: Span,
    ) {
        self.register_solver_region_constraint(c.with_spans(span));
    }

    fn register_ty_outlives(&self, ty: Ty<'db>, r: ty::Region<DbInterner<'db>>, span: Span) {
        self.register_type_outlives_constraint(ty, r, span);
    }

    type OpaqueTypeStorageEntries = OpaqueTypeStorageEntries;
    #[inline]
    fn opaque_types_storage_num_entries(&self) -> OpaqueTypeStorageEntries {
        self.inner.borrow_mut().opaque_types().num_entries()
    }
    fn clone_opaque_types_lookup_table(&self) -> Vec<(OpaqueTypeKey<'db>, Ty<'db>)> {
        self.inner
            .borrow_mut()
            .opaque_types()
            .iter_lookup_table()
            .map(|(k, h)| (k, h.ty))
            .collect()
    }
    fn clone_duplicate_opaque_types(&self) -> Vec<(OpaqueTypeKey<'db>, Ty<'db>)> {
        self.inner
            .borrow_mut()
            .opaque_types()
            .iter_duplicate_entries()
            .map(|(k, h)| (k, h.ty))
            .collect()
    }
    fn clone_opaque_types_added_since(
        &self,
        prev_entries: OpaqueTypeStorageEntries,
    ) -> Vec<(OpaqueTypeKey<'db>, Ty<'db>)> {
        self.inner
            .borrow_mut()
            .opaque_types()
            .opaque_types_added_since(prev_entries)
            .map(|(k, h)| (k, h.ty))
            .collect()
    }
    fn opaques_with_sub_unified_hidden_type(
        &self,
        ty: ty::TyVid,
    ) -> Vec<ty::OpaqueAliasTy<DbInterner<'db>>> {
        self.opaques_with_sub_unified_hidden_type(ty)
    }

    fn register_hidden_type_in_storage(
        &self,
        opaque_type_key: OpaqueTypeKey<'db>,
        hidden_ty: Ty<'db>,
        span: Span,
    ) -> Option<Ty<'db>> {
        self.register_hidden_type_in_storage(
            opaque_type_key,
            ProvisionalHiddenType {
                span,
                ty: hidden_ty,
            },
        )
    }
    fn add_duplicate_opaque_type(
        &self,
        opaque_type_key: OpaqueTypeKey<'db>,
        hidden_ty: Ty<'db>,
        span: Span,
    ) {
        self.inner.borrow_mut().opaque_types().add_duplicate(
            opaque_type_key,
            ProvisionalHiddenType {
                span,
                ty: hidden_ty,
            },
        )
    }

    fn reset_opaque_types(&self) {
        let _ = self.take_opaque_types();
    }
}

fn lower_universe<'db, T: TypeFoldable<DbInterner<'db>> + Copy>(
    infcx: &InferenceContext<'db>,
    for_universe: ty::UniverseIndex,
    value: T,
) -> T {
    let value = value.fold_with(&mut LowerUniverseFolder {
        infcx,
        for_universe,
        cache: Default::default(),
    });

    // This assertion is needed because we don't lower the universes of placeholders
    // in the folder.
    #[cfg(debug_assertions)]
    {
        let value_universe = ty::max_universe(infcx, value);
        assert!(
            for_universe.can_name(value_universe),
            "variable in universe {:?} can't name value in universe {:?}",
            for_universe,
            value_universe,
        );
    }

    value
}

/// Canonicalizing inputs puts all inference variables and placeholders
/// into the root universe.
///
/// This means when instantiating the query response we need to pull
/// down the universe of returned `var_values` to the universe of
/// the inference variable in `orig_values`.
///
/// This folder is similar to the `Generalizer`, except that it simply
/// structurally folds non-rigid aliases as these should have already
/// been generalized in the query so we shouldn't try to do it again.
struct LowerUniverseFolder<'a, 'db> {
    infcx: &'a InferenceContext<'db>,
    for_universe: ty::UniverseIndex,
    cache: SsoHashMap<Ty<'db>, Ty<'db>>,
}
impl<'a, 'db> ty::TypeFolder<DbInterner<'db>> for LowerUniverseFolder<'a, 'db> {
    fn cx(&self) -> DbInterner<'db> {
        self.infcx.interner
    }

    fn fold_ty(&mut self, t: Ty<'db>) -> Ty<'db> {
        if !(t.has_free_regions() || t.has_infer()) {
            return t;
        }

        if let Some(&answer) = self.cache.get(&t) {
            return answer;
        }

        let folded = match t.kind() {
            ty::Infer(ty::TyVar(vid)) => {
                let vid = self.infcx.root_var(vid);
                let probe = self.infcx.inner.borrow_mut().type_variables().probe(vid);
                match probe {
                    TypeVariableValue::Known { value: u } => u.super_fold_with(self),
                    TypeVariableValue::Unknown { universe } => {
                        if self.for_universe.can_name(universe) {
                            t
                        } else {
                            let mut inner = self.infcx.inner.borrow_mut();
                            let origin = inner.type_variables().var_span(vid);
                            let new_var_id =
                                inner.type_variables().new_var(self.for_universe, origin);
                            inner.type_variables().equate(vid, new_var_id);
                            Ty::new_var(self.cx(), new_var_id)
                        }
                    }
                }
            }
            _ => t.super_fold_with(self),
        };

        self.cache.insert(t, folded);
        folded
    }

    fn fold_const(&mut self, c: Const<'db>) -> Const<'db> {
        if !(c.has_free_regions() || c.has_infer()) {
            return c;
        }

        match c.kind() {
            ty::ConstKind::Infer(ty::InferConst::Var(vid)) => {
                let vid = self.infcx.root_const_var(vid);
                let universe = match self.infcx.try_resolve_const_var(vid) {
                    Ok(value) => return value.fold_with(self),
                    Err(universe) => universe,
                };
                if self.for_universe.can_name(universe) {
                    c
                } else {
                    let origin = self.infcx.const_var_span(vid).unwrap();
                    let new_var_id = self
                        .infcx
                        .inner
                        .borrow_mut()
                        .const_unification_table()
                        .new_key(ConstVariableValue::Unknown {
                            span: origin,
                            universe: self.for_universe,
                        })
                        .vid;

                    self.infcx
                        .inner
                        .borrow_mut()
                        .const_unification_table()
                        .union(vid, new_var_id);

                    ty::Const::new_var(self.cx(), new_var_id)
                }
            }
            _ => c.super_fold_with(self),
        }
    }

    fn fold_region(&mut self, r: ty::Region<DbInterner<'db>>) -> ty::Region<DbInterner<'db>> {
        match r.kind() {
            ty::RegionKind::ReBound(..) | ty::RegionKind::ReErased => r,
            _ => {
                let r_universe = self.infcx.universe_of_region(r);
                if self.for_universe.can_name(r_universe) {
                    r
                } else {
                    // FIXME: unfortunately we lose the relating span here unless we take another
                    // argument.
                    let new_region = self
                        .infcx
                        .next_region_var_in_universe(self.for_universe, DUMMY_SP);
                    self.infcx.equate_regions(
                        r,
                        new_region,
                        ty::solve::VisibleForLeakCheck::Yes,
                        DUMMY_SP,
                    );
                    new_region
                }
            }
        }
    }
}
