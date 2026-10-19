//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::opaque_types::{OpaqueTypeStorageEntries, ProvisionalHiddenType};
use super::snapshot::undo_log::UndoLog;
use super::{
    DbInterner, InferenceContext, OpaqueTypeKey, Region, Span, Ty, TypeOutlivesConstraint,
};
use super::{type_variable::TypeVariableValue, unify_key::ConstVariableValue};
use ena::undo_log::UndoLogs;
use rustc_type_ir::{self as ty, Interner, TypeVisitableExt, solve::TyOrConstInferVar};
use tracing::debug;
impl<'db> InferenceContext<'db> {
    pub fn take_opaque_types(&self) -> Vec<(OpaqueTypeKey<'db>, ProvisionalHiddenType<'db>)> {
        self.inner
            .borrow_mut()
            .opaque_type_storage
            .take_opaque_types()
            .collect()
    }
    pub fn clone_opaque_types(&self) -> Vec<(OpaqueTypeKey<'db>, ProvisionalHiddenType<'db>)> {
        self.inner
            .borrow_mut()
            .opaque_type_storage
            .iter_opaque_types()
            .collect()
    }
    pub fn register_hidden_type_in_storage(
        &self,
        opaque_type_key: OpaqueTypeKey<'db>,
        hidden_ty: ProvisionalHiddenType<'db>,
    ) -> Option<Ty<'db>> {
        self.inner
            .borrow_mut()
            .opaque_types()
            .register(opaque_type_key, hidden_ty)
    }
    pub fn opaque_types_storage_num_entries(&self) -> OpaqueTypeStorageEntries {
        self.inner.borrow_mut().opaque_types().num_entries()
    }
    pub fn clone_opaque_types_lookup_table(&self) -> Vec<(OpaqueTypeKey<'db>, Ty<'db>)> {
        self.inner
            .borrow_mut()
            .opaque_types()
            .iter_lookup_table()
            .map(|(k, h)| (k, h.ty))
            .collect()
    }
    pub fn clone_duplicate_opaque_types(&self) -> Vec<(OpaqueTypeKey<'db>, Ty<'db>)> {
        self.inner
            .borrow_mut()
            .opaque_types()
            .iter_duplicate_entries()
            .map(|(k, h)| (k, h.ty))
            .collect()
    }
    pub fn clone_opaque_types_added_since(
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
    pub fn add_duplicate_opaque_type(
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
    pub fn reset_opaque_types(&self) {
        let _ = self.take_opaque_types();
    }
    pub fn register_type_outlives_constraint_inner(&self, obligation: TypeOutlivesConstraint<'db>) {
        let mut inner = self.inner.borrow_mut();
        inner.undo_log.push(UndoLog::PushTypeOutlivesConstraint);
        inner.region_obligations.push(obligation);
    }
    pub fn register_type_outlives_constraint(
        &self,
        sup_type: Ty<'db>,
        sub_region: Region<'db>,
        span: Span,
    ) {
        assert!(!self.interner.assumptions_on_binders());

        // `is_global` means the type has no params, infer, placeholder, or non-`'static`
        // free regions. If the type has none of these things, then we can skip registering
        // this outlives obligation since it has no components which affect lifetime
        // checking in an interesting way.
        if sup_type.is_global() {
            return;
        }

        debug!(?sup_type, ?sub_region, ?span);
        let origin = span;

        self.register_type_outlives_constraint_inner(TypeOutlivesConstraint {
            sup_type,
            sub_region,
            origin,
        });
    }
    #[inline(always)]
    pub fn ty_or_const_infer_var_changed(&self, var: TyOrConstInferVar) -> bool {
        match var {
            TyOrConstInferVar::Ty(vid) => !matches!(
                self.inner.borrow().try_type_variables_probe_ref(vid),
                Some(TypeVariableValue::Unknown { .. })
            ),
            TyOrConstInferVar::TyInt(vid) => !matches!(
                self.inner
                    .borrow()
                    .int_unification_storage
                    .try_probe_value(vid),
                Some(ty::IntVarValue::Unknown)
            ),
            TyOrConstInferVar::TyFloat(vid) => !matches!(
                self.inner
                    .borrow()
                    .float_unification_storage
                    .try_probe_value(vid),
                Some(ty::FloatVarValue::Unknown)
            ),
            TyOrConstInferVar::Const(vid) => !matches!(
                self.inner
                    .borrow()
                    .const_unification_storage
                    .try_probe_value(vid),
                Some(ConstVariableValue::Unknown { .. })
            ),
        }
    }
}
