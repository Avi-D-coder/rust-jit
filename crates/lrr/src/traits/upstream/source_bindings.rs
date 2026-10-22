//! Demanded source-definition/generic-Type closure, shared by IR construction.
use super::{identity::DefinitionId, prepared::{self, DefinitionReceipt}, source_terms::SourceTypePending};
use crate::{core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{dependency::InputValue, key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    frontend::{declaration_generic_types::DeclarationGenericTypesKey,
        trait_associated_types::TraitAssociatedTypesKey},
    traits::term::{TyTerm, GenericArg}};
use std::{collections::{BTreeMap, BTreeSet}, sync::Arc};
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum DefinitionKind { Nominal, Trait, Impl, Declaration }
/// Actual declaration demands and still-symbolic associated member demands
/// occupy different indexes. A legacy member name key is never a DefId.
#[derive(Default, Clone)]
pub(super) struct SourceRequirements {
    definitions: BTreeMap<Digest, DefinitionKind>,
    projections: BTreeSet<(Digest, Digest)>,
}
impl SourceRequirements {
    pub(super) fn new() -> Self { Self::default() }
    pub(super) fn insert(&mut self, definition: Digest, kind: DefinitionKind) {
        self.definitions.insert(definition, kind);
    }
    /// Whether every demand of `other` is also a demand of `self`.
    pub(super) fn covers(&self, other: &Self) -> bool {
        other
            .definitions
            .iter()
            .all(|(definition, kind)| self.definitions.get(definition) == Some(kind))
            && other.projections.is_subset(&self.projections)
    }
}
impl From<BTreeMap<Digest, DefinitionKind>> for SourceRequirements {
    fn from(definitions: BTreeMap<Digest, DefinitionKind>) -> Self {
        Self { definitions, projections: BTreeSet::new() }
    }
}
impl FromIterator<(Digest, DefinitionKind)> for SourceRequirements {
    fn from_iter<T: IntoIterator<Item = (Digest, DefinitionKind)>>(iter: T) -> Self {
        Self::from(iter.into_iter().collect::<BTreeMap<_, _>>())
    }
}
#[derive(Debug, Clone)]
pub(super) struct SourceProjectionBinding {
    pub(super) trait_definition: DefinitionId,
    pub(super) associated_definition: DefinitionId,
    pub(super) query: Arc<QueryValue>,
}
#[derive(Clone, Copy)]
pub(super) struct SourceTypeBindings<'a> {
    pub(super) definitions: &'a BTreeMap<Digest, DefinitionId>,
    pub(super) projections: &'a BTreeMap<(Digest, Digest), SourceProjectionBinding>,
}
#[derive(Debug)]
pub(super) struct SourceBindings {
    pub(super) catalog: Arc<InputValue>,
    pub(super) definitions: BTreeMap<Digest, DefinitionId>,
    pub(super) generics: BTreeMap<DefinitionId, Arc<QueryValue>>,
    pub(super) projections: BTreeMap<(Digest, Digest), SourceProjectionBinding>,
    receipts: Vec<DefinitionReceipt>,
    pub(super) pending: Option<SourceTypePending>,
}
impl SourceBindings {
    pub(super) fn type_bindings(&self) -> SourceTypeBindings<'_> {
        SourceTypeBindings { definitions: &self.definitions, projections: &self.projections }
    }
    pub(super) fn empty(catalog: Arc<InputValue>) -> Self {
        Self { catalog, definitions: BTreeMap::new(), generics: BTreeMap::new(),
            projections: BTreeMap::new(), receipts: Vec::new(), pending: None }
    }
    pub(super) fn prepare(db: &mut Database, catalog_input: Arc<InputValue>, required: impl Into<SourceRequirements>) -> QueryPoll<Self> {
        macro_rules! ready { ($value:expr) => { match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        } }; }
        let InputValue::DiscoverySeal(catalog) = catalog_input.as_ref() else { return QueryPoll::Ready(Err(fault("source binding catalog differs"))); };
        let world = match prepared::catalog_world(catalog) { Ok(world) => world, Err(error) => return QueryPoll::Ready(Err(error)) };
        let local = catalog.impl_catalog.as_ref().expect("checked source catalog").root().outline.crate_key;
        let key = catalog.seal.key();
        let mut result = Self::empty(Arc::clone(&catalog_input));
        let mut required = required.into();
        let mut projection_sources = BTreeMap::new();
        loop {
          if let Some((definition, kind)) = required.definitions.pop_first() {
            if result.definitions.contains_key(&definition) { continue; }
            let Some(receipt) = ready!(DefinitionReceipt::observe(db, definition)) else {
                result.pending = Some(SourceTypePending::SourceDefinition); return QueryPoll::Ready(Ok(result));
            };
            let valid = match kind { DefinitionKind::Trait => receipt.is_trait(), DefinitionKind::Impl => receipt.is_impl(),
                DefinitionKind::Declaration => true, DefinitionKind::Nominal => match receipt.is_nominal() {
                    Ok(valid) => valid, Err(error) => return QueryPoll::Ready(Err(error)),
                } };
            if !valid { result.pending = Some(SourceTypePending::SourceDefinition); return QueryPoll::Ready(Ok(result)); }
            let generics = ready!(db.fetch(&SyntaxProvider, QueryKey::DeclarationGenericTypes(Box::new(DeclarationGenericTypesKey {
                inventory: key.clone(), definition,
            })))).value;
            let QueryValue::DeclarationGenericTypes(actual) = generics.as_ref() else { return QueryPoll::Ready(Err(fault("source generic Type query differs"))); };
            if !receipt.matches_generics(actual.declaration()) || actual.pending().is_some() {
                result.pending = Some(if actual.pending() == Some(crate::frontend::declaration_generic_types::GenericTypesPending::Parameters) {
                    SourceTypePending::ParameterLayout
                } else { SourceTypePending::ConstTypeMetadata }); return QueryPoll::Ready(Ok(result));
            }
            let id = receipt.definition_id(world, local);
            result.definitions.insert(definition, id);
            for ty in actual.const_types().values() { require_type(&mut required, ty); }
            result.generics.insert(id, generics); result.receipts.push(receipt);
          } else if let Some((trait_definition, item)) = required.projections.pop_first() {
            if projection_sources.contains_key(&(trait_definition, item)) { continue; }
            let query = ready!(db.fetch(&SyntaxProvider, QueryKey::TraitAssociatedTypes(Box::new(TraitAssociatedTypesKey {
                inventory: key.clone(), trait_definition,
            })))).value;
            let QueryValue::TraitAssociatedTypes(plan) = query.as_ref() else {
                return QueryPoll::Ready(Err(fault("projection associated source query differs")));
            };
            if plan.key().inventory != key || plan.key().trait_definition != trait_definition {
                return QueryPoll::Ready(Err(fault("projection associated source owner differs")));
            }
            let associated = match plan.lookup(item) {
                Ok(associated) => associated,
                Err(_) => { result.pending = Some(SourceTypePending::ProjectionMetadata); return QueryPoll::Ready(Ok(result)); }
            };
            required.insert(trait_definition, DefinitionKind::Trait);
            required.insert(associated, DefinitionKind::Declaration);
            projection_sources.insert((trait_definition, item), (associated, query));
          } else { break; }
        }
        for ((trait_source, item), (associated_source, query)) in projection_sources {
            let (Some(&trait_definition), Some(&associated_definition)) =
                (result.definitions.get(&trait_source), result.definitions.get(&associated_source)) else {
                return QueryPoll::Ready(Err(fault("resolved projection declaration source binding absent")));
            };
            let (QueryValue::DeclarationGenericTypes(parent), QueryValue::DeclarationGenericTypes(associated)) =
                (result.generics[&trait_definition].as_ref(), result.generics[&associated_definition].as_ref()) else {
                return QueryPoll::Ready(Err(fault("projection parent generic source queries differ")));
            };
            if !projection_parent_matches(parent.declaration(), associated.declaration()) {
                result.pending = Some(SourceTypePending::ProjectionMetadata); return QueryPoll::Ready(Ok(result));
            }
            result.projections.insert((trait_source, item), SourceProjectionBinding { trait_definition, associated_definition, query });
        }
        QueryPoll::Ready(Ok(result))
    }
}
pub(super) fn require_type(required: &mut SourceRequirements, ty: &TyTerm) {
    let mut terms = vec![ty];
    while let Some(ty) = terms.pop() {
        if let Some((_, pointee)) = crate::traits::pointer_terms::as_raw_pointer(ty) { terms.push(pointee); continue; }
        match ty {
            TyTerm::Nominal(id, args) => { required.insert(*id, DefinitionKind::Nominal);
                terms.extend(args.iter().filter_map(|arg| if let GenericArg::Type(ty) = arg { Some(ty) } else { None })); },
            TyTerm::Tuple(fields) => terms.extend(fields),
            TyTerm::Array(ty, _) | TyTerm::Slice(ty) | TyTerm::Ref(_, _, ty) => terms.push(ty),
            TyTerm::Function(signature) => { terms.extend(&signature.value.parameters); terms.push(&signature.value.result); },
            TyTerm::Projection(projection) => {
                required.insert(projection.trait_ref.trait_key, DefinitionKind::Trait);
                required.projections.insert((projection.trait_ref.trait_key, projection.item));
                terms.push(&projection.trait_ref.self_ty);
                terms.extend(projection.arguments().filter_map(|argument|
                    if let GenericArg::Type(ty) = argument { Some(ty) } else { None }));
            }
            _ => {},
        }
    }
}
/// Validate the genuine associated declaration's captured parent-first binder
/// against its actual owning trait. This is source metadata, not a trait goal.
pub(super) fn projection_parent_matches(
    parent: &crate::frontend::declaration_generics::DeclarationGenerics,
    associated: &crate::frontend::declaration_generics::DeclarationGenerics,
) -> bool {
    let (Some(parent_layout), Some(layout)) = (parent.layout(), associated.layout()) else { return false; };
    let (Some((_, parent_header, _, _)), Some((_, header, context, parsed))) =
        (parent.source_parts(), associated.source_parts()) else { return false; };
    if !parent.has_trait_self() || associated.has_trait_self() || associated.is_function()
        || parent_layout.inherited != 0 || parent_layout.implicit_self
        || !layout.implicit_self || layout.inherited as usize != parent.parameters().len()
        || header.context != parent_header.tokens {
        return false;
    }
    use ra_ap_syntax::{AstNode, ast};
    if !matches!(parsed.syntax_at(context.range, |node| ast::TypeAlias::can_cast(node.kind())), Ok(Some(_))) {
        return false;
    }
    parent.parameters().iter().enumerate().all(|(index, parameter)| {
        associated.parameters().get(layout.own as usize + index).is_some_and(|child|
            child.kind == parameter.kind && child.name == parameter.name
                && child.ordinal == layout.own + parameter.ordinal
                && child.bounds == parameter.bounds && child.const_type == parameter.const_type)
    })
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into()) }
impl StableEncode for SourceBindings {
    const DOMAIN: &'static str = "lrr/v2/demanded-source-type-bindings";
    fn encode_fields(&self, e: &mut StableEncoder) {
        let InputValue::DiscoverySeal(catalog) = self.catalog.as_ref() else { unreachable!() };
        e.digest(catalog.seal.fingerprint());
        e.map(&self.definitions, |e, definition, id| { e.digest(*definition); e.record(id); });
        e.map(&self.generics, |e, id, value| { e.record(id); let QueryValue::DeclarationGenericTypes(generics) = value.as_ref() else { unreachable!() }; e.record(generics.as_ref()); });
        e.map(&self.projections, |e, (owner, item), binding| {
            e.digest(*owner); e.digest(*item); e.record(&binding.trait_definition); e.record(&binding.associated_definition);
            let QueryValue::TraitAssociatedTypes(plan) = binding.query.as_ref() else { unreachable!("captured projection member query") };
            e.record(plan.as_ref());
        });
        e.option(&self.pending, |e, pending| e.u32(*pending as u32));
    }
}
