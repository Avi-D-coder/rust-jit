//! Pinned inference variable/resolution client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference_context.py.
use super::{
    context::Context,
    facts::SourceFacts,
    identity::DefinitionId,
    model::{Arg, Args, GenericParameter, Generics},
};
use rustc_type_ir::{Interner, inherent::*};
// Field projection only: the actual source table is already parent-first.
impl<'cx, F: SourceFacts<'cx>> GenericParameter<'cx, F> {
    pub(crate) fn index(self) -> u32 {
        match self {
            Self::Type(p) | Self::Const(p, _) => p.index(),
            Self::Region(p) => p.parameter.index(),
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> Args<'cx, F> {
    pub(crate) fn for_item<Make>(
        cx: Context<'cx, F>,
        def_id: DefinitionId,
        mut mk_kind: Make,
    ) -> Self
    where
        Make: FnMut(GenericParameter<'cx, F>, &[Arg<'cx, F>]) -> Arg<'cx, F>,
    {
        let defs = cx.generics_of(def_id);
        let count = defs.count();
        let mut args = Vec::with_capacity(count);
        Self::fill_single(&mut args, defs, &mut mk_kind);
        cx.mk_args_from_iter(args.into_iter())
    }
    pub(crate) fn fill_single<Make>(
        args: &mut Vec<Arg<'cx, F>>,
        defs: Generics<'cx, F>,
        mk_kind: &mut Make,
    ) where
        Make: FnMut(GenericParameter<'cx, F>, &[Arg<'cx, F>]) -> Arg<'cx, F>,
    {
        args.reserve(defs.parameters.items().len());
        for param in defs.parameters.iter() {
            let kind = mk_kind(param, args);
            assert_eq!(param.index() as usize, args.len(), "{args:#?}, {defs:#?}");
            args.push(kind);
        }
    }
}
