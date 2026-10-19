//! Exhaustive Ty traversal adapted mechanically from pinned rustc_middle.
//! MIT OR Apache-2.0; retained original and licenses: vendor/rustc-trait-solver.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18, ty/structural_impls.rs.
use super::{context::Context, facts::SourceFacts, model::Ty};
use rustc_type_ir::{self as ty, inherent::IntoKind, *};
impl<'cx, S: SourceFacts<'cx>> TypeSuperFoldable<Context<'cx, S>> for Ty<'cx, S> {
    fn try_super_fold_with<F: FallibleTypeFolder<Context<'cx, S>>>(
        self,
        folder: &mut F,
    ) -> Result<Self, F::Error> {
        let kind = match self.kind() {
            ty::RawPtr(ty, mutbl) => ty::RawPtr(ty.try_fold_with(folder)?, mutbl),
            ty::Array(typ, sz) => ty::Array(typ.try_fold_with(folder)?, sz.try_fold_with(folder)?),
            ty::Slice(typ) => ty::Slice(typ.try_fold_with(folder)?),
            ty::Adt(tid, args) => ty::Adt(tid, args.try_fold_with(folder)?),
            ty::Dynamic(trait_ty, region) => ty::Dynamic(
                trait_ty.try_fold_with(folder)?,
                region.try_fold_with(folder)?,
            ),
            ty::Tuple(ts) => ty::Tuple(ts.try_fold_with(folder)?),
            ty::FnDef(def_id, args) => ty::FnDef(def_id, args.try_fold_with(folder)?),
            ty::FnPtr(sig_tys, hdr) => ty::FnPtr(sig_tys.try_fold_with(folder)?, hdr),
            ty::UnsafeBinder(f) => ty::UnsafeBinder(f.try_fold_with(folder)?),
            ty::Ref(r, ty, mutbl) => {
                ty::Ref(r.try_fold_with(folder)?, ty.try_fold_with(folder)?, mutbl)
            }
            ty::Coroutine(did, args) => ty::Coroutine(did, args.try_fold_with(folder)?),
            ty::CoroutineWitness(did, args) => {
                ty::CoroutineWitness(did, args.try_fold_with(folder)?)
            }
            ty::Closure(did, args) => ty::Closure(did, args.try_fold_with(folder)?),
            ty::CoroutineClosure(did, args) => {
                ty::CoroutineClosure(did, args.try_fold_with(folder)?)
            }
            ty::Alias(is_rigid, data) => ty::Alias(is_rigid, data.try_fold_with(folder)?),
            ty::Pat(ty, pat) => ty::Pat(ty.try_fold_with(folder)?, pat.try_fold_with(folder)?),

            ty::Bool
            | ty::Char
            | ty::Str
            | ty::Int(_)
            | ty::Uint(_)
            | ty::Float(_)
            | ty::Error(_)
            | ty::Infer(_)
            | ty::Param(..)
            | ty::Bound(..)
            | ty::Placeholder(..)
            | ty::Never
            | ty::Foreign(..) => return Ok(self),
        };

        Ok(if self.kind() == kind {
            self
        } else {
            folder.cx().ty(kind)
        })
    }

    fn super_fold_with<F: TypeFolder<Context<'cx, S>>>(self, folder: &mut F) -> Self {
        let kind = match self.kind() {
            ty::RawPtr(ty, mutbl) => ty::RawPtr(ty.fold_with(folder), mutbl),
            ty::Array(typ, sz) => ty::Array(typ.fold_with(folder), sz.fold_with(folder)),
            ty::Slice(typ) => ty::Slice(typ.fold_with(folder)),
            ty::Adt(tid, args) => ty::Adt(tid, args.fold_with(folder)),
            ty::Dynamic(trait_ty, region) => {
                ty::Dynamic(trait_ty.fold_with(folder), region.fold_with(folder))
            }
            ty::Tuple(ts) => ty::Tuple(ts.fold_with(folder)),
            ty::FnDef(def_id, args) => ty::FnDef(def_id, args.fold_with(folder)),
            ty::FnPtr(sig_tys, hdr) => ty::FnPtr(sig_tys.fold_with(folder), hdr),
            ty::UnsafeBinder(f) => ty::UnsafeBinder(f.fold_with(folder)),
            ty::Ref(r, ty, mutbl) => ty::Ref(r.fold_with(folder), ty.fold_with(folder), mutbl),
            ty::Coroutine(did, args) => ty::Coroutine(did, args.fold_with(folder)),
            ty::CoroutineWitness(did, args) => ty::CoroutineWitness(did, args.fold_with(folder)),
            ty::Closure(did, args) => ty::Closure(did, args.fold_with(folder)),
            ty::CoroutineClosure(did, args) => ty::CoroutineClosure(did, args.fold_with(folder)),
            ty::Alias(is_rigid, data) => ty::Alias(is_rigid, data.fold_with(folder)),
            ty::Pat(ty, pat) => ty::Pat(ty.fold_with(folder), pat.fold_with(folder)),

            ty::Bool
            | ty::Char
            | ty::Str
            | ty::Int(_)
            | ty::Uint(_)
            | ty::Float(_)
            | ty::Error(_)
            | ty::Infer(_)
            | ty::Param(..)
            | ty::Bound(..)
            | ty::Placeholder(..)
            | ty::Never
            | ty::Foreign(..) => return self,
        };

        if self.kind() == kind {
            self
        } else {
            folder.cx().ty(kind)
        }
    }
}

impl<'cx, S: SourceFacts<'cx>> TypeSuperVisitable<Context<'cx, S>> for Ty<'cx, S> {
    fn super_visit_with<V: TypeVisitor<Context<'cx, S>>>(&self, visitor: &mut V) -> V::Result {
        match self.kind() {
            ty::RawPtr(ty, _mutbl) => ty.visit_with(visitor),
            ty::Array(typ, sz) => {
                try_visit!(typ.visit_with(visitor));
                sz.visit_with(visitor)
            }
            ty::Slice(typ) => typ.visit_with(visitor),
            ty::Adt(_, args) => args.visit_with(visitor),
            ty::Dynamic(trait_ty, reg) => {
                try_visit!(trait_ty.visit_with(visitor));
                reg.visit_with(visitor)
            }
            ty::Tuple(ts) => ts.visit_with(visitor),
            ty::FnDef(_, args) => args.visit_with(visitor),
            ty::FnPtr(sig_tys, _) => sig_tys.visit_with(visitor),
            ty::UnsafeBinder(f) => f.visit_with(visitor),
            ty::Ref(r, ty, _) => {
                try_visit!(r.visit_with(visitor));
                ty.visit_with(visitor)
            }
            ty::Coroutine(_did, args) => args.visit_with(visitor),
            ty::CoroutineWitness(_did, args) => args.visit_with(visitor),
            ty::Closure(_did, args) => args.visit_with(visitor),
            ty::CoroutineClosure(_did, args) => args.visit_with(visitor),
            ty::Alias(_, data) => data.visit_with(visitor),

            ty::Pat(ty, pat) => {
                try_visit!(ty.visit_with(visitor));
                pat.visit_with(visitor)
            }

            ty::Error(guar) => guar.visit_with(visitor),

            ty::Bool
            | ty::Char
            | ty::Str
            | ty::Int(_)
            | ty::Uint(_)
            | ty::Float(_)
            | ty::Infer(_)
            | ty::Bound(..)
            | ty::Placeholder(..)
            | ty::Param(..)
            | ty::Never
            | ty::Foreign(..) => V::Result::output(),
        }
    }
}
