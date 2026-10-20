//! Owned export of original source variables after their real upstream owner
//! applies a response. No normalization, inference, source query or runtime commit.
use super::{
    identity::DefinitionId,
    infer::delegate::SolverContext,
    model::{Arg, Ty},
    prepared::{Cx, PreparedProvider},
    root_input::SourceVariableFamily,
    root_variables::RootVariables,
    source_generics::SourceGenericLayouts,
    source_terms::SourceTypePending,
};
use crate::{
    core::digest::Digest,
    ir::op::PrimitiveType,
    traits::term::{
        BoundKind, ConstTerm, GenericArg, InferVarKey, ProjectionKey, RegionTerm, TraitRef, TyTerm,
    },
};
use rustc_type_ir::{self as ty, inherent::*};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum RootResponsePending {
    Owner(SourceTypePending),
    Specification(InferVarKey),
    UnknownInference,
    SourceDefinition,
    Parameter,
    Projection,
    Region,
    Constant,
    UnsupportedType,
}
/// Portable data, not a solver success/admission/activation or commit token.
/// Every original key has a value; unresolved representatives have explicit
/// families and retain actual original source identity.
#[derive(Debug)]
pub(super) struct OwnedRootResponse {
    world: Digest,
    values: BTreeMap<InferVarKey, TyTerm>,
    unresolved: BTreeMap<InferVarKey, SourceVariableFamily>,
}
impl OwnedRootResponse {
    pub(super) fn world(&self) -> Digest {
        self.world
    }
    pub(super) fn values(&self) -> &BTreeMap<InferVarKey, TyTerm> {
        &self.values
    }
    pub(super) fn unresolved(&self) -> &BTreeMap<InferVarKey, SourceVariableFamily> {
        &self.unresolved
    }
}
type RootTy<'cx> = Ty<'cx, PreparedProvider<'cx>>;
fn family(value: RootTy<'_>) -> Option<SourceVariableFamily> {
    match value.kind() {
        ty::TyKind::Infer(ty::InferTy::TyVar(_)) => Some(SourceVariableFamily::Type),
        ty::TyKind::Infer(ty::InferTy::IntVar(_)) => Some(SourceVariableFamily::Integer),
        ty::TyKind::Infer(ty::InferTy::FloatVar(_)) => Some(SourceVariableFamily::Float),
        _ => None,
    }
}
/// Export only values read from original variables in the same inference owner.
/// The specification map belongs to that owner; callers cannot replace it with
/// an arbitrary family assignment. Fresh unmapped response variables stay pending.
pub(super) fn export_root_response<'cx>(
    owner: &SolverContext<'cx>,
    variables: &RootVariables<'_, 'cx>,
    layouts: &SourceGenericLayouts,
) -> Result<OwnedRootResponse, RootResponsePending> {
    variables
        .require_owner(owner)
        .map_err(RootResponsePending::Owner)?;
    let mut resolved = BTreeMap::new();
    let mut representatives = HashMap::new();
    for (&key, &kind) in variables.specifications() {
        let original = variables
            .variable(key)
            .map_err(RootResponsePending::Owner)?;
        if family(original) != Some(kind) {
            return Err(RootResponsePending::Specification(key));
        }
        let value = variables
            .resolved(key)
            .map_err(RootResponsePending::Owner)?;
        // A Type variable can resolve to an integer variable. Only an actual
        // original Integer key may represent that still-open numeric variable;
        // never relabel the Type source key or invent an Integer key.
        if family(value) == Some(kind) {
            representatives.entry(value).or_insert((key, kind));
        }
        resolved.insert(key, value);
    }
    let mut exporter = Exporter {
        cx: variables.context(),
        layouts,
        representatives,
        unresolved: BTreeMap::new(),
    };
    let values = resolved
        .into_iter()
        .map(|(key, value)| exporter.ty(value).map(|value| (key, value)))
        .collect::<Result<_, _>>()?;
    Ok(OwnedRootResponse {
        world: exporter.cx.services().source_world(),
        values,
        unresolved: exporter.unresolved,
    })
}
struct Exporter<'data, 'cx> {
    cx: Cx<'cx>,
    layouts: &'data SourceGenericLayouts,
    representatives: HashMap<RootTy<'cx>, (InferVarKey, SourceVariableFamily)>,
    unresolved: BTreeMap<InferVarKey, SourceVariableFamily>,
}
impl<'cx> Exporter<'_, 'cx> {
    fn definition(&self, id: DefinitionId) -> Result<Digest, RootResponsePending> {
        if id.world() != self.cx.services().source_world()
            || self.layouts.type_bindings().definitions.get(&id.source()) != Some(&id)
        {
            return Err(RootResponsePending::SourceDefinition);
        }
        Ok(id.source())
    }
    fn parameter(
        &self,
        parameter: super::parameters::SourceParameter,
        kind: BoundKind,
    ) -> Result<crate::traits::term::ParamKey, RootResponsePending> {
        let key = parameter.source();
        let layouts = self.layouts;
        let bindings = layouts.type_bindings();
        let id = bindings
            .definitions
            .get(&key.definition)
            .ok_or(RootResponsePending::Parameter)?;
        self.definition(*id)?;
        if self.layouts.parameter(key, kind) != Some(parameter) {
            return Err(RootResponsePending::Parameter);
        }
        Ok(key)
    }
    fn region(&self, value: ty::Region<Cx<'cx>>) -> Result<RegionTerm, RootResponsePending> {
        match value.kind() {
            ty::RegionKind::ReStatic => Ok(RegionTerm::Static),
            ty::RegionKind::ReEarlyParam(parameter) => Ok(RegionTerm::Param(
                self.parameter(parameter.parameter, BoundKind::Region)?,
            )),
            // Portable source regions cannot represent erased/inference/late
            // bound identities here. In particular, erased never means static.
            _ => Err(RootResponsePending::Region),
        }
    }
    fn constant(&self, value: ty::Const<Cx<'cx>>) -> Result<ConstTerm, RootResponsePending> {
        match value.kind() {
            ty::ConstKind::Param(parameter) => Ok(ConstTerm::Param(
                self.parameter(parameter, BoundKind::Const)?,
            )),
            ty::ConstKind::Value(value) => {
                // Existing portable ConstTerm::Value is the source encoder's
                // u64 usize scalar, not arbitrary CTFE or signed/ADT values.
                let ty::TyKind::Uint(ty::UintTy::Usize) = value.ty.kind() else {
                    return Err(RootResponsePending::Constant);
                };
                let ty::ValTreeKind::Leaf(scalar) = value.tree.kind() else {
                    return Err(RootResponsePending::Constant);
                };
                if scalar.bytes != 8 {
                    return Err(RootResponsePending::Constant);
                }
                Ok(ConstTerm::Value(
                    u64::try_from(scalar.bits).map_err(|_| RootResponsePending::Constant)?,
                ))
            }
            _ => Err(RootResponsePending::Constant),
        }
    }
    fn argument(
        &mut self,
        value: Arg<'cx, PreparedProvider<'cx>>,
    ) -> Result<GenericArg, RootResponsePending> {
        Ok(match value {
            Arg::Type(value) => GenericArg::Type(self.ty(value)?),
            Arg::Region(value) => GenericArg::Region(self.region(value)?),
            Arg::Const(value) => GenericArg::Const(self.constant(value)?),
        })
    }
    fn arguments(
        &mut self,
        values: &[Arg<'cx, PreparedProvider<'cx>>],
        kinds: &[BoundKind],
    ) -> Result<Vec<GenericArg>, RootResponsePending> {
        if values.len() != kinds.len()
            || values.iter().zip(kinds).any(|(value, kind)| {
                !matches!(
                    (value, kind),
                    (Arg::Type(_), BoundKind::Type)
                        | (Arg::Region(_), BoundKind::Region)
                        | (Arg::Const(_), BoundKind::Const)
                )
            })
        {
            return Err(RootResponsePending::Parameter);
        }
        values.iter().map(|value| self.argument(*value)).collect()
    }
    fn projection(&mut self, alias: ty::AliasTy<Cx<'cx>>) -> Result<TyTerm, RootResponsePending> {
        let ty::AliasTyKind::Projection { def_id } = alias.kind else {
            return Err(RootResponsePending::Projection);
        };
        self.definition(def_id)?;
        let bindings = self.layouts.type_bindings();
        let mut members = bindings
            .projections
            .iter()
            .filter(|(_, binding)| binding.associated_definition == def_id);
        let Some((&(trait_source, item), binding)) = members.next() else {
            return Err(RootResponsePending::Projection);
        };
        if members.next().is_some()
            || self.definition(binding.trait_definition)? != trait_source
            || self.layouts.projection_parent(def_id) != Some(binding.trait_definition)
        {
            return Err(RootResponsePending::Projection);
        }
        let crate::db::value::QueryValue::TraitAssociatedTypes(query) = binding.query.as_ref()
        else {
            return Err(RootResponsePending::Projection);
        };
        if query.key().trait_definition != trait_source || query.lookup(item) != Ok(def_id.source())
        {
            return Err(RootResponsePending::Projection);
        }
        let layouts = self.layouts;
        let kinds = layouts
            .projection_argument_kinds(def_id)
            .ok_or(RootResponsePending::Projection)?;
        let parent_kinds = layouts
            .argument_kinds(binding.trait_definition)
            .ok_or(RootResponsePending::Projection)?;
        let parent_count = parent_kinds.len();
        if parent_kinds.first() != Some(&BoundKind::Type)
            || kinds.get(..parent_count) != Some(parent_kinds)
        {
            return Err(RootResponsePending::Projection);
        }
        let arguments = self.arguments(alias.args.items(), kinds)?;
        let Some(GenericArg::Type(self_ty)) = arguments.first() else {
            return Err(RootResponsePending::Projection);
        };
        Ok(TyTerm::Projection(Box::new(ProjectionKey {
            trait_ref: TraitRef {
                trait_key: trait_source,
                self_ty: self_ty.clone(),
                args: arguments[1..parent_count].to_vec(),
            },
            item,
            item_args: arguments[parent_count..].to_vec(),
        })))
    }
    fn ty(&mut self, value: RootTy<'cx>) -> Result<TyTerm, RootResponsePending> {
        use PrimitiveType as P;
        use ty::{FloatTy, IntTy, TyKind, UintTy};
        Ok(match value.kind() {
            TyKind::Bool => TyTerm::Primitive(P::Bool),
            TyKind::Char => TyTerm::Primitive(P::Char),
            TyKind::Int(value) => TyTerm::Primitive(match value {
                IntTy::I8 => P::I8,
                IntTy::I16 => P::I16,
                IntTy::I32 => P::I32,
                IntTy::I64 => P::I64,
                IntTy::I128 => P::I128,
                IntTy::Isize => P::Isize,
            }),
            TyKind::Uint(value) => TyTerm::Primitive(match value {
                UintTy::U8 => P::U8,
                UintTy::U16 => P::U16,
                UintTy::U32 => P::U32,
                UintTy::U64 => P::U64,
                UintTy::U128 => P::U128,
                UintTy::Usize => P::Usize,
            }),
            TyKind::Float(value) => TyTerm::Primitive(match value {
                FloatTy::F32 => P::F32,
                FloatTy::F64 => P::F64,
                _ => return Err(RootResponsePending::UnsupportedType),
            }),
            TyKind::Str => TyTerm::Str,
            TyKind::Never => TyTerm::Never,
            TyKind::Tuple(fields) if fields.items().is_empty() => TyTerm::Primitive(P::Unit),
            TyKind::Tuple(fields) => TyTerm::Tuple(
                fields
                    .items()
                    .iter()
                    .map(|field| self.ty(*field))
                    .collect::<Result<_, _>>()?,
            ),
            TyKind::Slice(element) => TyTerm::Slice(Box::new(self.ty(element)?)),
            TyKind::Array(element, size) => {
                TyTerm::Array(Box::new(self.ty(element)?), self.constant(size)?)
            }
            TyKind::Ref(region, element, mutable) => TyTerm::Ref(
                self.region(region)?,
                mutable == ty::Mutability::Mut,
                Box::new(self.ty(element)?),
            ),
            TyKind::RawPtr(element, mutable) => crate::traits::pointer_terms::raw_pointer(
                mutable == ty::Mutability::Mut,
                self.ty(element)?,
            ),
            TyKind::Adt(adt, args) => {
                let id = self.definition(adt.definition)?;
                let layouts = self.layouts;
                let kinds = layouts
                    .argument_kinds(adt.definition)
                    .ok_or(RootResponsePending::Parameter)?;
                TyTerm::Nominal(id, self.arguments(args.items(), kinds)?)
            }
            TyKind::Param(parameter) => TyTerm::Param(self.parameter(parameter, BoundKind::Type)?),
            TyKind::Alias(ty::IsRigid::No, alias) => self.projection(alias)?,
            TyKind::Infer(_) => {
                let &(key, kind) = self
                    .representatives
                    .get(&value)
                    .ok_or(RootResponsePending::UnknownInference)?;
                if family(value) != Some(kind) {
                    return Err(RootResponsePending::UnknownInference);
                }
                self.unresolved.insert(key, kind);
                TyTerm::Infer(key)
            }
            _ => return Err(RootResponsePending::UnsupportedType),
        })
    }
}
