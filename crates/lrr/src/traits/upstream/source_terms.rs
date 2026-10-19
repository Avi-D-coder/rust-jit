//! Actual demanded impl source terms -> shared IR, without inference or effects.
use super::{configuration::PreparedSolverConfiguration, identity::DefinitionId,
    model::{self, AllocateList, Arg, Ty, AdtDefinition, EarlyRegion, ValueConst, ScalarInt},
    parameters::SourceParameter, prepared::{self, Cx, DefinitionReceipt}};
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
}
#[derive(Debug)]
pub struct PreparedImplSourceTypes {
    key: ImplHeaderSyntaxKey,
    head: Arc<QueryValue>,
    catalog: Arc<InputValue>,
    definitions: BTreeMap<Digest, DefinitionId>,
    receipts: Vec<DefinitionReceipt>,
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
    fn catalog(&self) -> &DiscoveryInput {
        let InputValue::DiscoverySeal(catalog) = self.catalog.as_ref() else { unreachable!("captured source catalog") }; catalog
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
        let world = match prepared::catalog_world(catalog) { Ok(world) => world, Err(error) => return QueryPoll::Ready(Err(error)) };
        let local_unit = catalog.impl_catalog.as_ref().expect("checked catalog").root().outline.crate_key;
        let mut plan = Self { key: key.clone(), head, catalog: Arc::clone(&input),
            definitions: BTreeMap::new(), receipts: Vec::new(), pending: None };
        let ImplHeadTypeResult::Resolved { self_type, trait_ref } = plan.head().result() else {
            plan.pending = Some(SourceTypePending::HeadTypes); return QueryPoll::Ready(Ok(plan));
        };
        let mut terms = vec![self_type];
        let mut required = BTreeMap::new();
        if let Some(reference) = trait_ref {
            required.insert(reference.trait_key, false);
            for arg in &reference.args { if let GenericArg::Type(ty) = arg { terms.push(ty); } }
        }
        while let Some(ty) = terms.pop() {
            if let Some((_, pointee)) = crate::traits::pointer_terms::as_raw_pointer(ty) { terms.push(pointee); continue; }
            match ty {
                TyTerm::Nominal(definition, args) => {
                    required.insert(*definition, true);
                    for arg in args { if let GenericArg::Type(ty) = arg { terms.push(ty); } }
                }
                TyTerm::Tuple(fields) => terms.extend(fields),
                TyTerm::Array(ty, _) | TyTerm::Slice(ty) | TyTerm::Ref(_, _, ty) => terms.push(ty),
                TyTerm::Function(function) => { terms.extend(&function.value.parameters); terms.push(&function.value.result); }
                _ => {},
            }
        }
        for (definition, nominal) in required {
            let Some(receipt) = ready!(DefinitionReceipt::observe(db, definition)) else {
                plan.pending = Some(SourceTypePending::SourceDefinition); return QueryPoll::Ready(Ok(plan));
            };
            let valid = if nominal { match receipt.is_nominal() { Ok(value) => value, Err(error) => return QueryPoll::Ready(Err(error)) } } else { receipt.is_trait() };
            if !valid { plan.pending = Some(SourceTypePending::SourceDefinition); return QueryPoll::Ready(Ok(plan)); }
            plan.definitions.insert(definition, receipt.definition_id(world, local_unit)); plan.receipts.push(receipt);
        }
        QueryPoll::Ready(Ok(plan))
    }
    pub(super) fn encode<'cx>(&self, cx: Cx<'cx>, configuration: &PreparedSolverConfiguration)
        -> Result<EncodedImplTypes<'cx>, SourceTypePending> {
        if let Some(pending) = self.pending { return Err(pending); }
        if configuration.source() != self.catalog().impl_catalog.as_ref().expect("checked catalog").root()
            || cx.services().source_world() != self.catalog().seal.fingerprint() { return Err(SourceTypePending::SourceDefinition); }
        let ImplHeadTypeResult::Resolved { self_type, trait_ref } = self.head().result() else { return Err(SourceTypePending::HeadTypes); };
        let encoder = Encoder { plan: self, cx, configuration };
        let self_ty = encoder.ty(self_type, &[])?;
        let trait_arguments = trait_ref.as_ref().map(|reference| {
            let definition = *self.definitions.get(&reference.trait_key).ok_or(SourceTypePending::SourceDefinition)?;
            let mut args = vec![Arg::Type(self_ty)];
            for argument in &reference.args { args.push(encoder.arg(argument, &[])?); }
            Ok::<_, SourceTypePending>(EncodedTraitArguments { definition, args: cx.list(args) })
        }).transpose()?;
        Ok(EncodedImplTypes { self_ty, trait_arguments })
    }
}
pub(super) struct EncodedImplTypes<'cx> {
    pub(super) self_ty: Ty<'cx, prepared::PreparedProvider<'cx>>,
    pub(super) trait_arguments: Option<EncodedTraitArguments<'cx>>,
}
/// Actual shared argument list plus source definition identity. Constructing
/// TraitRef calls upstream's generic-layout guard and must wait for real typed
/// fact publication; do not bypass that guard or install dummy source callbacks.
pub(super) struct EncodedTraitArguments<'cx> {
    pub(super) definition: DefinitionId,
    pub(super) args: model::Args<'cx, prepared::PreparedProvider<'cx>>,
}
struct Encoder<'a, 'cx> { plan: &'a PreparedImplSourceTypes, cx: Cx<'cx>, configuration: &'a PreparedSolverConfiguration }
impl<'cx> Encoder<'_, 'cx> {
    fn parameter(&self, key: term::ParamKey, kind: BoundKind) -> Result<SourceParameter, SourceTypePending> {
        SourceParameter::from_impl(self.plan.head().syntax(), key, kind).map_err(|_| SourceTypePending::ParameterLayout)
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
            RegionTerm::Param(key) => RegionKind::ReEarlyParam(EarlyRegion { parameter: self.parameter(*key, BoundKind::Region)?, name: None }),
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
                let id = *self.plan.definitions.get(definition).ok_or(SourceTypePending::SourceDefinition)?;
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
            TyTerm::Infer(_) => return Err(SourceTypePending::Inference), TyTerm::Placeholder(_) => return Err(SourceTypePending::Placeholder),
            TyTerm::Dyn(_) => return Err(SourceTypePending::DynMetadata), TyTerm::Opaque(..) => return Err(SourceTypePending::OpaqueMetadata),
            TyTerm::Projection(_) => return Err(SourceTypePending::ProjectionMetadata),
        };
        Ok(self.cx.ty(kind))
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into()) }
impl StableEncode for PreparedImplSourceTypes {
    const DOMAIN: &'static str = "lrr/v1/prepared-impl-source-type-bindings";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key); e.record(self.head()); e.digest(self.catalog().seal.fingerprint());
        e.map(&self.definitions, |e, source, id| { e.digest(*source); e.record(id); });
        e.option(&self.pending, |e, pending| e.u32(*pending as u32));
    }
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
    let before = db.trace.events.len();
    let count = configuration.with_context(db, Arc::clone(&input), |cx, _| -> Result<usize, Diagnostic> {
        let mut count = 0;
        for value in &plans {
            let QueryValue::ImplSourceTypes(plan) = value.as_ref() else { return Err(fault("actual source binding variant differs")); };
            assert_eq!(plan.pending(), None);
            let actual = plan.encode(cx, configuration).map_err(|pending| fault(&format!("actual shared IR preparation pending: {pending:?}")))?;
            let again = plan.encode(cx, configuration).map_err(|_| fault("repeated IR encoding failed"))?;
            assert!(actual.self_ty == again.self_ty, "same real context interns identical Types");
            let ImplHeadTypeResult::Resolved { self_type, trait_ref } = plan.head().result() else { return Err(fault("actual head not ready")); };
            check_shape(self_type, actual.self_ty);
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
                            _ => panic!("this actual fixture's generic argument shape remains unverified"),
                        }
                    }
                }
                _ => panic!("source trait-argument presence differs"),
            }
            count += 1;
        }
        Ok(count)
    })??;
    assert_eq!(db.trace.events.len(), before, "actual IR encoding performs no source queries");
    let mut foreign = catalog.as_ref().clone(); foreign.sources.clear();
    let foreign = Arc::new(InputValue::DiscoverySeal(Box::new(foreign)));
    assert!(configuration.with_context(db, foreign, |_, _| ()).is_err());
    db.begin_expansion_epoch(key.epoch + 1)?;
    assert!(configuration.with_context(db, input, |_, _| ()).is_err(), "old source world cannot create a fresh live context");
    Ok(count)
}
#[cfg(test)]
fn check_shape<'cx>(source: &TyTerm, encoded: Ty<'cx, prepared::PreparedProvider<'cx>>) {
    match (source, encoded.kind()) {
        (TyTerm::Primitive(PrimitiveType::Isize), TyKind::Int(IntTy::Isize))
        | (TyTerm::Primitive(PrimitiveType::I32), TyKind::Int(IntTy::I32)) => {},
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
