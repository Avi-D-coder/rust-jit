//! Pinned rustc reflective-field client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; tools/upstream_reflection.py.
use super::{
    context::Context,
    facts::SourceFacts,
    model::{AdtDefinition, Args},
};
use rustc_abi::{FIRST_VARIANT, FieldIdx, VariantIdx};
use rustc_type_ir::{self as ty, ConstKind, FieldInfo, inherent::*};

impl<'cx, F: SourceFacts<'cx>> AdtDefinition<'cx, F> {
    pub(super) fn pinned_field_representing_type_info(
        self,
        tcx: Context<'cx, F>,
        args: Args<'cx, F>,
    ) -> Option<FieldInfo<Context<'cx, F>>> {
        if !self.is_field_representing_type() {
            return None;
        }
        let base = args.type_at(0);
        let variant_idx = match args.const_at(1).kind() {
            ConstKind::Value(v) => VariantIdx::from_u32(v.to_leaf().to_u32()),
            _ => return None,
        };
        let field_idx = match args.const_at(2).kind() {
            ConstKind::Value(v) => FieldIdx::from_u32(v.to_leaf().to_u32()),
            _ => return None,
        };
        let (ty, variant, name) = match base.kind() {
            ty::TyKind::Adt(base_def, base_args) => {
                let variant = base_def.variant(variant_idx);
                let field = &variant.fields[field_idx.as_usize()];
                let ty = field.ty(tcx, base_args).skip_norm_wip();
                (ty, base_def.is_enum().then_some(variant.name), field.name)
            }
            ty::TyKind::Tuple(tys) => {
                if variant_idx != FIRST_VARIANT {
                    panic!(
                        "expected variant of tuple to be FIRST_VARIANT, but found {variant_idx:?}"
                    )
                }
                (
                    if let Some(ty) = tys.items().get(field_idx.as_usize()) {
                        *ty
                    } else {
                        panic!(
                            "expected valid tuple index, but got {field_idx:?}, tuple length: {}",
                            tys.len()
                        )
                    },
                    None,
                    tcx.intern_integer_symbol(field_idx.as_usize()),
                )
            }
            _ => panic!(),
        };
        Some(FieldInfo {
            base,
            ty,
            variant,
            variant_idx,
            name,
            field_idx,
        })
    }

    fn is_field_representing_type(self) -> bool {
        self.context
            .services()
            .field_representing_type_definition(self.context)
            == Some(self.definition)
    }
}
