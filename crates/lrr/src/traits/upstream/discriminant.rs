//! Complete pinned compiler discriminant-Type client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; original/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/upstream_discriminant.py.
use super::{context::Context, facts::SourceFacts, model::Ty};
use rustc_type_ir::inherent::Ty as _;
use rustc_type_ir::lang_items::SolverTraitLangItem;
use rustc_type_ir::{self as ty, InferTy::*, Interner, inherent::*};
trait CoroutineDiscriminant<'cx, F: SourceFacts<'cx>> {
    fn discr_ty(&self, cx: Context<'cx, F>) -> Ty<'cx, F>;
}
impl<'cx, F: SourceFacts<'cx>> CoroutineDiscriminant<'cx, F>
    for ty::CoroutineArgs<Context<'cx, F>>
{
    fn discr_ty(&self, tcx: Context<'cx, F>) -> Ty<'cx, F> {
        tcx.ty(ty::TyKind::Uint(ty::UintTy::U32))
    }
}
impl<'cx, F: SourceFacts<'cx>> Ty<'cx, F> {
    pub(super) fn pinned_discriminant_ty(self, tcx: Context<'cx, F>) -> Ty<'cx, F> {
        match self.kind() {
            ty::TyKind::Adt(adt, _) if adt.is_enum() => adt.enum_discriminant_type(tcx),
            ty::TyKind::Coroutine(_, args) => args.as_coroutine().discr_ty(tcx),

            ty::TyKind::Param(_) | ty::TyKind::Alias(..) | ty::TyKind::Infer(ty::TyVar(_)) => {
                let assoc_items = tcx.associated_type_def_ids(
                    tcx.require_trait_lang_item(SolverTraitLangItem::DiscriminantKind),
                );
                Ty::new_projection_from_args(
                    tcx,
                    ty::IsRigid::No,
                    assoc_items
                        .into_iter()
                        .next()
                        .expect("prepared DiscriminantKind has its associated type"),
                    tcx.mk_args(&[self.into()]),
                )
            }

            ty::TyKind::Pat(ty, _) => ty.pinned_discriminant_ty(tcx),
            ty::TyKind::UnsafeBinder(bound_ty) => tcx
                .instantiate_bound_regions_with_erased(bound_ty.into())
                .pinned_discriminant_ty(tcx),

            ty::TyKind::Bool
            | ty::TyKind::Char
            | ty::TyKind::Int(_)
            | ty::TyKind::Uint(_)
            | ty::TyKind::Float(_)
            | ty::TyKind::Adt(..)
            | ty::TyKind::Foreign(_)
            | ty::TyKind::Str
            | ty::TyKind::Array(..)
            | ty::TyKind::Slice(_)
            | ty::TyKind::RawPtr(_, _)
            | ty::TyKind::Ref(..)
            | ty::TyKind::FnDef(..)
            | ty::TyKind::FnPtr(..)
            | ty::TyKind::Dynamic(..)
            | ty::TyKind::Closure(..)
            | ty::TyKind::CoroutineClosure(..)
            | ty::TyKind::CoroutineWitness(..)
            | ty::TyKind::Never
            | ty::TyKind::Tuple(_)
            | ty::TyKind::Error(_)
            | ty::TyKind::Infer(IntVar(_) | FloatVar(_)) => Ty::new_u8(tcx),

            ty::TyKind::Bound(..)
            | ty::TyKind::Placeholder(_)
            | ty::TyKind::Infer(FreshTy(_) | ty::FreshIntTy(_) | ty::FreshFloatTy(_)) => {
                panic!("`discriminant_ty` applied to unexpected type: {:?}", self)
            }
        }
    }
}
