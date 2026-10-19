//! Pinned compiler relevant-impl index traversal; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; original/licenses in vendor/rustc-trait-solver.
//! Mechanical adapter: tools/sync_upstream_candidates.py; no solver matching here.
use super::{
    facts::SourceFacts,
    identity::DefinitionId,
    model::Ty,
    prepared::{Cx, PreparedProvider},
};
use rustc_type_ir::{self as ty, VisitorResult, inherent::IntoKind, try_visit};
impl<'cx> PreparedProvider<'cx> {
    pub(super) fn for_each_relevant_source_impl<R: VisitorResult>(
        self,
        cx: Cx<'cx>,
        trait_def_id: DefinitionId,
        self_ty: Ty<'cx, PreparedProvider<'cx>>,
        mut f: impl FnMut(DefinitionId) -> R,
    ) -> R {
        let tcx = cx;
        let trait_impls = self.prepared_impls(trait_def_id);
        let mut consider_impls_for_simplified_type = |simp| {
            if let Some(impls_for_type) = trait_impls.non_blanket_impls().get(&simp) {
                for &impl_def_id in impls_for_type {
                    try_visit!(f(impl_def_id))
                }
            }

            R::output()
        };

        match self_ty.kind() {
            ty::Bool
            | ty::Char
            | ty::Int(_)
            | ty::Uint(_)
            | ty::Float(_)
            | ty::Adt(_, _)
            | ty::Foreign(_)
            | ty::Str
            | ty::Array(_, _)
            | ty::Slice(_)
            | ty::RawPtr(_, _)
            | ty::Ref(_, _, _)
            | ty::FnDef(_, _)
            | ty::FnPtr(..)
            | ty::Dynamic(_, _)
            | ty::Closure(..)
            | ty::CoroutineClosure(..)
            | ty::Coroutine(_, _)
            | ty::Never
            | ty::Tuple(_)
            | ty::UnsafeBinder(_) => {
                let simp = ty::fast_reject::simplify_type(
                    tcx,
                    self_ty,
                    ty::fast_reject::TreatParams::AsRigid,
                )
                .unwrap();
                try_visit!(consider_impls_for_simplified_type(simp));
            }

            // HACK: For integer and float variables we have to manually look at all impls
            // which have some integer or float as a self type.
            ty::Infer(ty::IntVar(_)) => {
                use ty::IntTy::*;
                use ty::UintTy::*;
                // This causes a compiler error if any new integer kinds are added.
                let (I8 | I16 | I32 | I64 | I128 | Isize): ty::IntTy;
                let (U8 | U16 | U32 | U64 | U128 | Usize): ty::UintTy;
                let possible_integers = [
                    // signed integers
                    ty::fast_reject::SimplifiedType::Int(I8),
                    ty::fast_reject::SimplifiedType::Int(I16),
                    ty::fast_reject::SimplifiedType::Int(I32),
                    ty::fast_reject::SimplifiedType::Int(I64),
                    ty::fast_reject::SimplifiedType::Int(I128),
                    ty::fast_reject::SimplifiedType::Int(Isize),
                    // unsigned integers
                    ty::fast_reject::SimplifiedType::Uint(U8),
                    ty::fast_reject::SimplifiedType::Uint(U16),
                    ty::fast_reject::SimplifiedType::Uint(U32),
                    ty::fast_reject::SimplifiedType::Uint(U64),
                    ty::fast_reject::SimplifiedType::Uint(U128),
                    ty::fast_reject::SimplifiedType::Uint(Usize),
                ];
                for simp in possible_integers {
                    try_visit!(consider_impls_for_simplified_type(simp));
                }
            }

            ty::Infer(ty::FloatVar(_)) => {
                // This causes a compiler error if any new float kinds are added.
                let (ty::FloatTy::F16 | ty::FloatTy::F32 | ty::FloatTy::F64 | ty::FloatTy::F128);
                let possible_floats = [
                    ty::fast_reject::SimplifiedType::Float(ty::FloatTy::F16),
                    ty::fast_reject::SimplifiedType::Float(ty::FloatTy::F32),
                    ty::fast_reject::SimplifiedType::Float(ty::FloatTy::F64),
                    ty::fast_reject::SimplifiedType::Float(ty::FloatTy::F128),
                ];

                for simp in possible_floats {
                    try_visit!(consider_impls_for_simplified_type(simp));
                }
            }

            // Pattern type might not have a simplified type.
            ty::Pat(_, _) => {
                if let Some(simp) = ty::fast_reject::simplify_type(
                    tcx,
                    self_ty,
                    ty::fast_reject::TreatParams::AsRigid,
                ) {
                    try_visit!(consider_impls_for_simplified_type(simp));
                }
            }

            // This is only for diagnostics and normally ty vars should be handled by the callers.
            ty::Infer(ty::TyVar(_)) => {
                for &impl_def_id in trait_impls.non_blanket_impls().values().flatten() {
                    try_visit!(f(impl_def_id));
                }
            }

            // The only traits applying to aliases and placeholders are blanket impls.
            //
            // Impls which apply to an alias after normalization are handled by
            // `assemble_candidates_after_normalizing_self_ty`.
            ty::Alias(ty::IsRigid::Yes, _) | ty::Placeholder(..) | ty::Error(_) => (),
            // FIXME(-Znext-solver=no): Need to support aliases not marked as
            // rigid for the old solver.
            ty::Alias(ty::IsRigid::No, _) => (),

            // FIXME: These should ideally not exist as a self type. It would be nice for
            // the builtin auto trait impls of coroutines to instead directly recurse
            // into the witness.
            ty::CoroutineWitness(..) => (),

            // These are used in diagnostics or in the old solver.
            ty::Param(_) | ty::Bound(_, _) => (),

            // These variants should not exist as a self type.
            ty::Infer(ty::FreshTy(_) | ty::FreshIntTy(_) | ty::FreshFloatTy(_)) => {
                panic!("unexpected self type: {self_ty:?}");
            }
        }

        SourceFacts::for_each_blanket_impl(self, cx, trait_def_id, f)
    }
}
