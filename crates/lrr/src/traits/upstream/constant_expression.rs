//! Pinned compiler const-expression representation and constructors; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; unchanged originals/licenses: vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_relations.py.
use super::{
    context::Context,
    expression_operators::{BinOp, CastKind, UnOp},
    facts::SourceFacts,
    model::{Arg, Args, Ty},
};
use rustc_type_ir::{self as ty, Interner, inherent::*};
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum ExprKind {
    Binop(BinOp),
    UnOp(UnOp),
    FunctionCall,
    Cast(CastKind),
}
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) struct ExprConst<'cx, F: SourceFacts<'cx>> {
    pub kind: ExprKind,
    args: Args<'cx, F>,
}

impl<'cx, F: SourceFacts<'cx>> rustc_type_ir::inherent::ExprConst<Context<'cx, F>>
    for ExprConst<'cx, F>
{
    fn args(self) -> Args<'cx, F> {
        self.args
    }
}

impl<'cx, F: SourceFacts<'cx>> ExprConst<'cx, F> {
    pub(crate) fn new_binop(
        tcx: Context<'cx, F>,
        binop: BinOp,
        lhs_ty: Ty<'cx, F>,
        rhs_ty: Ty<'cx, F>,
        lhs_ct: ty::Const<Context<'cx, F>>,
        rhs_ct: ty::Const<Context<'cx, F>>,
    ) -> Self {
        let args = tcx.mk_args_from_iter::<_, Arg<'cx, F>>(
            [lhs_ty.into(), rhs_ty.into(), lhs_ct.into(), rhs_ct.into()].into_iter(),
        );

        Self {
            kind: ExprKind::Binop(binop),
            args,
        }
    }

    pub(crate) fn binop_args(
        self,
    ) -> (
        Ty<'cx, F>,
        Ty<'cx, F>,
        ty::Const<Context<'cx, F>>,
        ty::Const<Context<'cx, F>>,
    ) {
        assert!(matches!(self.kind, ExprKind::Binop(_)));

        match self.args().items() {
            [lhs_ty, rhs_ty, lhs_ct, rhs_ct] => (
                lhs_ty.expect_ty(),
                rhs_ty.expect_ty(),
                lhs_ct.expect_const(),
                rhs_ct.expect_const(),
            ),
            _ => panic!("Invalid args for `Binop` expr {self:?}"),
        }
    }

    pub(crate) fn new_unop(
        tcx: Context<'cx, F>,
        unop: UnOp,
        ty: Ty<'cx, F>,
        ct: ty::Const<Context<'cx, F>>,
    ) -> Self {
        let args = tcx.mk_args_from_iter::<_, Arg<'cx, F>>([ty.into(), ct.into()].into_iter());

        Self {
            kind: ExprKind::UnOp(unop),
            args,
        }
    }

    pub(crate) fn unop_args(self) -> (Ty<'cx, F>, ty::Const<Context<'cx, F>>) {
        assert!(matches!(self.kind, ExprKind::UnOp(_)));

        match self.args().items() {
            [ty, ct] => (ty.expect_ty(), ct.expect_const()),
            _ => panic!("Invalid args for `UnOp` expr {self:?}"),
        }
    }

    pub(crate) fn new_call(
        tcx: Context<'cx, F>,
        func_ty: Ty<'cx, F>,
        func_expr: ty::Const<Context<'cx, F>>,
        arguments: impl IntoIterator<Item = ty::Const<Context<'cx, F>>>,
    ) -> Self {
        let args = tcx.mk_args_from_iter::<_, Arg<'cx, F>>(
            [func_ty.into(), func_expr.into()]
                .into_iter()
                .chain(arguments.into_iter().map(|ct| ct.into())),
        );

        Self {
            kind: ExprKind::FunctionCall,
            args,
        }
    }

    pub(crate) fn call_args(
        self,
    ) -> (
        Ty<'cx, F>,
        ty::Const<Context<'cx, F>>,
        impl Iterator<Item = ty::Const<Context<'cx, F>>>,
    ) {
        assert!(matches!(self.kind, ExprKind::FunctionCall));

        match self.args().items() {
            [func_ty, func, rest @ ..] => (
                func_ty.expect_ty(),
                func.expect_const(),
                rest.iter().map(|arg| arg.expect_const()),
            ),
            _ => panic!("Invalid args for `Call` expr {self:?}"),
        }
    }

    pub(crate) fn new_cast(
        tcx: Context<'cx, F>,
        cast: CastKind,
        value_ty: Ty<'cx, F>,
        value: ty::Const<Context<'cx, F>>,
        to_ty: Ty<'cx, F>,
    ) -> Self {
        let args = tcx.mk_args_from_iter::<_, Arg<'cx, F>>(
            [value_ty.into(), value.into(), to_ty.into()].into_iter(),
        );

        Self {
            kind: ExprKind::Cast(cast),
            args,
        }
    }

    pub(crate) fn cast_args(self) -> (Ty<'cx, F>, ty::Const<Context<'cx, F>>, Ty<'cx, F>) {
        assert!(matches!(self.kind, ExprKind::Cast(_)));

        match self.args().items() {
            [value_ty, value, to_ty] => (
                value_ty.expect_ty(),
                value.expect_const(),
                to_ty.expect_ty(),
            ),
            _ => panic!("Invalid args for `Cast` expr {self:?}"),
        }
    }

    pub(crate) fn new(kind: ExprKind, args: Args<'cx, F>) -> Self {
        Self { kind, args }
    }

    pub(crate) fn args(self) -> Args<'cx, F> {
        self.args
    }
}
