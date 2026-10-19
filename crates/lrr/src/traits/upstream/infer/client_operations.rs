//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::{DbInterner, InferenceContext};
use rustc_type_ir::{self as ty, InferCtxtLike, Interner, TyVid, inherent::*};
impl<'db> InferenceContext<'db> {
    pub fn opaques_with_sub_unified_hidden_type(
        &self,
        ty_vid: TyVid,
    ) -> Vec<ty::OpaqueAliasTy<DbInterner<'db>>> {
        // Avoid accidentally allowing more code to compile with the old solver.
        if !self.next_trait_solver() {
            return vec![];
        }

        let ty_sub_vid = self.sub_unification_table_root_var(ty_vid);
        let inner = &mut *self.inner.borrow_mut();
        // This is iffy, can't call `type_variables()` as we're already
        // borrowing the `opaque_type_storage` here.
        let mut type_variables = inner.type_variable_storage.with_log(&mut inner.undo_log);
        inner
            .opaque_type_storage
            .iter_opaque_types()
            .filter_map(|(key, hidden_ty)| {
                if let ty::Infer(ty::TyVar(hidden_vid)) = hidden_ty.ty.kind() {
                    let opaque_sub_vid = type_variables.sub_unification_table_root_var(hidden_vid);
                    if opaque_sub_vid == ty_sub_vid {
                        return Some(ty::OpaqueAliasTy::new_opaque_from_args(
                            self.interner,
                            key.def_id.into(),
                            key.args,
                        ));
                    }
                }

                None
            })
            .collect()
    }
    pub fn typing_mode_raw(&self) -> ty::TypingMode<DbInterner<'db>> {
        self.typing_mode
    }
}
