//! Pinned inference variable/resolution client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference_context.py.
use super::{Const, DbInterner, InferenceContext, PreparedProvider, Region, Span, Ty};
use super::{
    resolve, type_variable, type_variable::TypeVariableValue, unify_key::ConstVariableValue,
};
use crate::traits::upstream::{
    binders::BoundVarReplacerDelegate, model::ErrorToken as ErrorGuaranteed,
};
use rustc_type_ir::inherent::{GenericArg as _, Ty as _};
use rustc_type_ir::{
    self as ty, BoundConst, BoundRegion, BoundTy, BoundVariableKind, ConstVid, FloatVarValue,
    FloatVid, InferConst, IntVarValue, IntVid, Interner, RegionVid, TermKind, TyVid, TypeFoldable,
    TypeVisitableExt, UniverseIndex, inherent::*,
};
use tracing::debug;
type GenericArg<'db> = crate::traits::upstream::model::Arg<'db, PreparedProvider<'db>>;
type Binder<'db, T> = rustc_type_ir::Binder<DbInterner<'db>, T>;
type Term<'db> = crate::traits::upstream::model::Term<'db, PreparedProvider<'db>>;
type Clauses<'db> = crate::traits::upstream::model::Clauses<'db, PreparedProvider<'db>>;
type SolverDefId<'db> = crate::traits::upstream::identity::DefinitionId;
#[derive(Clone, Copy, Debug)]
pub enum BoundRegionConversionTime {
    /// when a fn is called
    FnCall,

    /// when two higher-ranked types are compared
    HigherRankedType,

    /// when projecting an associated type
    AssocTypeProjection(crate::traits::upstream::identity::DefinitionId),
}

impl<'db> InferenceContext<'db> {
    pub fn num_ty_vars(&self) -> usize {
        self.inner.borrow_mut().type_variables().num_vars()
    }
    pub fn next_ty_var(&self, span: Span) -> Ty<'db> {
        let vid = self.next_ty_vid(span);
        Ty::new_var(self.interner, vid)
    }
    pub fn next_ty_vid(&self, span: Span) -> TyVid {
        self.next_ty_var_id_in_universe(self.universe(), span)
    }
    pub fn next_ty_var_id_in_universe(&self, universe: UniverseIndex, span: Span) -> TyVid {
        self.inner
            .borrow_mut()
            .type_variables()
            .new_var(universe, span)
    }
    pub fn next_ty_var_in_universe(&self, universe: UniverseIndex, span: Span) -> Ty<'db> {
        let vid = self.next_ty_var_id_in_universe(universe, span);
        Ty::new_var(self.interner, vid)
    }
    pub fn next_const_var(&self, span: Span) -> Const<'db> {
        let vid = self.next_const_vid(span);
        Const::new_var(self.interner, vid)
    }
    pub fn next_const_vid(&self, span: Span) -> ConstVid {
        self.next_const_vid_in_universe(self.universe(), span)
    }
    pub fn next_const_vid_in_universe(&self, universe: UniverseIndex, span: Span) -> ConstVid {
        self.inner
            .borrow_mut()
            .const_unification_table()
            .new_key(ConstVariableValue::Unknown { span, universe })
            .vid
    }
    pub fn next_const_var_in_universe(&self, universe: UniverseIndex, span: Span) -> Const<'db> {
        let vid = self.next_const_vid_in_universe(universe, span);
        Const::new_var(self.interner, vid)
    }
    pub fn next_int_var(&self) -> Ty<'db> {
        let vid = self.next_int_vid();
        Ty::new_infer(self.interner, ty::InferTy::IntVar(vid))
    }
    pub fn next_int_vid(&self) -> IntVid {
        self.inner
            .borrow_mut()
            .int_unification_table()
            .new_key(IntVarValue::Unknown)
    }
    pub fn next_float_var(&self) -> Ty<'db> {
        Ty::new_infer(self.interner, ty::InferTy::FloatVar(self.next_float_vid()))
    }
    pub fn next_float_vid(&self) -> FloatVid {
        self.inner
            .borrow_mut()
            .float_unification_table()
            .new_key(FloatVarValue::Unknown)
    }
    pub fn next_region_var(&self, span: Span) -> Region<'db> {
        self.next_region_var_in_universe(self.universe(), span)
    }
    pub fn next_region_vid(&self, span: Span) -> RegionVid {
        self.inner
            .borrow_mut()
            .unwrap_region_constraints()
            .new_region_var(self.universe(), span)
    }
    pub fn next_region_var_in_universe(&self, universe: UniverseIndex, span: Span) -> Region<'db> {
        let region_var = self
            .inner
            .borrow_mut()
            .unwrap_region_constraints()
            .new_region_var(universe, span);
        Region::new_var(self.interner, region_var)
    }
    pub fn next_term_var_of_kind(&self, term: Term<'db>, span: Span) -> Term<'db> {
        match term.kind() {
            TermKind::Ty(_) => self.next_ty_var(span).into(),
            TermKind::Const(_) => self.next_const_var(span).into(),
        }
    }
    pub fn universe_of_region(&self, r: Region<'db>) -> UniverseIndex {
        self.inner
            .borrow_mut()
            .unwrap_region_constraints()
            .universe(r)
    }
    pub fn num_region_vars(&self) -> usize {
        self.inner
            .borrow_mut()
            .unwrap_region_constraints()
            .num_region_vars()
    }
    pub fn tainted_by_errors(&self) -> Option<ErrorGuaranteed> {
        self.tainted_by_errors.get()
    }
    pub fn set_tainted_by_errors(&self, e: ErrorGuaranteed) {
        debug!("set_tainted_by_errors(ErrorGuaranteed)");
        self.tainted_by_errors.set(Some(e));
    }
    pub fn type_var_span(&self, vid: TyVid) -> Span {
        self.inner.borrow_mut().type_variables().var_span(vid)
    }
    pub fn const_var_span(&self, vid: ConstVid) -> Option<Span> {
        match self
            .inner
            .borrow_mut()
            .const_unification_table()
            .probe_value(vid)
        {
            ConstVariableValue::Known { .. } => None,
            ConstVariableValue::Unknown { span, .. } => Some(span),
        }
    }
    pub fn instantiate_binder_with_fresh_vars<T>(
        &self,
        span: Span,
        _lbrct: BoundRegionConversionTime,
        value: Binder<'db, T>,
    ) -> T
    where
        T: TypeFoldable<DbInterner<'db>> + Clone,
    {
        if let Some(inner) = value.clone().no_bound_vars() {
            return inner;
        }

        let bound_vars = value.clone().bound_vars();
        let mut args = Vec::with_capacity(bound_vars.len());

        for bound_var_kind in bound_vars {
            let arg: GenericArg<'db> = match bound_var_kind {
                BoundVariableKind::Ty(..) => self.next_ty_var(span).into(),
                BoundVariableKind::Region(_) => self.next_region_var(span).into(),
                BoundVariableKind::Const => self.next_const_var(span).into(),
            };
            args.push(arg);
        }

        struct ToFreshVars<'db> {
            args: Vec<GenericArg<'db>>,
        }

        impl<'db> BoundVarReplacerDelegate<'db, PreparedProvider<'db>> for ToFreshVars<'db> {
            fn replace_region(&mut self, br: BoundRegion<DbInterner<'db>>) -> Region<'db> {
                self.args[br.var.index()].expect_region()
            }
            fn replace_ty(&mut self, bt: BoundTy<DbInterner<'db>>) -> Ty<'db> {
                self.args[bt.var.index()].expect_ty()
            }
            fn replace_const(&mut self, bv: BoundConst<DbInterner<'db>>) -> Const<'db> {
                self.args[bv.var.index()].expect_const()
            }
        }
        let delegate = ToFreshVars { args };
        self.interner.replace_bound_vars_uncached(value, delegate)
    }
    pub fn try_resolve_ty_var(&self, vid: TyVid) -> Result<Ty<'db>, ty::UniverseIndex> {
        use self::type_variable::TypeVariableValue;

        match self.inner.borrow_mut().type_variables().probe(vid) {
            TypeVariableValue::Known { value } => Ok(value),
            TypeVariableValue::Unknown { universe } => Err(universe),
        }
    }
    pub fn shallow_resolve_ty_var_or_get_root(&self, vid: TyVid) -> Result<Ty<'db>, TyVid> {
        let (root, value) = self
            .inner
            .borrow_mut()
            .type_variables()
            .probe_with_root_vid(vid);

        match value {
            TypeVariableValue::Known { value } => Ok(value),
            TypeVariableValue::Unknown { universe: _ } => Err(root),
        }
    }
    pub fn shallow_resolve(&self, ty: Ty<'db>) -> Ty<'db> {
        if let ty::Infer(v) = ty.kind() {
            match v {
                ty::TyVar(v) => {
                    // Not entirely obvious: if `typ` is a type variable,
                    // it can be resolved to an int/float variable, which
                    // can then be recursively resolved, hence the
                    // recursion. Note though that we prevent type
                    // variables from unifying to other type variables
                    // directly (though they may be embedded
                    // structurally), and we prevent cycles in any case,
                    // so this recursion should always be of very limited
                    // depth.
                    //
                    // Note: if these two lines are combined into one we get
                    // dynamic borrow errors on `self.inner`.
                    let (root_vid, value) = self
                        .inner
                        .borrow_mut()
                        .type_variables()
                        .probe_with_root_vid(v);
                    value.known().map_or_else(
                        || {
                            if root_vid == v {
                                ty
                            } else {
                                Ty::new_var(self.interner, root_vid)
                            }
                        },
                        |t| self.shallow_resolve(t),
                    )
                }

                ty::IntVar(v) => {
                    let (root, value) = self
                        .inner
                        .borrow_mut()
                        .int_unification_table()
                        .inlined_probe_key_value(v);
                    match value {
                        ty::IntVarValue::IntType(ty) => self.interner.ty(ty::TyKind::Int(ty)),
                        ty::IntVarValue::UintType(ty) => self.interner.ty(ty::TyKind::Uint(ty)),
                        ty::IntVarValue::Unknown => {
                            if root == v {
                                ty
                            } else {
                                Ty::new_infer(self.interner, ty::InferTy::IntVar(root))
                            }
                        }
                    }
                }

                ty::FloatVar(v) => {
                    let (root, value) = self
                        .inner
                        .borrow_mut()
                        .float_unification_table()
                        .inlined_probe_key_value(v);
                    match value {
                        ty::FloatVarValue::Known(ty) => self.interner.ty(ty::TyKind::Float(ty)),
                        ty::FloatVarValue::Unknown => {
                            if root == v {
                                ty
                            } else {
                                Ty::new_infer(self.interner, ty::InferTy::FloatVar(root))
                            }
                        }
                    }
                }

                ty::FreshTy(_) | ty::FreshIntTy(_) | ty::FreshFloatTy(_) => ty,
            }
        } else {
            ty
        }
    }
    pub fn shallow_resolve_const(&self, ct: Const<'db>) -> Const<'db> {
        match ct.kind() {
            ty::ConstKind::Infer(infer_ct) => match infer_ct {
                InferConst::Var(vid) => {
                    let (root, value) = self
                        .inner
                        .borrow_mut()
                        .const_unification_table()
                        .inlined_probe_key_value(vid);
                    value.known().unwrap_or_else(|| {
                        if root.vid == vid {
                            ct
                        } else {
                            ty::Const::new_var(self.interner, root.vid)
                        }
                    })
                }
                InferConst::Fresh(_) => ct,
            },

            ty::ConstKind::Param(_)
            | ty::ConstKind::Bound(_, _)
            | ty::ConstKind::Placeholder(_)
            | ty::ConstKind::Alias(_, _)
            | ty::ConstKind::Value(_)
            | ty::ConstKind::Error(_)
            | ty::ConstKind::Expr(_) => ct,
        }
    }
    pub fn shallow_resolve_term(&self, term: Term<'db>) -> Term<'db> {
        match term.kind() {
            ty::TermKind::Ty(ty) => self.shallow_resolve(ty).into(),
            ty::TermKind::Const(ct) => self.shallow_resolve_const(ct).into(),
        }
    }
    pub fn root_var(&self, var: ty::TyVid) -> ty::TyVid {
        self.inner.borrow_mut().type_variables().root_var(var)
    }
    pub fn sub_unify_ty_vids_raw(&self, a: ty::TyVid, b: ty::TyVid) {
        self.inner.borrow_mut().type_variables().sub_unify(a, b);
    }
    pub fn sub_unification_table_root_var(&self, var: ty::TyVid) -> ty::TyVid {
        self.inner
            .borrow_mut()
            .type_variables()
            .sub_unification_table_root_var(var)
    }
    pub fn root_float_var(&self, var: ty::FloatVid) -> ty::FloatVid {
        self.inner.borrow_mut().float_unification_table().find(var)
    }
    pub fn root_const_var(&self, var: ty::ConstVid) -> ty::ConstVid {
        self.inner
            .borrow_mut()
            .const_unification_table()
            .find(var)
            .vid
    }
    pub fn shallow_resolve_const_var(&self, vid: ty::ConstVid) -> Const<'db> {
        match self.try_resolve_const_var(vid) {
            Ok(ct) => ct,
            Err(_) => ty::Const::new_var(self.interner, self.root_const_var(vid)),
        }
    }
    pub fn shallow_resolve_ty_var(&self, vid: ty::TyVid) -> Ty<'db> {
        match self.try_resolve_ty_var(vid) {
            Ok(ty) => ty,
            Err(_) => Ty::new_var(self.interner, self.root_var(vid)),
        }
    }
    pub fn shallow_resolve_int_var(&self, vid: ty::IntVid) -> Ty<'db> {
        let mut inner = self.inner.borrow_mut();
        let value = inner.int_unification_table().probe_value(vid);
        match value {
            ty::IntVarValue::IntType(ty) => self.interner.ty(ty::TyKind::Int(ty)),
            ty::IntVarValue::UintType(ty) => self.interner.ty(ty::TyKind::Uint(ty)),
            ty::IntVarValue::Unknown => Ty::new_infer(
                self.interner,
                ty::InferTy::IntVar(inner.int_unification_table().find(vid)),
            ),
        }
    }
    pub fn shallow_resolve_float_var(&self, vid: ty::FloatVid) -> Ty<'db> {
        let mut inner = self.inner.borrow_mut();
        let value = inner.float_unification_table().probe_value(vid);
        match value {
            ty::FloatVarValue::Known(ty) => self.interner.ty(ty::TyKind::Float(ty)),
            ty::FloatVarValue::Unknown => Ty::new_infer(
                self.interner,
                ty::InferTy::FloatVar(inner.float_unification_table().find(vid)),
            ),
        }
    }
    pub fn deeply_resolve_ignoring_regions<T>(&self, value: T) -> T
    where
        T: TypeFoldable<DbInterner<'db>>,
    {
        if let Err(guar) = value.error_reported() {
            self.set_tainted_by_errors(guar);
        }
        if !value.has_non_region_infer() {
            return value;
        }
        let mut r = resolve::DeepResolverIgnoringRegions::new(self);
        value.fold_with(&mut r)
    }
    pub fn try_resolve_const_var(
        &self,
        vid: ty::ConstVid,
    ) -> Result<Const<'db>, ty::UniverseIndex> {
        match self
            .inner
            .borrow_mut()
            .const_unification_table()
            .probe_value(vid)
        {
            ConstVariableValue::Known { value } => Ok(value),
            ConstVariableValue::Unknown { span: _, universe } => Err(universe),
        }
    }
}
