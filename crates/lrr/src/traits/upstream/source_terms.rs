//! Actual demanded impl source terms -> shared IR, without inference or effects.
use super::{configuration::PreparedSolverConfiguration, identity::DefinitionId,
    model::{self, AllocateList, Arg, Ty, AdtDefinition, ValueConst, ScalarInt},
    parameters::SourceParameter, prepared::{self, Cx}};
use crate::{
    core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{dependency::{InputKey, InputValue}, key::QueryKey, memo::Database, poll::QueryPoll,
        provider::SyntaxProvider, seals::DiscoveryInput, value::QueryValue},
    frontend::impl_head_types::ImplHeadTypeResult,
    ir::op::PrimitiveType,
    traits::{header_syntax::ImplHeaderSyntaxKey, term::{self, BoundKind, TyTerm, GenericArg, ConstTerm, RegionTerm}},
};
use rustc_type_ir::{self as ty, inherent::*, *};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub enum SourceTypePending {
    HeadTypes = 0, SourceDefinition = 1, ParameterLayout = 2, Inference = 3, Placeholder = 4,
    RegionMetadata = 5, ConstTypeMetadata = 6, TargetPointerWidth = 7, DynMetadata = 8,
    OpaqueMetadata = 9, ProjectionMetadata = 10, FunctionMetadata = 11,
    InferenceOwner = 12,
}
#[derive(Debug)]
pub struct PreparedImplSourceTypes {
    key: ImplHeaderSyntaxKey,
    head: Arc<QueryValue>,
    bindings: super::source_bindings::SourceBindings,
    pending: Option<SourceTypePending>,
}
macro_rules! ready {
    ($value:expr) => { match $value {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    } };
}
impl PreparedImplSourceTypes {
    pub(super) fn bindings(&self) -> &super::source_bindings::SourceBindings { &self.bindings }
    pub(super) fn source_generics(&self) -> &BTreeMap<DefinitionId, Arc<QueryValue>> { &self.bindings.generics }
    pub(super) fn catalog_input(&self) -> &Arc<InputValue> { &self.bindings.catalog }
    fn catalog(&self) -> &DiscoveryInput {
        let InputValue::DiscoverySeal(catalog) = self.bindings.catalog.as_ref() else { unreachable!("captured source catalog") }; catalog
    }
    pub fn key(&self) -> &ImplHeaderSyntaxKey { &self.key }
    pub fn pending(&self) -> Option<SourceTypePending> { self.pending }
    fn head(&self) -> &crate::frontend::impl_head_types::PreparedImplHeadTypes {
        let QueryValue::ImplHeadTypes(head) = self.head.as_ref() else { unreachable!("captured head") }; head
    }
    pub(crate) fn prepare(db: &mut Database, key: &ImplHeaderSyntaxKey) -> QueryPoll<Self> {
        let head = ready!(db.fetch(&SyntaxProvider, QueryKey::ImplHeadTypes(Box::new(key.clone())))).value;
        let Some(input) = db.input(&InputKey::DiscoverySeal(key.inventory.clone())) else {
            return QueryPoll::Ready(Err(fault("source term catalog absent")));
        };
        let InputValue::DiscoverySeal(catalog) = input.as_ref() else { return QueryPoll::Ready(Err(fault("source term catalog variant differs"))); };
        if let Err(error) = prepared::catalog_world(catalog) { return QueryPoll::Ready(Err(error)); }
        let mut plan = Self { key: key.clone(), head,
            bindings: super::source_bindings::SourceBindings::empty(Arc::clone(&input)), pending: None };
        let ImplHeadTypeResult::Resolved { self_type, trait_ref } = plan.head().result() else {
            plan.pending = Some(SourceTypePending::HeadTypes); return QueryPoll::Ready(Ok(plan));
        };
        use super::source_bindings::{DefinitionKind, require_type, SourceBindings};
        let mut required = super::source_bindings::SourceRequirements::new();
        require_type(&mut required, self_type);
        if let Some(reference) = trait_ref {
            required.insert(reference.trait_key, DefinitionKind::Trait);
            for arg in &reference.args { if let GenericArg::Type(ty) = arg { require_type(&mut required, ty); } }
        }
        if let Some(owner) = plan.head().syntax().canonical_owner() { required.insert(owner.identity(), DefinitionKind::Impl); }
        plan.bindings = ready!(SourceBindings::prepare(db, input, required));
        plan.pending = plan.bindings.pending;
        QueryPoll::Ready(Ok(plan))
    }
    pub(super) fn encode<'cx>(&self, cx: Cx<'cx>, configuration: &PreparedSolverConfiguration)
        -> Result<EncodedImplTypes<'cx>, SourceTypePending> {
        if let Some(pending) = self.pending { return Err(pending); }
        if configuration.source() != self.catalog().impl_catalog.as_ref().expect("checked catalog").root()
            || cx.services().source_world() != self.catalog().seal.fingerprint() { return Err(SourceTypePending::SourceDefinition); }
        let ImplHeadTypeResult::Resolved { self_type, trait_ref } = self.head().result() else { return Err(SourceTypePending::HeadTypes); };
        let encoder = Encoder { bindings: self.bindings.type_bindings(), cx, configuration, inference: None };
        let self_ty = encoder.ty(self_type, &[])?;
        let trait_arguments = trait_ref.as_ref()
            .map(|reference| encoder.trait_arguments(reference, self_ty)).transpose()?;
        Ok(EncodedImplTypes { self_ty, trait_arguments })
    }
}
pub(super) struct EncodedImplTypes<'cx> {
    pub(super) self_ty: Ty<'cx, prepared::PreparedProvider<'cx>>,
    pub(super) trait_arguments: Option<EncodedTraitArguments<'cx>>,
}
/// Actual shared argument list plus source definition identity. The upstream
/// constructor reads genuine prepared generic metadata; it grants no complete
/// source-fact readiness and cannot bypass the upstream argument-layout guard.
pub(super) struct EncodedTraitArguments<'cx> {
    pub(super) definition: DefinitionId,
    pub(super) args: model::Args<'cx, prepared::PreparedProvider<'cx>>,
}
impl<'cx> EncodedTraitArguments<'cx> {
    pub(super) fn construct(self, cx: Cx<'cx>) -> ty::TraitRef<Cx<'cx>> {
        ty::TraitRef::new_from_args(cx, self.definition, self.args)
    }
}
/// Shared source predicate translation; source enumeration/relaxation and
/// completeness stay with the actual retained predicate plan.
pub(super) fn encode_predicate<'cx>(cx: Cx<'cx>, configuration: &PreparedSolverConfiguration,
    bindings: super::source_bindings::SourceTypeBindings<'_>, predicate: &crate::traits::predicate::PredicateResult)
    -> Result<model::Clause<'cx, prepared::PreparedProvider<'cx>>, SourceTypePending> {
    let encoder = Encoder { bindings, cx, configuration, inference: None };
    encode_predicate_using(&encoder, predicate)
}

/// The same source walker, with actual root variables owned by its caller's
/// pinned InferCtxt. Ordinary declaration encoding remains rigid.
pub(super) fn encode_root_predicate<'owner, 'cx>(
    owner: &'owner super::infer::delegate::SolverContext<'cx>,
    variables: &'owner super::root_variables::RootVariables<'owner, 'cx>,
    configuration: &'owner PreparedSolverConfiguration,
    bindings: super::source_bindings::SourceTypeBindings<'owner>,
    predicate: &crate::traits::predicate::PredicateResult,
) -> Result<model::Clause<'cx, prepared::PreparedProvider<'cx>>, SourceTypePending> {
    variables.require_owner(owner)?;
    let encoder = Encoder {
        bindings, cx: variables.context(), configuration, inference: Some(variables),
    };
    encode_predicate_using(&encoder, predicate)
}

/// Argument/result equations use the same source Type walker as predicates.
/// Numeric family and identity come from the original owning inference table;
/// this transport supplies neither coercion adjustments nor a solved goal.
pub(super) fn encode_root_type<'owner, 'cx>(
    owner: &'owner super::infer::delegate::SolverContext<'cx>,
    variables: &'owner super::root_variables::RootVariables<'owner, 'cx>,
    configuration: &'owner PreparedSolverConfiguration,
    bindings: super::source_bindings::SourceTypeBindings<'owner>,
    term: &TyTerm,
) -> Result<Ty<'cx, prepared::PreparedProvider<'cx>>, SourceTypePending> {
    variables.require_owner(owner)?;
    Encoder { bindings, cx: variables.context(), configuration, inference: Some(variables) }.ty(term, &[])
}

fn encode_predicate_using<'cx>(encoder: &Encoder<'_, 'cx>, predicate: &crate::traits::predicate::PredicateResult)
    -> Result<model::Clause<'cx, prepared::PreparedProvider<'cx>>, SourceTypePending> {
    use crate::traits::predicate::PredicateResult;
    let cx = encoder.cx;
    let kind = match predicate {
        PredicateResult::Implemented(reference) => {
            let self_ty = encoder.ty(&reference.self_ty, &[])?;
            let reference = encoder.trait_arguments(reference, self_ty)?.construct(cx);
            ty::ClauseKind::Trait(ty::TraitClause { trait_ref: reference, polarity: ty::ClausePolarity::Positive })
        }
        PredicateResult::RegionOutlives { longer, shorter } =>
            ty::ClauseKind::RegionOutlives(ty::OutlivesClause(encoder.region(longer, &[])?, encoder.region(shorter, &[])?)),
        PredicateResult::TypeOutlives { ty, region } =>
            ty::ClauseKind::TypeOutlives(ty::OutlivesClause(encoder.ty(ty, &[])?, encoder.region(region, &[])?)),
        PredicateResult::Relaxed { .. } | PredicateResult::Pending(_) =>
            unreachable!("only a prepared positive source requirement enters the Type encoder"),
    };
    Ok(model::Clause::new(cx, ty::Binder::dummy(kind)))
}
struct Encoder<'a, 'cx> {
    bindings: super::source_bindings::SourceTypeBindings<'a>, cx: Cx<'cx>, configuration: &'a PreparedSolverConfiguration,
    inference: Option<&'a super::root_variables::RootVariables<'a, 'cx>>,
}
pub(super) fn encode_declared<'cx>(cx: Cx<'cx>, configuration: &PreparedSolverConfiguration,
    bindings: super::source_bindings::SourceTypeBindings<'_>, ty: &TyTerm) -> Result<Ty<'cx, prepared::PreparedProvider<'cx>>, SourceTypePending> {
    Encoder { bindings, cx, configuration, inference: None }.ty(ty, &[])
}
impl<'cx> Encoder<'_, 'cx> {
    fn trait_arguments(&self, reference: &term::TraitRef, self_ty: Ty<'cx, prepared::PreparedProvider<'cx>>)
        -> Result<EncodedTraitArguments<'cx>, SourceTypePending> {
        let definition = *self.bindings.definitions.get(&reference.trait_key).ok_or(SourceTypePending::SourceDefinition)?;
        let mut args = vec![Arg::Type(self_ty)];
        for argument in &reference.args { args.push(self.arg(argument, &[])?); }
        self.require_arguments(definition, &args)?;
        Ok(EncodedTraitArguments { definition, args: self.cx.list(args) })
    }

    fn require_arguments(&self, definition: DefinitionId, arguments: &[Arg<'cx, prepared::PreparedProvider<'cx>>])
        -> Result<(), SourceTypePending> {
        let actual = self.cx.services().source_argument_kinds(definition)
            .ok_or(SourceTypePending::ParameterLayout)?;
        if actual.len() != arguments.len() || !actual.iter().zip(arguments).all(|(parameter, argument)|
            matches!((parameter, argument), (BoundKind::Type, Arg::Type(_))
                | (BoundKind::Region, Arg::Region(_))
                | (BoundKind::Const, Arg::Const(_)))) {
            return Err(SourceTypePending::ParameterLayout);
        }
        Ok(())
    }

    fn parameter(&self, key: term::ParamKey, kind: BoundKind) -> Result<SourceParameter, SourceTypePending> {
        self.cx.services().source_parameter(key, kind).ok_or(SourceTypePending::ParameterLayout)
    }
    fn bound(&self, depth: u32, index: u32, kind: BoundKind, binders: &[&[BoundKind]]) -> Result<(), SourceTypePending> {
        if depth as usize >= binders.len() || binders[binders.len() - 1 - depth as usize].get(index as usize) != Some(&kind) {
            return Err(SourceTypePending::ParameterLayout);
        }
        Ok(())
    }
    fn arg(&self, arg: &GenericArg, binders: &[&[BoundKind]]) -> Result<Arg<'cx, prepared::PreparedProvider<'cx>>, SourceTypePending> {
        Ok(match arg { GenericArg::Type(ty) => Arg::Type(self.ty(ty, binders)?),
            GenericArg::Region(region) => Arg::Region(self.region(region, binders)?),
            GenericArg::Const(value) => Arg::Const(self.constant(value, None, binders)?), })
    }
    fn region(&self, region: &RegionTerm, binders: &[&[BoundKind]]) -> Result<ty::Region<Cx<'cx>>, SourceTypePending> {
        let kind = match region {
            RegionTerm::Static => RegionKind::ReStatic,
            RegionTerm::Param(key) => RegionKind::ReEarlyParam(self.cx.services().source_early_region(*key).ok_or(SourceTypePending::ParameterLayout)?),
            RegionTerm::Bound { depth, index } => { self.bound(*depth, *index, BoundKind::Region, binders)?;
                RegionKind::ReBound(BoundVarIndexKind::Bound(DebruijnIndex::from_u32(*depth)), BoundRegion { var: BoundVar::from_u32(*index), kind: BoundRegionKind::Anon }) },
            RegionTerm::Local { .. } => return Err(SourceTypePending::RegionMetadata),
            RegionTerm::Placeholder(_) => return Err(SourceTypePending::Placeholder),
        };
        Ok(self.cx.intern_region(kind))
    }
    fn constant(&self, value: &ConstTerm, expected: Option<Ty<'cx, prepared::PreparedProvider<'cx>>>,
        binders: &[&[BoundKind]]) -> Result<ty::Const<Cx<'cx>>, SourceTypePending> {
        let kind = match value {
            ConstTerm::Param(key) => ConstKind::Param(self.parameter(*key, BoundKind::Const)?),
            ConstTerm::Value(bits) => {
                let ty = expected.ok_or(SourceTypePending::ConstTypeMetadata)?;
                let bytes = self.configuration.pointer_bytes().ok_or(SourceTypePending::TargetPointerWidth)?;
                if bytes != 8 { return Err(SourceTypePending::TargetPointerWidth); }
                ConstKind::Value(ValueConst { ty, tree: self.cx.valtree(ValTreeKind::Leaf(ScalarInt { bits: u128::from(*bits), bytes })) })
            }
            ConstTerm::Bound { depth, index } => { self.bound(*depth, *index, BoundKind::Const, binders)?;
                ConstKind::Bound(BoundVarIndexKind::Bound(DebruijnIndex::from_u32(*depth)), BoundConst::new(BoundVar::from_u32(*index))) },
            ConstTerm::Placeholder(_) => return Err(SourceTypePending::Placeholder),
        };
        Ok(ty::Const::new(self.cx, kind))
    }
    fn ty(&self, term: &TyTerm, binders: &[&[BoundKind]]) -> Result<Ty<'cx, prepared::PreparedProvider<'cx>>, SourceTypePending> {
        if let Some((mutable, pointee)) = crate::traits::pointer_terms::as_raw_pointer(term) {
            return Ok(self.cx.ty(TyKind::RawPtr(self.ty(pointee, binders)?, if mutable { Mutability::Mut } else { Mutability::Not })));
        }
        let kind = match term {
            TyTerm::Primitive(primitive) => match primitive {
                PrimitiveType::Unit => TyKind::Tuple(model::List::default()), PrimitiveType::Bool => TyKind::Bool, PrimitiveType::Char => TyKind::Char,
                PrimitiveType::I8 => TyKind::Int(IntTy::I8), PrimitiveType::I16 => TyKind::Int(IntTy::I16), PrimitiveType::I32 => TyKind::Int(IntTy::I32),
                PrimitiveType::I64 => TyKind::Int(IntTy::I64), PrimitiveType::I128 => TyKind::Int(IntTy::I128), PrimitiveType::Isize => TyKind::Int(IntTy::Isize),
                PrimitiveType::U8 => TyKind::Uint(UintTy::U8), PrimitiveType::U16 => TyKind::Uint(UintTy::U16), PrimitiveType::U32 => TyKind::Uint(UintTy::U32),
                PrimitiveType::U64 => TyKind::Uint(UintTy::U64), PrimitiveType::U128 => TyKind::Uint(UintTy::U128), PrimitiveType::Usize => TyKind::Uint(UintTy::Usize),
                PrimitiveType::F32 => TyKind::Float(FloatTy::F32), PrimitiveType::F64 => TyKind::Float(FloatTy::F64),
            },
            TyTerm::Str => TyKind::Str, TyTerm::Never => TyKind::Never,
            TyTerm::Param(key) => TyKind::Param(self.parameter(*key, BoundKind::Type)?),
            TyTerm::Nominal(definition, args) => {
                let id = *self.bindings.definitions.get(definition).ok_or(SourceTypePending::SourceDefinition)?;
                let args = args.iter().map(|arg| self.arg(arg, binders)).collect::<Result<Vec<_>, _>>()?;
                self.require_arguments(id, &args)?;
                TyKind::Adt(AdtDefinition { context: self.cx, definition: id }, self.cx.list(args))
            }
            TyTerm::Tuple(fields) => TyKind::Tuple(self.cx.list(fields.iter().map(|ty| self.ty(ty, binders)).collect::<Result<Vec<_>, _>>()?)),
            TyTerm::Slice(ty) => TyKind::Slice(self.ty(ty, binders)?),
            TyTerm::Ref(region, mutable, ty) => TyKind::Ref(self.region(region, binders)?, self.ty(ty, binders)?, if *mutable { Mutability::Mut } else { Mutability::Not }),
            TyTerm::Array(ty, value) => TyKind::Array(self.ty(ty, binders)?, self.constant(value, Some(self.cx.ty(TyKind::Uint(UintTy::Usize))), binders)?),
            TyTerm::Bound { depth, index } => { self.bound(*depth, *index, BoundKind::Type, binders)?;
                TyKind::Bound(BoundVarIndexKind::Bound(DebruijnIndex::from_u32(*depth)), BoundTy { var: BoundVar::from_u32(*index), kind: BoundTyKind::Anon }) },
            TyTerm::Function(signature) => {
                // Legacy runtime terms erase qualifiers (E-R5-008). Only an
                // actual source header and its retained binder justify FnPtr.
                let header = signature.value.source_header.as_ref()
                    .ok_or(SourceTypePending::FunctionMetadata)?;
                let abi = header.abi.parse::<rustc_abi::ExternAbi>()
                    .map_err(|_| SourceTypePending::FunctionMetadata)?;
                let mut nested_binders = binders.to_vec();
                nested_binders.push(&signature.variables);
                let mut inputs_and_output = signature.value.parameters.iter()
                    .map(|parameter| self.ty(parameter, &nested_binders))
                    .collect::<Result<Vec<_>, _>>()?;
                inputs_and_output.push(self.ty(&signature.value.result, &nested_binders)?);
                let bound_vars = self.cx.mk_bound_variable_kinds_from_iter(
                    signature.variables.iter().map(|kind| match kind {
                        BoundKind::Type => BoundVariableKind::Ty(BoundTyKind::Anon),
                        BoundKind::Region => BoundVariableKind::Region(BoundRegionKind::Anon),
                        BoundKind::Const => BoundVariableKind::Const,
                    }));
                let signature_types = ty::Binder::bind_with_vars(
                    ty::FnSigTys { inputs_and_output: self.cx.list(inputs_and_output) }, bound_vars);
                let fn_sig_kind = ty::FnSigKind::new(abi,
                    if header.is_unsafe { model::Safety::Unsafe } else { model::Safety::Safe },
                    header.c_variadic, None, signature.value.parameters.len())
                    .map_err(|_| SourceTypePending::FunctionMetadata)?;
                TyKind::FnPtr(signature_types, ty::FnHeader { fn_sig_kind })
            }
            TyTerm::Infer(key) => return self.inference
                .ok_or(SourceTypePending::Inference)?.variable(*key),
            TyTerm::Placeholder(_) => return Err(SourceTypePending::Placeholder),
            TyTerm::Dyn(_) => return Err(SourceTypePending::DynMetadata), TyTerm::Opaque(..) => return Err(SourceTypePending::OpaqueMetadata),
            TyTerm::Projection(projection) => {
                let binding = self.bindings.projections.get(&(projection.trait_ref.trait_key, projection.item))
                    .ok_or(SourceTypePending::ProjectionMetadata)?;
                if self.cx.services().source_projection_parent(binding.associated_definition) != Some(binding.trait_definition) {
                    return Err(SourceTypePending::ProjectionMetadata);
                }
                let mut args = vec![Arg::Type(self.ty(&projection.trait_ref.self_ty, binders)?)];
                for argument in projection.arguments() { args.push(self.arg(argument, binders)?); }
                let args = self.cx.list(args);
                let kind = ty::AliasTyKind::Projection { def_id: binding.associated_definition };
                if !self.cx.check_alias_term_args_compatible(kind.into(), args) {
                    return Err(SourceTypePending::ParameterLayout);
                }
                let alias = ty::AliasTy::new_from_args(self.cx, kind, args);
                return Ok(alias.to_ty(self.cx, ty::IsRigid::No));
            }
        };
        Ok(self.cx.ty(kind))
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into()) }
impl StableEncode for PreparedImplSourceTypes {
    const DOMAIN: &'static str = "lrr/v3/prepared-impl-source-type-bindings";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key); e.record(self.head()); e.record(&self.bindings);
        e.option(&self.pending, |e, pending| e.u32(*pending as u32));
    }
}

/// Exact source-identity controls for the unchanged Iterator/Discriminant
/// projection witnesses; never used to construct a source recipe or answer.
#[cfg(test)]
pub(super) fn check_actual_projection_alias<'cx>(
    cx: Cx<'cx>, value: Ty<'cx, prepared::PreparedProvider<'cx>>,
    source_self: term::ParamKey, expected_trait: Digest, expected_item: Digest,
) {
    let TyKind::Alias(ty::IsRigid::No, alias) = value.kind() else {
        panic!("actual projected source Type must remain an unnormalized shared alias")
    };
    let ty::AliasTyKind::Projection { def_id } = alias.kind else { panic!("source projection lost its alias kind") };
    assert_eq!(def_id.source(), expected_item, "actual associated declaration ID, never a name hash");
    assert_eq!(def_id.world(), cx.services().source_world());
    assert_eq!(cx.projection_parent(def_id).source(), expected_trait);
    assert!(cx.check_alias_term_args_compatible(alias.kind.into(), alias.args));
    let wrong = cx.list(Vec::<Arg<'cx, prepared::PreparedProvider<'cx>>>::new());
    assert!(!cx.check_alias_term_args_compatible(alias.kind.into(), wrong), "actual alias binder rejects a missing Self argument");
    let (reference, own) = alias.trait_ref_and_own_args(cx);
    assert_eq!(reference.def_id.source(), expected_trait);
    assert!(own.is_empty(), "these unchanged witnesses have no associated item arguments");
    assert_eq!(reference.args.items().len(), 1);
    assert!(matches!(reference.args.items()[0], Arg::Type(value)
        if matches!(value.kind(), TyKind::Param(parameter) if parameter.source()==source_self)));
}

#[cfg(test)]
pub(crate) fn check_actual_encodings(db: &mut Database, key: &crate::db::seals::SealKey,
    local_unit: Digest) -> Result<usize, Diagnostic> {
    use super::configuration::SolverConfigurationKey;
    let input = db.input(&InputKey::DiscoverySeal(key.clone())).ok_or_else(|| fault("control catalog absent"))?;
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else { return Err(fault("control catalog differs")); };
    let root = catalog.impl_catalog.as_ref().ok_or_else(|| fault("control is not a global source catalog"))?.root();
    let configuration = match db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(root)))) {
        QueryPoll::Ready(Ok(value)) => value.value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("actual configuration needs input")),
    };
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else { return Err(fault("control configuration variant differs")); };
    let headers = match db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(key.clone())) {
        QueryPoll::Ready(Ok(value)) => value.value, QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("control headers need input")),
    };
    let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else { return Err(fault("control headers differ")); };
    let mut plans = Vec::new();
    for raw in headers.inventory_root().headers().iter().filter(|raw| raw.scope.crate_key == local_unit) {
        let query = QueryKey::ImplSourceTypes(Box::new(ImplHeaderSyntaxKey { inventory: key.clone(), implementation: raw.identity }));
        let plan = match db.fetch(&SyntaxProvider, query.clone()) {
            QueryPoll::Ready(Ok(value)) => value.value, QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => return Err(fault("actual source bindings need input")),
        };
        let again = match db.fetch(&SyntaxProvider, query) { QueryPoll::Ready(Ok(value)) => value.value, _ => return Err(fault("actual repeated source binding not ready")) };
        assert!(Arc::ptr_eq(&plan, &again)); plans.push(plan);
    }
    let layouts = super::source_generics::SourceGenericLayouts::from_plans(Arc::clone(&input), &plans)?;
    for value in &plans {
        let QueryValue::ImplSourceTypes(plan) = value.as_ref() else { unreachable!() };
        for (id, actual) in &plan.bindings.generics {
            let QueryValue::DeclarationGenericTypes(actual) = actual.as_ref() else { unreachable!() };
            let again = match db.fetch(&SyntaxProvider, QueryKey::DeclarationGenerics(id.source())) {
                QueryPoll::Ready(Ok(value)) => value.value, _ => return Err(fault("actual generic memo is unavailable")),
            };
            assert!(Arc::ptr_eq(actual.declaration_root(), &again), "ordinary trait and constructor clients share the generic query");
        }
    }
    let before = db.trace.events.len();
    let count = configuration.with_source_context(db, Arc::clone(&input), Some(&layouts), |cx, _| -> Result<usize, Diagnostic> {
        super::source_generics::check_materialized(cx, &layouts);
        let mut count = 0;
        let mut references = 0;
        for value in &plans {
            let QueryValue::ImplSourceTypes(plan) = value.as_ref() else { return Err(fault("actual source binding variant differs")); };
            assert_eq!(plan.pending(), None);
            let actual = plan.encode(cx, configuration).map_err(|pending| fault(&format!("actual shared IR preparation pending: {pending:?}")))?;
            let again = plan.encode(cx, configuration).map_err(|_| fault("repeated IR encoding failed"))?;
            assert!(actual.self_ty == again.self_ty, "same real context interns identical Types");
            let ImplHeadTypeResult::Resolved { self_type, trait_ref } = plan.head().result() else { return Err(fault("actual head not ready")); };
            check_shape(self_type, actual.self_ty);
            let source_id = plan.head().syntax().canonical_owner().expect("actual source owner").identity();
            let id = plan.bindings.definitions[&source_id];
            let QueryValue::DeclarationGenericTypes(generics) = plan.bindings.generics[&id].as_ref() else { unreachable!() };
            let (source, _, context, _) = generics.declaration().source_parts().expect("actual retained declaration source");
            let span = model::Span { origin: source.content, start: context.range.start, end: context.range.end };
            super::infer::tests::check_actual_storage(cx, actual.self_ty, span);

            match (trait_ref, actual.trait_arguments) {
                (None, None) => {},
                (Some(source), Some(encoded)) => {
                    assert_eq!(encoded.definition.source(), source.trait_key);
                    assert_eq!(encoded.definition.world(), plan.catalog().seal.fingerprint());
                    assert_eq!(encoded.args.items().len(), source.args.len() + 1);
                    assert!(matches!(encoded.args.items().first(), Some(Arg::Type(ty)) if *ty == actual.self_ty));
                    for (source, encoded) in source.args.iter().zip(encoded.args.items().iter().skip(1)) {
                        match (source, encoded) {
                            (GenericArg::Type(source), Arg::Type(encoded)) => check_shape(source, *encoded),
                            (GenericArg::Region(source), Arg::Region(encoded)) => {
                                check_region(source, *encoded);
                                let RegionTerm::Param(key) = source else { panic!("this source trait region substitution remains unverified") };
                                let parameter = cx.services().source_parameter(*key, BoundKind::Region).unwrap();
                                assert_eq!(model::Args::identity_for_item(cx, id).items()[parameter.index() as usize], Arg::Region(*encoded),
                                    "source argument and real compiler identity args share the exact region representation");
                            }
                            _ => panic!("this actual fixture's generic argument shape remains unverified"),
                        }
                    }
                    let definition = encoded.definition;
                    let args = encoded.args;
                    let reference = encoded.construct(cx);
                    super::infer::tests::check_trait_fast_path(cx, reference, span);
                    references += 1;
                    assert_eq!(reference.def_id, definition);
                    assert_eq!(reference.args, args, "actual upstream TraitRef preserves Self and every source argument");
                    #[cfg(debug_assertions)]
                    {
                        let bad = cx.list(vec![Arg::Region(cx.intern_region(RegionKind::ReStatic))]);
                        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                            ty::TraitRef::new_from_args(cx, definition, bad))).is_err(),
                            "upstream's actual constructor must still reject a wrong Self namespace");
                    }
                }
                _ => panic!("source trait-argument presence differs"),
            }
            count += 1;
        }
        eprintln!("actual guarded upstream TraitRefs:{references}; actual pinned inference storage/rollback per source head; ordinary generic metadata and actual constructor, no ready facts or solver evaluation");
        Ok(count)
    })??;
    assert_eq!(db.trace.events.len(), before, "actual IR encoding performs no source queries");
    let mut foreign = catalog.as_ref().clone(); foreign.sources.clear();
    let foreign = Arc::new(InputValue::DiscoverySeal(Box::new(foreign)));
    assert!(configuration.with_source_context(db, Arc::clone(&foreign), Some(&layouts), |_, _| ()).is_err());
    assert!(configuration.with_context(db, foreign, |_, _| ()).is_err());
    db.begin_expansion_epoch(key.epoch + 1)?;
    assert!(configuration.with_context(db, input, |_, _| ()).is_err(), "old source world cannot create a fresh live context");
    Ok(count)
}
#[cfg(test)]
fn check_region<'cx>(source: &RegionTerm, encoded: ty::Region<Cx<'cx>>) {
    match (source, encoded.kind()) {
        (RegionTerm::Static, RegionKind::ReStatic) => {},
        (RegionTerm::Param(key), RegionKind::ReEarlyParam(region)) => {
            assert_eq!(*key, region.parameter.source());
            assert_eq!(region.parameter.kind(), BoundKind::Region);
            assert!(region.name.is_some(), "retain the real declared region name, as identity args do");
        }
        _ => panic!("actual source region shape remains unverified"),
    }
}
#[cfg(test)]
fn check_shape<'cx>(source: &TyTerm, encoded: Ty<'cx, prepared::PreparedProvider<'cx>>) {
    match (source, encoded.kind()) {
        (TyTerm::Primitive(PrimitiveType::Isize), TyKind::Int(IntTy::Isize))
        | (TyTerm::Primitive(PrimitiveType::I32), TyKind::Int(IntTy::I32))
        | (TyTerm::Primitive(PrimitiveType::Usize), TyKind::Uint(UintTy::Usize))
        | (TyTerm::Primitive(PrimitiveType::U8), TyKind::Uint(UintTy::U8))
        | (TyTerm::Primitive(PrimitiveType::U32), TyKind::Uint(UintTy::U32))
        | (TyTerm::Primitive(PrimitiveType::U64), TyKind::Uint(UintTy::U64)) => {},
        (TyTerm::Primitive(PrimitiveType::Unit), TyKind::Tuple(fields)) => assert!(fields.items().is_empty()),
        (TyTerm::Slice(source), TyKind::Slice(encoded)) => check_shape(source, encoded),
        (TyTerm::Ref(region, mutable, source), TyKind::Ref(encoded_region, encoded_source, encoded_mut)) => {
            check_region(region, encoded_region);
            assert_eq!(*mutable, encoded_mut == Mutability::Mut);
            check_shape(source, encoded_source);
        }
        (TyTerm::Nominal(definition, args), TyKind::Adt(adt, encoded_args)) => {
            assert_eq!(adt.definition.source(), *definition);
            assert_eq!(args.len(), encoded_args.items().len());
            for (source, encoded) in args.iter().zip(encoded_args.items()) {
                match (source, encoded) {
                    (GenericArg::Type(source), Arg::Type(encoded)) => check_shape(source, *encoded),
                    _ => panic!("this fixture argument remains structurally unverified"),
                }
            }
        }
        (TyTerm::Param(source), TyKind::Param(parameter)) => {
            assert_eq!(*source, parameter.source()); assert_eq!(parameter.kind(), BoundKind::Type);
            assert_eq!(parameter.index(), source.ordinal);
        }
        _ => panic!("actual fixture shared IR shape remains unverified"),
    }
}

#[cfg(test)]
pub(crate) fn check_erased_function_pending(db: &mut Database, key: &crate::db::seals::SealKey,
    local_unit: Digest) -> Result<(), Diagnostic> {
    use super::configuration::SolverConfigurationKey;
    let input = db.input(&InputKey::DiscoverySeal(key.clone())).ok_or_else(|| fault("alias control catalog absent"))?;
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else { return Err(fault("alias control catalog differs")); };
    let source = catalog.impl_catalog.as_ref().ok_or_else(|| fault("alias control needs global source"))?.root();
    let config = match db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(source)))) {
        QueryPoll::Ready(Ok(value)) => value.value, _ => return Err(fault("actual alias configuration not ready")),
    };
    let QueryValue::SolverConfiguration(config) = config.as_ref() else { return Err(fault("actual alias configuration differs")); };
    let headers = match db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(key.clone())) {
        QueryPoll::Ready(Ok(value)) => value.value, _ => return Err(fault("actual alias headers not ready")),
    };
    let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else { return Err(fault("actual alias headers differ")); };
    let raw = headers.inventory_root().headers().iter().find(|raw| raw.scope.crate_key == local_unit)
        .ok_or_else(|| fault("actual alias local impl absent"))?;
    let retained = match db.fetch(&SyntaxProvider, QueryKey::ImplSourceTypes(Box::new(ImplHeaderSyntaxKey {
        inventory: key.clone(), implementation: raw.identity,
    }))) { QueryPoll::Ready(Ok(value)) => value.value, _ => return Err(fault("actual alias source plan not ready")) };
    let QueryValue::ImplSourceTypes(plan) = retained.as_ref() else { return Err(fault("actual alias source plan differs")); };
    let ImplHeadTypeResult::Resolved { self_type: TyTerm::Function(signature), .. } = plan.head().result() else {
        panic!("the ordinary alias path must reach the function-pointer family");
    };
    let source_header = signature.value.source_header.as_ref()
        .expect("the unchanged unsafe extern alias must retain its actual source header");
    assert!(source_header.is_unsafe);
    assert_eq!(source_header.abi, "C");
    assert!(!source_header.c_variadic);
    let layouts = super::source_generics::SourceGenericLayouts::from_plans(Arc::clone(&input), &[Arc::clone(&retained)])?;
    config.with_source_context(db, input, Some(&layouts), |cx, _| {
        match plan.encode(cx, config) {
            Ok(encoded) => {
                let TyKind::FnPtr(_, header) = encoded.self_ty.kind() else {
                    panic!("the actual source alias must preserve the FnPtr family");
                };
                assert_eq!(header.safety(), model::Safety::Unsafe);
                assert_eq!(header.abi(), rustc_abi::ExternAbi::C { unwind: false });
                assert!(!header.c_variadic());
            }
            Err(pending) => panic!("the actual source alias failed shared encoding: {pending:?}"),
        }
        let mut erased = signature.clone();
        erased.value.source_header = None;
        let encoder = Encoder { bindings: plan.bindings.type_bindings(), cx, configuration: config, inference: None };
        assert!(matches!(encoder.ty(&TyTerm::Function(erased), &[]), Err(SourceTypePending::FunctionMetadata)),
            "removing actual source metadata must retain the erased runtime hold");
    })
}

/// Source component control over an independently selected unchanged impl.
/// It observes genuine declaration/IR values; it neither publishes ready facts
/// nor evaluates a goal, and introduces no alternate source-region producer.
#[cfg(test)]
pub(crate) fn check_anonymous_impl_regions(
    db: &mut Database, key: &crate::db::seals::SealKey, local_unit: Digest,
    module: &[&str], expected_written: &[(&str, BoundKind)],
    expected_anonymous: usize, expected_upstream_source_ordinals: &[u32],
    inherited_method: Option<Digest>,
) -> Result<usize, Diagnostic> {
    use super::parameters::{DeclarationParameters, DeclaredParameterPreparation};
    use super::source_generics::SourceGenericLayouts;
    use crate::syntax::signatures::GenericKind;
    use ra_ap_syntax::{AstNode, ast};
    use std::collections::BTreeSet;
    macro_rules! fetched {
        ($query:expr) => { match db.fetch(&SyntaxProvider, $query) {
            QueryPoll::Ready(Ok(value)) => value.value,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(_) => return Err(fault("anonymous impl control needs source input")),
        } };
    }
    let input = db.input(&InputKey::DiscoverySeal(key.clone()))
        .ok_or_else(|| fault("anonymous impl control catalog absent"))?;
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else { return Err(fault("anonymous impl control catalog differs")); };
    let root = catalog.impl_catalog.as_ref().ok_or_else(|| fault("anonymous impl control requires actual completion"))?.root();
    let configuration = fetched!(QueryKey::SolverConfiguration(Box::new(
        super::configuration::SolverConfigurationKey::from_source(root))));
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else { return Err(fault("anonymous impl configuration differs")); };
    let headers = fetched!(QueryKey::PreparedScopeImplHeaders(key.clone()));
    let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else { return Err(fault("anonymous impl inventory differs")); };
    let raw = headers.inventory_root().headers().iter().filter(|raw|
        raw.scope.crate_key == local_unit && raw.scope.module_path.iter().map(String::as_str).eq(module.iter().copied()))
        .collect::<Vec<_>>();
    assert_eq!(raw.len(), 1, "exact original module has one actual impl header");
    let query = QueryKey::ImplSourceTypes(Box::new(ImplHeaderSyntaxKey { inventory: key.clone(), implementation: raw[0].identity }));
    let retained = fetched!(query.clone());
    let again = fetched!(query.clone());
    assert!(Arc::ptr_eq(&retained, &again), "anonymous impl plan shares the actual source memo");
    let QueryValue::ImplSourceTypes(plan) = retained.as_ref() else { return Err(fault("anonymous impl source query differs")); };
    assert_eq!(plan.pending(), None, "actual source head {:?}: {:?}", plan.head().syntax().path(), plan.head().result());
    let syntax = plan.head().syntax();
    let definition = syntax.canonical_owner().expect("actual anonymous impl canonical owner").identity();
    let id = plan.bindings.definitions[&definition];
    let QueryValue::DeclarationGenericTypes(generic_types) = plan.bindings.generics[&id].as_ref() else { unreachable!("retained declaration generics") };
    let declaration = generic_types.declaration();
    let written = declaration.parameters().iter().filter(|p| p.anonymous_region.is_none())
        .map(|p| (p.name.as_str(), match p.kind { GenericKind::Lifetime => BoundKind::Region,
            GenericKind::Type => BoundKind::Type, GenericKind::Const => BoundKind::Const })).collect::<Vec<_>>();
    assert_eq!(written, expected_written, "original written generic order is retained");
    let anonymous = declaration.parameters().iter().filter(|p| p.anonymous_region.is_some()).collect::<Vec<_>>();
    assert_eq!(anonymous.len(), expected_anonymous);
    let mut sites = BTreeSet::new();
    for parameter in &anonymous {
        let site = parameter.anonymous_region.unwrap();
        assert!(sites.insert((site.start, site.end)), "repeated anonymous occurrences have distinct actual sites");
        let node = syntax.parsed().syntax_at(site, |node|
            ast::RefType::can_cast(node.kind()) || ast::LifetimeArg::can_cast(node.kind()))?
            .expect("anonymous parameter retains its actual impl-head syntax site");
        if let Some(reference) = ast::RefType::cast(node.clone()) {
            assert!(reference.lifetime().is_none_or(|lt| lt.syntax().text().to_string() == "'_"));
        } else {
            let argument = ast::LifetimeArg::cast(node).unwrap();
            assert_eq!(argument.lifetime().unwrap().syntax().text().to_string(), "'_");
        }
    }
    let DeclaredParameterPreparation::Ready(parameters) = DeclarationParameters::prepare(declaration)? else {
        return Err(fault("anonymous impl declaration mapping pending"));
    };
    let actual_ordinals = (0..parameters.count()).map(|index|
        parameters.from_upstream(index).map(|p| p.source().ordinal)).collect::<Result<Vec<_>, _>>()?;
    assert_eq!(actual_ordinals, expected_upstream_source_ordinals, "real upstream lifetime-first slots preserve source ordinals");
    assert_eq!(syntax.generics(), declaration.parameters(), "header and ordinary declaration share the actual parameter capture");
    if let Some(method) = inherited_method {
        let member = fetched!(QueryKey::DeclarationGenerics(method));
        let QueryValue::DeclarationGenerics(member) = member.as_ref() else { return Err(fault("anonymous parent method query differs")); };
        let layout = member.layout().expect("actual associated method layout");
        assert_eq!(layout.own, 0);
        assert_eq!(layout.inherited as usize, declaration.parameters().len());
        assert!(!layout.implicit_self);
        for (parent, child) in declaration.parameters().iter().zip(member.parameters()) {
            assert_eq!(parent.kind, child.kind); assert_eq!(parent.anonymous_region, child.anonymous_region);
        }
        let DeclaredParameterPreparation::Ready(member_parameters) = DeclarationParameters::prepare(member)? else {
            return Err(fault("actual anonymous parent method mapping pending"));
        };
        assert_eq!(member_parameters.parent_count(), parameters.count());
        for index in 0..parameters.count() {
            let parent = parameters.from_upstream(index)?;
            let child = member_parameters.from_upstream(index)?;
            assert_eq!(child.source().definition, method);
            assert_eq!(child.source().ordinal, parent.source().ordinal);
            assert_eq!(child.kind(), parent.kind());
        }
    }
    let layouts = SourceGenericLayouts::from_plans(Arc::clone(&input), &[Arc::clone(&retained)])?;
    let expected = anonymous.iter().map(|p| {
        let key = term::ParamKey { definition, ordinal: p.ordinal };
        (key, layouts.parameter(key, BoundKind::Region).expect("actual anonymous region parameter"))
    }).collect::<BTreeMap<_, _>>();
    struct Occurrences<'a> { expected: &'a BTreeMap<term::ParamKey, SourceParameter>, seen: BTreeSet<term::ParamKey> }
    impl<'cx> TypeVisitor<Cx<'cx>> for Occurrences<'_> {
        type Result = ();
        fn visit_region(&mut self, region: ty::Region<Cx<'cx>>) {
            if let RegionKind::ReEarlyParam(region) = region.kind()
                && let Some(expected) = self.expected.get(&region.parameter.source()) {
                assert_eq!(&region.parameter, expected);
                assert!(region.name.is_none(), "anonymous source sites do not acquire invented compiler names");
                self.seen.insert(region.parameter.source());
            }
        }
    }
    let before = db.trace.events.len();
    configuration.with_source_context(db, Arc::clone(&input), Some(&layouts), |cx, _| -> Result<(), Diagnostic> {
        let generics = cx.generics_of(id);
        assert_eq!(generics.count(), parameters.count() as usize);
        let identity = model::Args::identity_for_item(cx, id);
        for index in 0..parameters.count() {
            let parameter = parameters.from_upstream(index)?;
            assert_eq!(cx.services().source_parameter(parameter.source(), parameter.kind()), Some(parameter));
            assert_eq!(parameter.index(), index);
            if let Some(expected) = expected.get(&parameter.source()) {
                assert!(matches!(identity.items()[index as usize], Arg::Region(region)
                    if matches!(region.kind(), RegionKind::ReEarlyParam(region) if region.parameter == *expected && region.name.is_none())));
            }
        }
        let encoded = plan.encode(cx, configuration).map_err(|pending| fault(&format!("actual anonymous head encoding pending: {pending:?}")))?;
        let mut occurrences = Occurrences { expected: &expected, seen: BTreeSet::new() };
        encoded.self_ty.visit_with(&mut occurrences);
        if let Some(reference) = encoded.trait_arguments { reference.args.visit_with(&mut occurrences); }
        assert_eq!(occurrences.seen, expected.keys().copied().collect(), "all actual source sites survive in genuine ReEarlyParam values");
        Ok(())
    })??;
    assert_eq!(db.trace.events.len(), before, "anonymous head IR encoding makes no source query");
    let again = fetched!(query);
    assert!(Arc::ptr_eq(&retained, &again));
    eprintln!("actual anonymous impl source path={} canonical={definition:?} module={module:?}: {expected_anonymous} distinct sites {sites:?}, written={written:?}, upstream source ordinals={actual_ordinals:?}, inherited_method={inherited_method:?}; genuine early regions/identity args, memo/cold; no ready/root/runtime", syntax.path().as_str());
    Ok(expected_anonymous)
}
