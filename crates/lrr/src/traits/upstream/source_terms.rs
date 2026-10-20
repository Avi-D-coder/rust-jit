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
        Ok(EncodedTraitArguments { definition, args: self.cx.list(args) })
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
                TyKind::Adt(AdtDefinition { context: self.cx, definition: id }, self.cx.list(args))
            }
            TyTerm::Tuple(fields) => TyKind::Tuple(self.cx.list(fields.iter().map(|ty| self.ty(ty, binders)).collect::<Result<Vec<_>, _>>()?)),
            TyTerm::Slice(ty) => TyKind::Slice(self.ty(ty, binders)?),
            TyTerm::Ref(region, mutable, ty) => TyKind::Ref(self.region(region, binders)?, self.ty(ty, binders)?, if *mutable { Mutability::Mut } else { Mutability::Not }),
            TyTerm::Array(ty, value) => TyKind::Array(self.ty(ty, binders)?, self.constant(value, Some(self.cx.ty(TyKind::Uint(UintTy::Usize))), binders)?),
            TyTerm::Bound { depth, index } => { self.bound(*depth, *index, BoundKind::Type, binders)?;
                TyKind::Bound(BoundVarIndexKind::Bound(DebruijnIndex::from_u32(*depth)), BoundTy { var: BoundVar::from_u32(*index), kind: BoundTyKind::Anon }) },
            // Owned runtime fn terms erase safety/ABI (E-R5-008); aliases can
            // hide those qualifiers from the impl's syntax. Source facts must
            // provide the actual function signature before a shared fn Type
            // can be built. Never guess safe Rust ABI from this erased term.
            TyTerm::Function(_) => return Err(SourceTypePending::FunctionMetadata),
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
    let plan = match db.fetch(&SyntaxProvider, QueryKey::ImplSourceTypes(Box::new(ImplHeaderSyntaxKey {
        inventory: key.clone(), implementation: raw.identity,
    }))) { QueryPoll::Ready(Ok(value)) => value.value, _ => return Err(fault("actual alias source plan not ready")) };
    let QueryValue::ImplSourceTypes(plan) = plan.as_ref() else { return Err(fault("actual alias source plan differs")); };
    assert!(matches!(plan.head().result(), ImplHeadTypeResult::Resolved { self_type: TyTerm::Function(_), .. }),
        "the ordinary alias path really reaches the erased runtime fn family");
    config.with_context(db, input, |cx, _| {
        assert!(matches!(plan.encode(cx, config), Err(SourceTypePending::FunctionMetadata)),
            "an erased unsafe extern alias must never become a safe Rust shared Type");
    })
}
