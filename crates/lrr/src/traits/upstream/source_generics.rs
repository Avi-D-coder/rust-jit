//! Actual source parameter plans and their context-owned typed generic facts.
//! Neither phase authorizes a solver root or replaces complete source facts.
use super::{identity::DefinitionId, prepared::{self, Cx}, model::{self, AllocateList, GenericParameter, Generics, Symbol, EarlyRegion},
    parameters::{SourceParameter, DeclarationParameters, DeclaredParameterPreparation},
    source_bindings::{SourceBindings, SourceProjectionBinding, SourceTypeBindings, DefinitionKind},
    configuration::PreparedSolverConfiguration};
use crate::{core::{diagnostic::Diagnostic, digest::Digest},
    db::{dependency::InputValue, value::QueryValue, memo::Database, poll::QueryPoll},
    frontend::{declaration_generics::DeclarationGenerics, declaration_generic_types::DeclarationGenericTypes},
    traits::term::{ParamKey, BoundKind}};
use std::{collections::BTreeMap, sync::Arc};
struct SourceGenericPlan {
    query: Arc<QueryValue>,
    upstream: Vec<SourceParameter>,
    source: Vec<SourceParameter>,
    kinds: Vec<BoundKind>,
    parent_count: usize,
}
impl SourceGenericPlan {
    fn types(&self) -> &DeclarationGenericTypes {
        let QueryValue::DeclarationGenericTypes(types) = self.query.as_ref() else { unreachable!("retained generic Type query") }; types
    }
}
pub(super) struct SourceGenericLayouts {
    catalog: Arc<InputValue>,
    declarations: BTreeMap<DefinitionId, SourceGenericPlan>,
    definitions: BTreeMap<Digest, DefinitionId>,
    projections: BTreeMap<(Digest, Digest), SourceProjectionBinding>,
    projection_layouts: BTreeMap<DefinitionId, Vec<BoundKind>>,
    projection_parents: BTreeMap<DefinitionId, DefinitionId>,
}
impl SourceGenericLayouts {
    pub(super) fn from_bindings<'a>(catalog: Arc<InputValue>, bindings: impl IntoIterator<Item = &'a SourceBindings>) -> Result<Self, Diagnostic> {
        let mut result = Self { catalog, declarations: BTreeMap::new(), definitions: BTreeMap::new(),
            projections: BTreeMap::new(), projection_layouts: BTreeMap::new(), projection_parents: BTreeMap::new() };
        for bindings in bindings {
            if !Arc::ptr_eq(&bindings.catalog, &result.catalog) || bindings.pending.is_some() {
                return Err(fault("source generic bindings are pending or belong to another catalog"));
            }
            for (source, id) in &bindings.definitions {
                if result.definitions.insert(*source, *id).is_some_and(|previous| previous != *id) {
                    return Err(fault("actual source generic definition identity changed"));
                }
            }
            for (key, binding) in &bindings.projections {
                if let Some(previous) = result.projections.insert(*key, binding.clone())
                    && (previous.trait_definition != binding.trait_definition
                        || previous.associated_definition != binding.associated_definition
                        || !Arc::ptr_eq(&previous.query, &binding.query)) {
                    return Err(fault("actual projection declaration changed between source plans"));
                }
            }
            for (id, query) in &bindings.generics {
                if let Some(previous) = result.declarations.get(id) {
                    if !Arc::ptr_eq(&previous.query, query) { return Err(fault("generic Type query changed between actual plans")); }
                    continue;
                }
                let QueryValue::DeclarationGenericTypes(types) = query.as_ref() else { return Err(fault("actual generic Type query differs")); };
                if types.key().definition != id.source() || types.pending().is_some() { return Err(fault("actual generic Type metadata is pending")); }
                let DeclaredParameterPreparation::Ready(parameters) = DeclarationParameters::prepare(types.declaration())? else {
                    return Err(fault("actual source parameter mapping is pending"));
                };
                let upstream = (0..parameters.count()).map(|index| parameters.from_upstream(index)).collect::<Result<Vec<_>, _>>()?;
                let kinds = upstream.iter().map(|parameter| parameter.kind()).collect();
                let mut source = upstream.clone(); source.sort_by_key(|parameter| parameter.source().ordinal);
                result.declarations.insert(*id, SourceGenericPlan { query: Arc::clone(query), upstream, source, kinds, parent_count: parameters.parent_count() as usize });
            }
        }
        for binding in result.projections.values() {
            let parent = result.declarations.get(&binding.trait_definition)
                .ok_or_else(|| fault("projection source parent generic layout absent"))?;
            let associated = result.declarations.get(&binding.associated_definition)
                .ok_or_else(|| fault("projection source associated generic layout absent"))?;
            if !super::source_bindings::projection_parent_matches(parent.types().declaration(), associated.types().declaration())
                || associated.parent_count != parent.upstream.len() {
                return Err(fault("projection generic parent identity or argument count differs"));
            }
            if result.projection_parents.insert(binding.associated_definition, binding.trait_definition)
                .is_some_and(|previous| previous != binding.trait_definition) {
                return Err(fault("actual projection declaration has conflicting trait parents"));
            }
            result.projection_layouts.insert(binding.associated_definition, associated.kinds.clone());
        }
        Ok(result)
    }
    pub(super) fn from_plans(catalog: Arc<InputValue>, plans: &[Arc<QueryValue>]) -> Result<Self, Diagnostic> {
        let bindings = plans.iter().map(|value| match value.as_ref() {
            QueryValue::ImplSourceTypes(plan) if plan.pending().is_none() => Ok(plan.bindings()),
            _ => Err(fault("actual source Type plans are missing or pending")),
        }).collect::<Result<Vec<_>, _>>()?;
        Self::from_bindings(catalog, bindings)
    }
    pub(super) fn prepare_declarations(db: &mut Database, catalog: Arc<InputValue>, declarations: &[Digest]) -> QueryPoll<Self> {
        let bindings = match SourceBindings::prepare(db, Arc::clone(&catalog), declarations.iter().map(|id| (*id, DefinitionKind::Declaration)).collect::<BTreeMap<_, _>>()) {
            QueryPoll::Ready(Ok(bindings)) => bindings, QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready(Self::from_bindings(catalog, [&bindings]))
    }
    pub(super) fn matches_catalog(&self, catalog: &Arc<InputValue>) -> bool { Arc::ptr_eq(&self.catalog, catalog) }
    pub(super) fn get(&self, id: DefinitionId) -> Option<&DeclarationGenerics> { self.declarations.get(&id).map(|plan| plan.types().declaration()) }
    pub(super) fn argument_kinds(&self, id: DefinitionId) -> Option<&[BoundKind]> { self.declarations.get(&id).map(|plan| plan.kinds.as_slice()) }
    pub(super) fn projection_argument_kinds(&self, id: DefinitionId) -> Option<&[BoundKind]> {
        self.projection_layouts.get(&id).map(Vec::as_slice)
    }
    pub(super) fn projection_parent(&self, id: DefinitionId) -> Option<DefinitionId> {
        self.projection_parents.get(&id).copied()
    }
    pub(super) fn type_bindings(&self) -> SourceTypeBindings<'_> {
        SourceTypeBindings { definitions: &self.definitions, projections: &self.projections }
    }
    pub(super) fn parameter(&self, key: ParamKey, kind: BoundKind) -> Option<SourceParameter> {
        let id = self.definitions.get(&key.definition)?;
        let parameter = *self.declarations.get(id)?.source.get(key.ordinal as usize)?;
        (parameter.source() == key && parameter.kind() == kind).then_some(parameter)
    }
    pub(super) fn early_region(&self, key: ParamKey) -> Option<EarlyRegion<'_>> {
        let parameter = self.parameter(key, BoundKind::Region)?;
        let id = self.definitions.get(&key.definition)?;
        let name = self.declarations.get(id)?.types().declaration().parameters().get(key.ordinal as usize)?;
        Some(EarlyRegion { parameter, name: Some(Symbol(name.name.as_str())) })
    }
    pub(super) fn materialize<'cx>(&'cx self, cx: Cx<'cx>, configuration: &PreparedSolverConfiguration)
        -> Result<BTreeMap<DefinitionId, Generics<'cx, prepared::PreparedProvider<'cx>>>, Diagnostic> {
        let mut result = BTreeMap::new();
        for (id, plan) in &self.declarations {
            let mut parameters = Vec::new();
            for parameter in &plan.upstream {
                parameters.push(match parameter.kind() {
                    BoundKind::Type => GenericParameter::Type(*parameter),
                    BoundKind::Region => {
                        GenericParameter::Region(self.early_region(parameter.source())
                            .ok_or_else(|| fault("actual region parameter source absent"))?)
                    }
                    BoundKind::Const => {
                        let ty = plan.types().const_types().get(&parameter.source().ordinal)
                            .ok_or_else(|| fault("actual const parameter Type is pending"))?;
                        let ty = super::source_terms::encode_declared(cx, configuration, self.type_bindings(), ty)
                            .map_err(|pending| fault(&format!("actual const parameter Type representation pending: {pending:?}")))?;
                        GenericParameter::Const(*parameter, ty)
                    }
                });
            }
            result.insert(*id, Generics { parameters: cx.list(parameters), parent_count: plan.parent_count });
        }
        Ok(result)
    }
    #[cfg(test)]
    pub(super) fn definitions(&self) -> impl Iterator<Item = DefinitionId> + '_ { self.declarations.keys().copied() }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into()) }

#[cfg(test)]
pub(super) fn check_materialized<'cx>(cx: Cx<'cx>, layouts: &SourceGenericLayouts) -> (usize, usize, usize) {
    use rustc_type_ir::{self as ty, inherent::*, Interner};
    assert!(cx.services().ready(cx).is_err(), "typed generics do not make complete source facts or a ready solver");
    let mut rows = 0; let mut constants = 0; let mut regions = 0; let mut identity_refusals = 0;
    for id in layouts.definitions() {
        let actual = cx.generics_of(id);
        let args = model::Args::identity_for_item(cx, id);
        let source = &layouts.declarations[&id];
        let (parse, _, context, _) = source.types().declaration().source_parts().expect("actual declaration source");
        super::infer::tests::check_actual_fresh_arguments(cx, id, model::Span {
            origin: parse.content, start: context.range.start, end: context.range.end,
        });

        assert_eq!(actual.parent_count, source.parent_count);
        assert_eq!(actual.count(), source.upstream.len()); assert_eq!(args.len(), actual.count());
        cx.services().require_generic_value(id, actual).unwrap();
        let mut wrong_parent = actual; wrong_parent.parent_count += 1;
        assert!(cx.services().require_generic_value(id, wrong_parent).is_err());
        for (index, ((parameter, argument), expected)) in actual.parameters.iter().zip(args.items()).zip(&source.upstream).enumerate() {
            let real = match (parameter, argument) {
                (GenericParameter::Type(parameter), model::Arg::Type(ty)) => {
                    assert!(matches!(ty.kind(), ty::TyKind::Param(p) if p == parameter)); parameter
                }
                (GenericParameter::Region(region), model::Arg::Region(argument)) => {
                    assert!(matches!(argument.kind(), ty::RegionKind::ReEarlyParam(p) if p == region)); regions += 1; region.parameter
                }
                (GenericParameter::Const(parameter, ty), model::Arg::Const(argument)) => {
                    assert!(matches!(argument.kind(), ty::ConstKind::Param(p) if p == parameter));
                    let input = &source.types().const_types()[&parameter.source().ordinal];
                    match (input, ty.kind()) {
                        (crate::traits::term::TyTerm::Primitive(crate::ir::op::PrimitiveType::Usize), ty::TyKind::Uint(ty::UintTy::Usize))
                        | (crate::traits::term::TyTerm::Primitive(crate::ir::op::PrimitiveType::Bool), ty::TyKind::Bool) => {},
                        _ => panic!("this source const parameter Type remains unverified"),
                    }
                    let wrong_ty = if matches!(ty.kind(), ty::TyKind::Bool) { cx.ty(ty::TyKind::Uint(ty::UintTy::Usize)) } else { cx.ty(ty::TyKind::Bool) };
                    let mut wrong = actual.parameters.items().to_vec(); wrong[index] = GenericParameter::Const(parameter, wrong_ty);
                    assert!(cx.services().require_generic_value(id, Generics { parameters: cx.list(wrong), parent_count: actual.parent_count }).is_err());
                    constants += 1; parameter
                }
                _ => panic!("actual generic argument namespaces differ"),
            };
            if let Some(alternative) = layouts.declarations.values().flat_map(|plan| &plan.upstream)
                .find(|p| p.kind() == real.kind() && p.index() == real.index() && p.source() != real.source()) {
                let wrong = match parameter {
                    GenericParameter::Type(_) => GenericParameter::Type(*alternative),
                    GenericParameter::Region(region) => GenericParameter::Region(EarlyRegion { parameter: *alternative, name: region.name }),
                    GenericParameter::Const(_, ty) => GenericParameter::Const(*alternative, ty),
                };
                let mut parameters = actual.parameters.items().to_vec(); parameters[index] = wrong;
                assert!(cx.services().require_generic_value(id, Generics { parameters: cx.list(parameters), parent_count: actual.parent_count }).is_err(),
                    "same kind/position from another actual declaration is not this source parameter");
                identity_refusals += 1;
            }
            assert_eq!(real, *expected); assert_eq!(real.index(), index as u32);
            assert_eq!(cx.services().source_parameter(real.source(), real.kind()), Some(real));
        }
        rows += 1;
    }
    eprintln!("actual typed source generics: {rows}rows/{constants}consts/{regions}regions; {identity_refusals}source identity refusals; actual generics_of and identity_for_item, no full facts or solver root");
    (rows, constants, regions)
}

#[cfg(test)]
pub(crate) fn check_actual_declarations(db: &mut Database, seal: &crate::db::seals::SealKey,
    root_unit: Digest, names: &[&str], include_methods: bool) -> Result<(usize, usize, usize), Diagnostic> {
    use crate::{core::digest::StableEncode, db::{dependency::InputKey, key::QueryKey, provider::SyntaxProvider},
        frontend::declaration_generic_types::{DeclarationGenericTypesKey, GenericTypesPending}};
    let input = db.input(&InputKey::DiscoverySeal(seal.clone())).ok_or_else(|| fault("generic control catalog absent"))?;
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else { return Err(fault("generic control catalog differs")); };
    let root = catalog.impl_catalog.as_ref().ok_or_else(|| fault("generic control needs actual global completion"))?.root();
    let config = match db.fetch(&SyntaxProvider, QueryKey::SolverConfiguration(Box::new(super::configuration::SolverConfigurationKey::from_source(root)))) {
        QueryPoll::Ready(Ok(value)) => value.value, _ => return Err(fault("actual generic source settings pending")),
    };
    let QueryValue::SolverConfiguration(config) = config.as_ref() else { return Err(fault("actual generic settings query differs")); };
    let inventory = match db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal.clone())) {
        QueryPoll::Ready(Ok(value)) => value.value, _ => return Err(fault("actual generic inventory pending")),
    };
    let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.as_ref() else { return Err(fault("actual generic inventory differs")); };
    let environment = inventory.inventory_root().environment().ok_or_else(|| fault("actual generic environment pending"))?;
    let mut definitions = BTreeMap::new();
    for source in environment.sources().iter().filter(|source| source.outline().root_scope.crate_key == root_unit) {
        for item in &source.outline().items {
            if names.contains(&item.def_key.declared_name.as_str()) { definitions.insert(item.def_key.declared_name.clone(), item.def_key.fingerprint()); }
        }
    }
    if definitions.len() != names.len() {
        for unit in environment.generated_publication_for_control().0 {
            for (id, key) in &unit.definitions {
                if key.crate_key == root_unit && names.contains(&key.declared_name.as_str()) {
                    assert!(definitions.insert(key.declared_name.clone(), *id).is_none(), "actual named source declaration is unique");
                }
            }
        }
    }
    assert_eq!(definitions.len(), names.len(), "all actual named upstream declarations found");
    if include_methods {
        let members = environment.member_index()?;
        let owner = definitions["ArrayWindowsExample"];
        let methods = members.trait_functions(owner, "next");
        assert_eq!(methods.len(), 1);
        definitions.insert("next".into(), methods[0].1);
        let method = members.trait_declaration(definitions["SliceExt"], "array_windows_example").ok_or_else(|| fault("actual late-bound source method absent"))?;
        let pending = match db.fetch(&SyntaxProvider, QueryKey::DeclarationGenericTypes(Box::new(DeclarationGenericTypesKey { inventory: seal.clone(), definition: method }))) {
            QueryPoll::Ready(Ok(value)) => value.value, _ => return Err(fault("actual late-bound generic source query missing")),
        };
        assert!(matches!(pending.as_ref(), QueryValue::DeclarationGenericTypes(types) if types.pending() == Some(GenericTypesPending::Parameters)),
            "written late lifetimes are never fabricated early parameters");
    }
    for (name, definition) in &definitions {
        let value = match db.fetch(&SyntaxProvider, QueryKey::DeclarationGenericTypes(Box::new(DeclarationGenericTypesKey { inventory: seal.clone(), definition: *definition }))) {
            QueryPoll::Ready(Ok(value)) => value.value, other => panic!("actual generic declaration {name}: {other:?}"),
        };
        let QueryValue::DeclarationGenericTypes(types) = value.as_ref() else { unreachable!() };
        assert_eq!(types.pending(), None, "actual generic declaration {name} pending; layout {:?}", types.declaration().layout());
    }
    let layouts = match SourceGenericLayouts::prepare_declarations(db, Arc::clone(&input), &definitions.values().copied().collect::<Vec<_>>()) {
        QueryPoll::Ready(Ok(layouts)) => layouts, QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("actual generic source facts need input")),
    };
    let before = db.trace.events.len();
    let result = config.with_source_context(db, input, Some(&layouts), |cx, _| check_materialized(cx, &layouts))?;
    assert_eq!(db.trace.events.len(), before, "typed generic materialization does not query source");
    Ok(result)
}
