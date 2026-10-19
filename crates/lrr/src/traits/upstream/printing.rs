//! Pure IR formatting. No source lookup, namespace reconstruction or IO.
use super::{context::Context, facts::SourceFacts};
use rustc_type_ir::{self as ty, inherent::*, ir_print::IrPrint};
use std::fmt;
macro_rules! ordinary {
 ($($name:ident),*)=>{$(impl<'cx,F:SourceFacts<'cx>> IrPrint<ty::$name<Context<'cx,F>>> for Context<'cx,F>{
 fn print(t:&ty::$name<Self>,f:&mut fmt::Formatter<'_>)->fmt::Result{fmt::Debug::fmt(t,f)}
 fn print_debug(t:&ty::$name<Self>,f:&mut fmt::Formatter<'_>)->fmt::Result{fmt::Debug::fmt(t,f)}
 })*};
}
ordinary!(
    AliasTy,
    AliasTerm,
    TraitClause,
    HostEffectClause,
    ExistentialProjection,
    ProjectionClause,
    NormalizesTo,
    SubtypePredicate,
    CoercePredicate,
    FnSig
);
impl<'cx, F: SourceFacts<'cx>> IrPrint<ty::TraitRef<Context<'cx, F>>> for Context<'cx, F> {
    fn print(t: &ty::TraitRef<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Self::print_debug(t, f)
    }
    fn print_debug(t: &ty::TraitRef<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TraitRef")
            .field("definition", &t.def_id)
            .field("arguments", &t.args)
            .finish()
    }
}
impl<'cx, F: SourceFacts<'cx>> IrPrint<ty::ExistentialTraitRef<Context<'cx, F>>>
    for Context<'cx, F>
{
    fn print(t: &ty::ExistentialTraitRef<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Self::print_debug(t, f)
    }
    fn print_debug(t: &ty::ExistentialTraitRef<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExistentialTraitRef")
            .field("definition", &t.def_id)
            .field("arguments", &t.args)
            .finish()
    }
}
impl<'cx, F: SourceFacts<'cx>> IrPrint<ty::PatternKind<Context<'cx, F>>> for Context<'cx, F> {
    fn print(t: &ty::PatternKind<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Self::print_debug(t, f)
    }
    fn print_debug(t: &ty::PatternKind<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match t {
            ty::PatternKind::Range { start, end } => f
                .debug_struct("Range")
                .field("start", start)
                .field("end", end)
                .finish(),
            ty::PatternKind::Or(list) => f.debug_tuple("Or").field(list).finish(),
            ty::PatternKind::NotNull => f.write_str("NotNull"),
        }
    }
}
