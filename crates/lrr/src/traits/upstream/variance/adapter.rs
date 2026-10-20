//! Immutable compiler-context views over genuinely prepared source Types.
//! No query, source lookup, inferred variance policy or solver lives here.
use super::{Cx, Ty, VariancePending};
use crate::traits::upstream::{
    identity::DefinitionId,
    model::{self, GenericParameter},
};
use rustc_type_ir::{self as ty, Interner, TypeSuperVisitable, inherent::*};
use std::{collections::BTreeMap, ops::ControlFlow};

#[derive(Debug, Clone, Copy)]
pub(super) enum VarianceParameterKind {
    Type,
    Lifetime,
    Const {},
}
pub(super) struct VarianceParameter {
    pub index: u32,
    pub kind: VarianceParameterKind,
}
pub(super) struct VarianceGenerics {
    pub own_params: Vec<VarianceParameter>,
    pub parent: Option<DefinitionId>,
    count: usize,
}
impl VarianceGenerics {
    pub(super) fn count(&self) -> usize {
        self.count
    }
    pub(super) fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub(super) fn nominal<'cx>(
        generics: model::Generics<'cx, super::super::prepared::PreparedProvider<'cx>>,
    ) -> Result<Self, VariancePending> {
        if generics.parent_count != 0 {
            return Err(VariancePending::GenericLayout);
        }
        Ok(Self {
            count: generics.parameters.items().len(),
            own_params: generics
                .parameters
                .iter()
                .map(|parameter| VarianceParameter {
                    index: parameter.index(),
                    kind: match parameter {
                        GenericParameter::Type(_) => VarianceParameterKind::Type,
                        GenericParameter::Region(_) => VarianceParameterKind::Lifetime,
                        GenericParameter::Const(..) => VarianceParameterKind::Const {},
                    },
                })
                .collect(),
            parent: None,
        })
    }
}
pub(super) struct VarianceItem<'cx> {
    pub generics: VarianceGenerics,
    pub ty: Ty<'cx>,
    pub fields: Vec<Ty<'cx>>,
    pub signature: Option<ty::Binder<Cx<'cx>, ty::FnSig<Cx<'cx>>>>,
}
#[derive(Default)]
pub(super) struct VarianceLangItems {
    pub phantom_data: Option<DefinitionId>,
    pub unsafe_cell: Option<DefinitionId>,
    pub covariant_unsafe_cell: Option<DefinitionId>,
}
impl VarianceLangItems {
    pub(super) fn phantom_data(&self) -> Option<DefinitionId> {
        self.phantom_data
    }
    pub(super) fn unsafe_cell_type(&self) -> Option<DefinitionId> {
        self.unsafe_cell
    }
    pub(super) fn covariant_unsafe_cell_type(&self) -> Option<DefinitionId> {
        self.covariant_unsafe_cell
    }
}
pub(super) struct VarianceInputs<'cx> {
    pub items: BTreeMap<DefinitionId, VarianceItem<'cx>>,
    pub lang: VarianceLangItems,
}
#[derive(Clone, Copy)]
pub(super) struct VarianceSource<'a, 'cx> {
    pub inputs: &'a VarianceInputs<'cx>,
}
impl<'a, 'cx> VarianceSource<'a, 'cx> {
    pub(super) fn inference_items(self) -> impl Iterator<Item = DefinitionId> + 'a {
        self.inputs.items.keys().copied()
    }
    pub(super) fn generics_of(self, id: DefinitionId) -> &'a VarianceGenerics {
        &self.inputs.items[&id].generics
    }
    pub(super) fn lang_items(self) -> &'a VarianceLangItems {
        &self.inputs.lang
    }
    pub(super) fn definition_type(self, id: DefinitionId) -> Ty<'cx> {
        self.inputs.items[&id].ty
    }
    pub(super) fn field_types(self, id: DefinitionId) -> &'a [Ty<'cx>] {
        &self.inputs.items[&id].fields
    }
    pub(super) fn def_path_str(self, id: DefinitionId) -> String {
        format!("{id:?}")
    }
    pub(super) fn local_definition(self, id: DefinitionId) -> Option<DefinitionId> {
        <DefinitionId as ty::inherent::DefId<Cx<'cx>>>::as_local(id)
            .filter(|id| self.inputs.items.contains_key(id))
    }
    pub(super) fn fn_sig(self, id: DefinitionId) -> ty::Binder<Cx<'cx>, ty::FnSig<Cx<'cx>>> {
        self.inputs.items[&id]
            .signature
            .expect("actual typed signature must be admitted before variance inference")
    }
    pub(super) fn variances_of(self, _: DefinitionId) -> &'a [ty::Variance] {
        unreachable!(
            "foreign variance is an explicit source preparation hold before algorithm entry"
        )
    }
    pub(super) fn expand_free_alias_tys(self, _: Ty<'cx>) -> Ty<'cx> {
        unreachable!(
            "free alias expansion is an explicit source preparation hold before algorithm entry"
        )
    }
}

/// Admissibility/current source-layout validation only. This computes no
/// variance and does not replace any pinned constraint traversal.
pub(super) fn require_supported<'cx>(
    cx: Cx<'cx>,
    inputs: &VarianceInputs<'cx>,
) -> Result<(), VariancePending> {
    struct Check<'a, 'cx> {
        cx: Cx<'cx>,
        inputs: &'a VarianceInputs<'cx>,
        parameters: &'cx [GenericParameter<'cx, super::super::prepared::PreparedProvider<'cx>>],
    }
    impl<'cx> ty::TypeVisitor<Cx<'cx>> for Check<'_, 'cx> {
        type Result = ControlFlow<VariancePending>;
        fn visit_ty(&mut self, value: Ty<'cx>) -> Self::Result {
            match value.kind() {
                ty::TyKind::Adt(definition, args) => {
                    if args.items().len() != self.cx.generics_of(definition.definition).count() {
                        return ControlFlow::Break(VariancePending::GenericLayout);
                    }
                    if !args.items().is_empty()
                        && !self.inputs.items.contains_key(&definition.definition)
                    {
                        return ControlFlow::Break(VariancePending::Foreign);
                    }
                }
                ty::TyKind::Param(parameter) => {
                    match self.parameters.get(parameter.index() as usize) {
                        Some(GenericParameter::Type(expected)) if *expected == parameter => {}
                        _ => return ControlFlow::Break(VariancePending::GenericLayout),
                    }
                }
                ty::TyKind::Alias(_, alias)
                    if matches!(alias.kind, ty::AliasTyKind::Free { .. }) =>
                {
                    return ControlFlow::Break(VariancePending::FreeAlias);
                }
                ty::TyKind::FnPtr(..) | ty::TyKind::FnDef(..) => {
                    return ControlFlow::Break(VariancePending::Signature);
                }
                ty::TyKind::Infer(_)
                | ty::TyKind::Placeholder(_)
                | ty::TyKind::Bound(..)
                | ty::TyKind::Dynamic(..)
                | ty::TyKind::Coroutine(..)
                | ty::TyKind::CoroutineClosure(..)
                | ty::TyKind::CoroutineWitness(..)
                | ty::TyKind::Closure(..)
                | ty::TyKind::Foreign(..)
                | ty::TyKind::Error(_)
                | ty::TyKind::UnsafeBinder(_) => {
                    return ControlFlow::Break(VariancePending::TypeShape);
                }
                _ => {}
            }
            value.super_visit_with(self)
        }
        fn visit_region(&mut self, value: ty::Region<Cx<'cx>>) -> Self::Result {
            match value.kind() {
                ty::RegionKind::ReStatic => ControlFlow::Continue(()),
                ty::RegionKind::ReEarlyParam(parameter) => {
                    match self.parameters.get(parameter.index() as usize) {
                        Some(GenericParameter::Region(expected)) if *expected == parameter => {
                            ControlFlow::Continue(())
                        }
                        _ => ControlFlow::Break(VariancePending::GenericLayout),
                    }
                }
                _ => ControlFlow::Break(VariancePending::TypeShape),
            }
        }
        fn visit_const(&mut self, value: ty::Const<Cx<'cx>>) -> Self::Result {
            if let ty::ConstKind::Param(parameter) = value.kind() {
                match self.parameters.get(parameter.index() as usize) {
                    Some(GenericParameter::Const(expected, _)) if *expected == parameter => {}
                    _ => return ControlFlow::Break(VariancePending::GenericLayout),
                }
            }
            value.super_visit_with(self)
        }
    }
    use ty::TypeVisitable;
    for (definition, item) in &inputs.items {
        let mut check = Check {
            cx,
            inputs,
            parameters: cx.generics_of(*definition).parameters.items(),
        };
        for field in &item.fields {
            if let ControlFlow::Break(pending) = field.visit_with(&mut check) {
                return Err(pending);
            }
        }
    }
    Ok(())
}
