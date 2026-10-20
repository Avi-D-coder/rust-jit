//! Required shared-IR constructors and positional encodings. These construct
//! metadata only; inference remains the supplied upstream TypeRelation/delegate.
use super::{
    context::Context, facts::SourceFacts, identity::DefinitionId, model::*,
    parameters::SourceParameter,
};
use rustc_type_ir::{self as ty, inherent::*, *};

impl<'cx, F: SourceFacts<'cx>> ty::inherent::Ty<Context<'cx, F>> for Ty<'cx, F> {
    fn new_unit(cx: Context<'cx, F>) -> Self {
        cx.ty(TyKind::Tuple(List::default()))
    }
    fn new_bool(cx: Context<'cx, F>) -> Self {
        cx.ty(TyKind::Bool)
    }
    fn new_u8(cx: Context<'cx, F>) -> Self {
        cx.ty(TyKind::Uint(UintTy::U8))
    }
    fn new_usize(cx: Context<'cx, F>) -> Self {
        cx.ty(TyKind::Uint(UintTy::Usize))
    }
    fn new_infer(cx: Context<'cx, F>, v: InferTy) -> Self {
        cx.ty(TyKind::Infer(v))
    }
    fn new_var(cx: Context<'cx, F>, v: TyVid) -> Self {
        cx.ty(TyKind::Infer(InferTy::TyVar(v)))
    }
    fn new_param(cx: Context<'cx, F>, p: SourceParameter) -> Self {
        cx.ty(TyKind::Param(p))
    }
    fn new_placeholder(cx: Context<'cx, F>, p: PlaceholderType<Context<'cx, F>>) -> Self {
        cx.ty(TyKind::Placeholder(p))
    }
    fn new_bound(cx: Context<'cx, F>, d: DebruijnIndex, v: BoundTy<Context<'cx, F>>) -> Self {
        cx.ty(TyKind::Bound(BoundVarIndexKind::Bound(d), v))
    }
    fn new_anon_bound(cx: Context<'cx, F>, d: DebruijnIndex, v: BoundVar) -> Self {
        Self::new_bound(
            cx,
            d,
            BoundTy {
                var: v,
                kind: BoundTyKind::Anon,
            },
        )
    }
    fn new_canonical_bound(cx: Context<'cx, F>, v: BoundVar) -> Self {
        cx.ty(TyKind::Bound(
            BoundVarIndexKind::Canonical,
            BoundTy {
                var: v,
                kind: BoundTyKind::Anon,
            },
        ))
    }
    fn new_alias(cx: Context<'cx, F>, rigid: IsRigid, alias: AliasTy<Context<'cx, F>>) -> Self {
        cx.ty(TyKind::Alias(rigid, alias))
    }
    fn new_error(cx: Context<'cx, F>, e: ErrorToken) -> Self {
        cx.ty(TyKind::Error(e))
    }
    fn new_adt(cx: Context<'cx, F>, adt: AdtDefinition<'cx, F>, args: Args<'cx, F>) -> Self {
        cx.ty(TyKind::Adt(adt, args))
    }
    fn new_foreign(cx: Context<'cx, F>, d: DefinitionId) -> Self {
        cx.ty(TyKind::Foreign(d))
    }
    fn new_dynamic(
        cx: Context<'cx, F>,
        predicates: Existentials<'cx, F>,
        r: Region<Context<'cx, F>>,
    ) -> Self {
        cx.ty(TyKind::Dynamic(predicates, r))
    }
    fn new_coroutine(cx: Context<'cx, F>, d: DefinitionId, args: Args<'cx, F>) -> Self {
        cx.ty(TyKind::Coroutine(d, args))
    }
    fn new_coroutine_closure(cx: Context<'cx, F>, d: DefinitionId, args: Args<'cx, F>) -> Self {
        cx.ty(TyKind::CoroutineClosure(d, args))
    }
    fn new_closure(cx: Context<'cx, F>, d: DefinitionId, args: Args<'cx, F>) -> Self {
        cx.ty(TyKind::Closure(d, args))
    }
    fn new_coroutine_witness(cx: Context<'cx, F>, d: DefinitionId, args: Args<'cx, F>) -> Self {
        cx.ty(TyKind::CoroutineWitness(d, args))
    }
    fn new_coroutine_witness_for_coroutine(
        cx: Context<'cx, F>,
        d: DefinitionId,
        args: Args<'cx, F>,
    ) -> Self {
        let erased = cx.mk_args_from_iter(args.iter().map(|arg| match arg {
            Arg::Region(_) => Arg::Region(cx.intern_region(RegionKind::ReErased)),
            _ => arg,
        }));
        Self::new_coroutine_witness(cx, d, erased)
    }
    fn new_ptr(cx: Context<'cx, F>, t: Self, m: Mutability) -> Self {
        cx.ty(TyKind::RawPtr(t, m))
    }
    fn new_ref(cx: Context<'cx, F>, r: Region<Context<'cx, F>>, t: Self, m: Mutability) -> Self {
        cx.ty(TyKind::Ref(r, t, m))
    }
    fn new_array_with_const_len(cx: Context<'cx, F>, t: Self, n: Const<Context<'cx, F>>) -> Self {
        cx.ty(TyKind::Array(t, n))
    }
    fn new_slice(cx: Context<'cx, F>, t: Self) -> Self {
        cx.ty(TyKind::Slice(t))
    }
    fn new_tup(cx: Context<'cx, F>, fields: &[Self]) -> Self {
        cx.ty(TyKind::Tuple(cx.list(fields.to_vec())))
    }
    fn new_tup_from_iter<I, T>(cx: Context<'cx, F>, iter: I) -> T::Output
    where
        I: Iterator<Item = T>,
        T: CollectAndApply<Self, Self>,
    {
        T::collect_and_apply(iter, |fields| Self::new_tup(cx, fields))
    }
    fn new_fn_def(
        cx: Context<'cx, F>,
        d: DefinitionId,
        args: Binder<Context<'cx, F>, Args<'cx, F>>,
    ) -> Self {
        cx.ty(TyKind::FnDef(d, args))
    }
    fn new_fn_ptr(
        cx: Context<'cx, F>,
        sig: Binder<Context<'cx, F>, FnSig<Context<'cx, F>>>,
    ) -> Self {
        let (types, kind) = sig.split();
        cx.ty(TyKind::FnPtr(types, kind))
    }
    fn new_pat(cx: Context<'cx, F>, t: Self, p: Pattern<'cx, F>) -> Self {
        cx.ty(TyKind::Pat(t, p))
    }
    fn new_unsafe_binder(cx: Context<'cx, F>, t: Binder<Context<'cx, F>, Self>) -> Self {
        cx.ty(TyKind::UnsafeBinder(t.into()))
    }
    fn tuple_fields(self) -> Types<'cx, F> {
        match self.kind() {
            TyKind::Tuple(fields) => fields,
            _ => panic!("upstream tuple_fields requires a prepared tuple"),
        }
    }
    fn to_opt_closure_kind(self) -> Option<ClosureKind> {
        match self.kind() {
            TyKind::Int(IntTy::I8) => Some(ClosureKind::Fn),
            TyKind::Int(IntTy::I16) => Some(ClosureKind::FnMut),
            TyKind::Int(IntTy::I32) => Some(ClosureKind::FnOnce),
            TyKind::Bound(..) | TyKind::Placeholder(_) | TyKind::Param(_) | TyKind::Infer(_) => {
                None
            }
            TyKind::Error(_) => Some(ClosureKind::Fn),
            _ => panic!("invalid upstream closure-kind encoding"),
        }
    }
    fn from_closure_kind(cx: Context<'cx, F>, k: ClosureKind) -> Self {
        cx.ty(TyKind::Int(match k {
            ClosureKind::Fn => IntTy::I8,
            ClosureKind::FnMut => IntTy::I16,
            ClosureKind::FnOnce => IntTy::I32,
        }))
    }
    fn from_coroutine_closure_kind(cx: Context<'cx, F>, k: ClosureKind) -> Self {
        Self::from_closure_kind(
            cx,
            match k {
                ClosureKind::Fn => ClosureKind::FnMut,
                k => k,
            },
        )
    }
    fn has_unsafe_fields(self) -> bool {
        match self.kind() {
            TyKind::Adt(adt, _) => {
                adt.context
                    .services()
                    .adt_info(adt.context, adt.definition)
                    .unsafe_fields.copied("ADT unsafe fields")
            }
            _ => false,
        }
    }
    fn discriminant_ty(self, cx: Context<'cx, F>) -> Self {
        cx.services().discriminant_type(cx, self)
    }
}
impl<'cx, F: SourceFacts<'cx>> Tys<Context<'cx, F>> for Types<'cx, F> {
    fn inputs(self) -> &'cx [Ty<'cx, F>] {
        let values = self.items();
        &values[..values
            .len()
            .checked_sub(1)
            .expect("function signature must include a result")]
    }
    fn output(self) -> Ty<'cx, F> {
        *self
            .items()
            .last()
            .expect("function signature must include a result")
    }
}
impl<'cx, F: SourceFacts<'cx>> GenericArgs<Context<'cx, F>> for Args<'cx, F> {
    fn rebase_onto(self, cx: Context<'cx, F>, source: DefinitionId, target: Self) -> Self {
        let count = cx.generics_of(source).count();
        cx.mk_args_from_iter(target.iter().chain(self.iter().skip(count)))
    }
    fn type_at(self, i: usize) -> Ty<'cx, F> {
        self.items()[i].expect_ty()
    }
    fn region_at(self, i: usize) -> Region<Context<'cx, F>> {
        self.items()[i]
            .as_region()
            .expect("prepared argument is not a region")
    }
    fn const_at(self, i: usize) -> Const<Context<'cx, F>> {
        self.items()[i].expect_const()
    }
    fn identity_for_item(cx: Context<'cx, F>, definition: DefinitionId) -> Self {
        Self::for_item(cx, definition, |p, _| match p {
            GenericParameter::Type(p) => Arg::Type(Ty::new_param(cx, p)),
            GenericParameter::Region(p) => Arg::Region(cx.intern_region(RegionKind::ReEarlyParam(p))),
            GenericParameter::Const(p, _) => Arg::Const(Const::new(cx, ConstKind::Param(p))),
        })
    }
    fn extend_with_error(
        cx: Context<'cx, F>,
        definition: DefinitionId,
        args: &[Arg<'cx, F>],
    ) -> Self {
        Self::for_item(cx, definition, |p, _| {
                    if let Some(arg) = args.get(p.index() as usize) {
                        return *arg;
                    }
                    let error = cx.delay_bug("missing generic argument in prepared upstream IR");
                    match p {
                        GenericParameter::Type(_) => Ty::new_error(cx, error).into(),
                        GenericParameter::Region(_) => {
                            cx.intern_region(RegionKind::ReError(error)).into()
                        }
                        GenericParameter::Const(_, _) => {
                            Const::new(cx, ConstKind::Error(error)).into()
                        }
                    }
        })
    }
    fn split_closure_args(self) -> ClosureArgsParts<Context<'cx, F>> {
        match self.items() {
            [parent_args @ .., kind, sig, upvars] => ClosureArgsParts {
                parent_args,
                closure_kind_ty: kind.expect_ty(),
                closure_sig_as_fn_ptr_ty: sig.expect_ty(),
                tupled_upvars_ty: upvars.expect_ty(),
            },
            _ => panic!("invalid prepared upstream closure arguments"),
        }
    }
    fn split_coroutine_closure_args(self) -> CoroutineClosureArgsParts<Context<'cx, F>> {
        match self.items() {
            [parent_args @ .., kind, sig, upvars, captures] => CoroutineClosureArgsParts {
                parent_args,
                closure_kind_ty: kind.expect_ty(),
                signature_parts_ty: sig.expect_ty(),
                tupled_upvars_ty: upvars.expect_ty(),
                coroutine_captures_by_ref_ty: captures.expect_ty(),
            },
            _ => panic!("invalid prepared upstream coroutine closure arguments"),
        }
    }
    fn split_coroutine_args(self) -> CoroutineArgsParts<Context<'cx, F>> {
        match self.items() {
            [parent_args @ .., kind, resume, yield_ty, result, upvars] => CoroutineArgsParts {
                parent_args,
                kind_ty: kind.expect_ty(),
                resume_ty: resume.expect_ty(),
                yield_ty: yield_ty.expect_ty(),
                return_ty: result.expect_ty(),
                tupled_upvars_ty: upvars.expect_ty(),
            },
            _ => panic!("invalid prepared upstream coroutine arguments"),
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> BoundVarKinds<Context<'cx, F>>
    for List<'cx, BoundVariableKind<Context<'cx, F>>>
{
    fn from_vars(
        cx: Context<'cx, F>,
        iter: impl IntoIterator<Item = BoundVariableKind<Context<'cx, F>>>,
    ) -> Self {
        cx.list(iter.into_iter().collect())
    }
}
use super::model::AllocateList;

use super::model::{
    Clause, Clauses, ExprConst, ParamEnv, Pattern, Predicate, Safety, Span, Symbol, Term, Ty,
    ValueConst,
};
use rustc_type_ir::inherent::{Clause as _, Predicate as _, Ty as _};
