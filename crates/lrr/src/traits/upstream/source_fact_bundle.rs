//! Demand-only retained actual source facts for one upstream context.
//! Query availability, candidate coverage and root admission are distinct.
use super::{
    identity::DefinitionId,
    prepared,
    source_adt_facts::{PreparedSourceAdtFacts, SourceAdtFactsKey},
    source_alias_layouts::PreparedSourceAliasLayouts,
    source_bindings::{DefinitionKind, SourceBindings, SourceRequirements, require_type},
    source_declaration_clauses::{PreparedSourceDeclarationClauses, PreparedSourceDeclarationClauseHierarchy},
    source_definition_headers::{DefinitionHeaderKey, DefinitionHeaderRole, PreparedDefinitionHeader, SourceHeaderParent},
    source_definition_semantics::PreparedSourceDefinitionSemantics,
    source_generics::SourceGenericLayouts,
    source_impl_outlives::PreparedSourceImplOutlives,
    source_impl_semantics::{ImplSemantic, PreparedSourceImplSemantics},
    source_lang_identity::PreparedSourceLangIdentity,
    source_predicates::PreparedImplSourcePredicates,
    source_terms::{PreparedImplSourceTypes, SourceTypePending},
    source_trait_clauses::{PreparedSourceTraitClauses, SourceTraitClauseFilter, SourceTraitClausesKey},
    source_trait_semantics::PreparedSourceTraitSemantics,
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    db::{dependency::{InputKey, InputValue}, key::QueryKey, memo::{Database, PreparedInputGuard},
        poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    frontend::{declaration_source_predicates::DeclarationSourcePredicatesKey,
        trait_associated_types::{PreparedTraitAssociatedTypes, TraitAssociatedTypesKey}},
    traits::{header_syntax::ImplHeaderSyntaxKey, term::TyTerm, trait_catalog::PreparedImplTraitCatalog},
};
use std::{collections::{BTreeMap, BTreeSet}, sync::Arc};

pub(super) struct SourceFactRequest {
    pub(super) caller: Digest,
    pub(super) callee: Digest,
    /// Actual candidate-read demands, not every trait mentioned by source metadata.
    pub(super) traits: BTreeSet<Digest>,
    pub(super) input_types: Vec<TyTerm>,
}
#[derive(Debug)]
pub(super) enum SourceFactBundlePending {
    CandidateNotDemanded(Digest),
    CandidateMembership(Digest),
    SourceGenerics { definition: Option<Digest>, pending: SourceTypePending },
    GenericLayout(Diagnostic),
}
struct RetainedQuery {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
pub(super) struct SourceDefinitionFacts {
    id: DefinitionId,
    header: Arc<QueryValue>,
    semantics: Arc<QueryValue>,
    language: Arc<QueryValue>,
    bindings: SourceBindings,
}
impl SourceDefinitionFacts {
    pub(super) fn id(&self) -> DefinitionId { self.id }
    pub(super) fn header(&self) -> &PreparedDefinitionHeader {
        let QueryValue::DefinitionHeaderPlan(plan) = self.header.as_ref() else { unreachable!("retained header") }; plan
    }
    pub(super) fn semantics(&self) -> &PreparedSourceDefinitionSemantics {
        let QueryValue::SourceDefinitionSemantics(plan) = self.semantics.as_ref() else { unreachable!("retained scalar facts") }; plan
    }
    pub(super) fn language(&self) -> &PreparedSourceLangIdentity {
        let QueryValue::SourceLangIdentity(plan) = self.language.as_ref() else { unreachable!("retained language identity") }; plan
    }
}
pub(super) struct SourceImplFacts {
    canonical: Digest,
    semantics: Arc<QueryValue>,
    types: Arc<QueryValue>,
    predicates: Arc<QueryValue>,
    outlives: Arc<QueryValue>,
    /// The impl's associated type values (leaves and `type_of` of its items).
    associated: Arc<QueryValue>,
    associated_bindings: SourceBindings,
}
impl SourceImplFacts {
    pub(super) fn canonical(&self) -> Digest { self.canonical }
    pub(super) fn semantics(&self) -> &PreparedSourceImplSemantics {
        let QueryValue::SourceImplSemantics(plan) = self.semantics.as_ref() else { unreachable!("retained impl semantics") }; plan
    }
    pub(super) fn types(&self) -> &PreparedImplSourceTypes {
        let QueryValue::ImplSourceTypes(plan) = self.types.as_ref() else { unreachable!("retained impl Types") }; plan
    }
    pub(super) fn predicates(&self) -> &PreparedImplSourcePredicates {
        let QueryValue::ImplSourcePredicates(plan) = self.predicates.as_ref() else { unreachable!("retained impl predicates") }; plan
    }
    pub(super) fn associated(&self) -> &super::source_impl_associated::PreparedSourceImplAssociated {
        let QueryValue::SourceImplAssociated(plan) = self.associated.as_ref() else { unreachable!("retained impl associated types") }; plan
    }
    pub(super) fn associated_bindings(&self) -> &SourceBindings { &self.associated_bindings }
    pub(super) fn outlives(&self) -> &PreparedSourceImplOutlives {
        let QueryValue::SourceImplOutlives(plan) = self.outlives.as_ref() else { unreachable!("retained impl outlives inputs") }; plan
    }
}
/// Immutable source plans with relevant input guards. No complete-facts boolean
/// exists: callers inspect the demanded query family's existing typed outcome.
pub(super) struct SourceFactBundle {
    request: SourceFactRequest,
    catalog: Arc<InputValue>,
    membership: Arc<QueryValue>,
    retained: BTreeMap<QueryKey, RetainedQuery>,
    input_bindings: SourceBindings,
    definitions: BTreeMap<Digest, SourceDefinitionFacts>,
    traits: BTreeMap<Digest, Arc<QueryValue>>,
    associated_types: BTreeMap<Digest, Arc<QueryValue>>,
    aliases: BTreeMap<Digest, Arc<QueryValue>>,
    clauses: BTreeMap<(Digest, SourceTraitClauseFilter), Arc<QueryValue>>,
    own_clauses: BTreeMap<Digest, Arc<QueryValue>>,
    hierarchies: BTreeMap<Digest, Arc<QueryValue>>,
    implementations: BTreeMap<Digest, SourceImplFacts>,
    adts: BTreeMap<Digest, Arc<QueryValue>>,
    /// Relating two applications of a nominal reads its variances.
    variances: BTreeMap<Digest, Arc<QueryValue>>,
}
macro_rules! ready {
    ($value:expr) => { match $value {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    } };
}
fn capture(db: &mut Database, retained: &mut BTreeMap<QueryKey, RetainedQuery>, key: QueryKey) -> QueryPoll<Arc<QueryValue>> {
    if let Some(query) = retained.get(&key) { return QueryPoll::Ready(Ok(Arc::clone(&query.value))); }
    let outcome = ready!(db.fetch(&SyntaxProvider, key.clone()));
    if !outcome.cacheable || !outcome.value.matches(&key) {
        return QueryPoll::Ready(Err(fault("source bundle query is not immutable or its key differs")));
    }
    let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &key, outcome.fingerprint));
    let value = Arc::clone(&outcome.value);
    retained.insert(key, RetainedQuery { value: outcome.value, guard });
    QueryPoll::Ready(Ok(value))
}
impl SourceFactBundle {
    pub(super) fn prepare(db: &mut Database, catalog: Arc<InputValue>, mut request: SourceFactRequest) -> QueryPoll<Self> {
        let InputValue::DiscoverySeal(publication) = catalog.as_ref() else {
            return QueryPoll::Ready(Err(fault("source bundle catalog input differs")));
        };
        let world = match prepared::catalog_world(publication) {
            Ok(world) => world, Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let inventory = publication.seal.key();
        let local = publication.impl_catalog.as_ref().expect("validated complete catalog").root().outline.crate_key;
        if !matches!(db.input(&InputKey::DiscoverySeal(inventory.clone())), Some(current) if Arc::ptr_eq(&current, &catalog)) {
            return QueryPoll::Ready(Err(fault("source bundle catalog is not the actual current publication")));
        }
        let mut retained = BTreeMap::new();
        let membership = ready!(capture(db, &mut retained, QueryKey::ImplTraitCatalog(inventory.clone())));
        if !matches!(membership.as_ref(), QueryValue::ImplTraitCatalog(plan) if plan.key()==&inventory) {
            return QueryPoll::Ready(Err(fault("source bundle candidate catalog query differs")));
        }
        let QueryValue::ImplTraitCatalog(candidate_catalog) = membership.as_ref() else { unreachable!("validated catalog query") };
        if candidate_catalog.pending().is_empty() && !candidate_catalog.matches_catalog(publication) {
            return QueryPoll::Ready(Err(fault("source bundle membership belongs to another complete catalog")));
        }
        let mut required = SourceRequirements::new();
        for ty in &request.input_types { require_type(&mut required, ty); }
        let input_bindings = ready!(SourceBindings::prepare(db, Arc::clone(&catalog), required));
        // Pinned normalization first probes the projected item's actual parent
        // trait, including its complete relevant-impl bucket. A declaration
        // identity alone cannot certify that bucket, even when it is empty.
        // These keys came from the demanded input Types' ordinary Q77 join;
        // do not promote traits from unrelated metadata into candidate demands.
        request.traits.extend(input_bindings.projections.keys().map(|(parent, _)| *parent));
        let mut pending_definitions = BTreeSet::from([request.caller, request.callee]);
        pending_definitions.extend(request.traits.iter().copied());
        pending_definitions.extend(input_bindings.definitions.keys().copied());
        let mut result = Self { request, catalog, membership, retained, input_bindings,
            definitions: BTreeMap::new(), traits: BTreeMap::new(), associated_types: BTreeMap::new(),
            aliases: BTreeMap::new(), clauses: BTreeMap::new(),
            own_clauses: BTreeMap::new(), hierarchies: BTreeMap::new(), implementations: BTreeMap::new(), adts: BTreeMap::new(),
            variances: BTreeMap::new() };
        // Membership supplies every candidate. Type/where-clause availability
        // never deletes an actual member of a demanded bucket.
        for trait_definition in result.request.traits.iter().copied().collect::<Vec<_>>() {
            let candidates = result.candidate_catalog().candidates(trait_definition).map(|rows| rows.to_vec());
            let Some(candidates) = candidates else { continue; };
            for (raw, canonical) in candidates {
                if let Some(previous) = result.implementations.get(&raw) {
                    if previous.canonical != canonical { return QueryPoll::Ready(Err(fault("actual candidate canonical identity changed"))); }
                    continue;
                }
                let key = ImplHeaderSyntaxKey { inventory: inventory.clone(), implementation: raw };
                let semantics = ready!(capture(db, &mut result.retained, QueryKey::SourceImplSemantics(Box::new(key.clone()))));
                let types = ready!(capture(db, &mut result.retained, QueryKey::ImplSourceTypes(Box::new(key.clone()))));
                let predicates = ready!(capture(db, &mut result.retained, QueryKey::ImplSourcePredicates(Box::new(key.clone()))));
                let outlives = ready!(capture(db, &mut result.retained, QueryKey::SourceImplOutlives(Box::new(key.clone()))));
                let associated = ready!(capture(db, &mut result.retained, QueryKey::SourceImplAssociated(Box::new(key.clone()))));
                let mut associated_bindings = SourceBindings::empty(Arc::clone(&result.catalog));
                if let QueryValue::SourceImplAssociated(plan) = associated.as_ref()
                    && let Ok(rows) = plan.types()
                {
                    let mut required = SourceRequirements::new();
                    for row in rows {
                        require_type(&mut required, &row.target);
                        pending_definitions.insert(row.alias);
                    }
                    associated_bindings = ready!(SourceBindings::prepare(db, Arc::clone(&result.catalog), required));
                    pending_definitions.extend(associated_bindings.definitions.keys().copied());
                }
                let facts = SourceImplFacts { canonical, semantics, types, predicates, outlives, associated, associated_bindings };
                if facts.semantics().key() != &key || facts.types().key() != &key
                    || facts.predicates().key() != &key || facts.outlives().key() != &key {
                    return QueryPoll::Ready(Err(fault("source bundle impl query identity differs")));
                }
                if let ImplSemantic::Known(actual) = facts.semantics().impl_definition()
                    && actual != canonical {
                    return QueryPoll::Ready(Err(fault("source bundle impl canonical identity differs from actual membership")));
                }
                pending_definitions.insert(canonical);
                pending_definitions.extend(facts.types().bindings().definitions.keys().copied());
                pending_definitions.extend(facts.predicates().bindings().definitions.keys().copied());
                if let Ok(bindings) = facts.semantics().full_clause_bindings() {
                    pending_definitions.extend(bindings.definitions.keys().copied());
                }
                // Q89 retains its exact implied-query inputs. Keeping those
                // source keys is not a claim about their elaborated values.
                for plan in facts.outlives().trait_plans() {
                    pending_definitions.insert(plan.key().definition);
                }
                result.implementations.insert(raw, facts);
            }
        }
        let mut pending_clauses = BTreeSet::new();
        loop {
            if let Some(definition) = pending_definitions.pop_first() {
                if result.definitions.contains_key(&definition) { continue; }
                let key = DefinitionHeaderKey { inventory: inventory.clone(), definition };
                let header = ready!(capture(db, &mut result.retained, QueryKey::DefinitionHeaderPlan(Box::new(key.clone()))));
                let semantics = ready!(capture(db, &mut result.retained, QueryKey::SourceDefinitionSemantics(Box::new(key.clone()))));
                let language = ready!(capture(db, &mut result.retained, QueryKey::SourceLangIdentity(Box::new(key.clone()))));
                let QueryValue::DefinitionHeaderPlan(plan) = header.as_ref() else { return QueryPoll::Ready(Err(fault("source bundle header query differs"))); };
                if plan.key() != &key { return QueryPoll::Ready(Err(fault("source bundle header owner differs"))); }
                let role = plan.role();
                let id = plan.receipt().definition_id(world, local);
                if let SourceHeaderParent::Associated(parent) = plan.source_parent() { pending_definitions.insert(*parent); }
                let bindings = ready!(SourceBindings::prepare(db, Arc::clone(&result.catalog), BTreeMap::from([(definition, DefinitionKind::Declaration)])));
                pending_definitions.extend(bindings.definitions.keys().copied());
                if role == DefinitionHeaderRole::Trait {
                    let traits = ready!(capture(db, &mut result.retained, QueryKey::SourceTraitSemantics(Box::new(key.clone()))));
                    result.traits.insert(definition, traits);
                    let associated = ready!(capture(db, &mut result.retained, QueryKey::TraitAssociatedTypes(Box::new(TraitAssociatedTypesKey {
                        inventory: inventory.clone(), trait_definition: definition,
                    }))));
                    let QueryValue::TraitAssociatedTypes(plan) = associated.as_ref() else {
                        return QueryPoll::Ready(Err(fault("source bundle associated type query differs")));
                    };
                    if let Ok(members) = plan.definitions() {
                        pending_definitions.extend(members);
                    }
                    result.associated_types.insert(definition, associated);
                    pending_clauses.insert((definition, SourceTraitClauseFilter::Super));
                    pending_clauses.insert((definition, SourceTraitClauseFilter::Implied));
                }
                if matches!(role, DefinitionHeaderRole::TypeAlias | DefinitionHeaderRole::Constant) {
                    let alias = ready!(capture(db, &mut result.retained, QueryKey::SourceAliasLayouts(Box::new(key.clone()))));
                    result.aliases.insert(definition, alias);
                }
                if matches!(role, DefinitionHeaderRole::Struct | DefinitionHeaderRole::Enum | DefinitionHeaderRole::Union) {
                    let adt = ready!(capture(db, &mut result.retained, QueryKey::SourceAdtFacts(Box::new(SourceAdtFactsKey {
                        inventory: inventory.clone(), definition, fields: false,
                    }))));
                    result.adts.insert(definition, adt);
                    let variance = ready!(capture(db, &mut result.retained, QueryKey::SourceVarianceQueries(Box::new(key.clone()))));
                    result.variances.insert(definition, variance);
                }
                // Projection normalization through an impl adds the trait's
                // associated type's own clauses (`normalizes_to.rs`
                // `own_clauses_of(alias_def_id)`): an associated type of a
                // trait is read exactly when its alias is normalized.
                let trait_associated = role == DefinitionHeaderRole::TypeAlias
                    && match plan.source_parent() {
                        SourceHeaderParent::Associated(parent) => {
                            let parent_key = DefinitionHeaderKey { inventory: inventory.clone(), definition: *parent };
                            let parent_header = ready!(capture(db, &mut result.retained, QueryKey::DefinitionHeaderPlan(Box::new(parent_key))));
                            matches!(parent_header.as_ref(), QueryValue::DefinitionHeaderPlan(parent) if parent.role() == DefinitionHeaderRole::Trait)
                        }
                        _ => false,
                    };
                if trait_associated && !result.own_clauses.contains_key(&definition) {
                    let clause_key = DeclarationSourcePredicatesKey { inventory: inventory.clone(), definition };
                    let own = ready!(capture(db, &mut result.retained, QueryKey::SourceDeclarationClauses(Box::new(clause_key))));
                    if let QueryValue::SourceDeclarationClauses(plan) = own.as_ref()
                        && let Ok(bindings) = plan.bindings()
                    {
                        pending_definitions.extend(bindings.definitions.keys().copied());
                    }
                    result.own_clauses.insert(definition, own);
                }
                // Only the actual caller/callee own/parent clause demand is
                // seeded here; other definitions retain independent domains.
                if definition == result.request.caller || definition == result.request.callee {
                    let mut owners = BTreeSet::from([definition]);
                    while let Some(owner) = owners.pop_first() {
                        if result.own_clauses.contains_key(&owner) { continue; }
                        let clause_key = DeclarationSourcePredicatesKey { inventory: inventory.clone(), definition: owner };
                        let own = ready!(capture(db, &mut result.retained, QueryKey::SourceDeclarationClauses(Box::new(clause_key))));
                        let QueryValue::SourceDeclarationClauses(plan) = own.as_ref() else { return QueryPoll::Ready(Err(fault("source bundle own clause query differs"))); };
                        if let Ok(bindings) = plan.bindings() {
                            pending_definitions.extend(bindings.definitions.keys().copied());
                        }
                        if let Some(parent) = plan.source().parent_definition() {
                            owners.insert(parent);
                            pending_definitions.insert(parent);
                        }
                        result.own_clauses.insert(owner, own);
                    }
                    let clause_key = DeclarationSourcePredicatesKey { inventory: inventory.clone(), definition };
                    let hierarchy = ready!(capture(db, &mut result.retained, QueryKey::SourceDeclarationClauseHierarchy(Box::new(clause_key))));
                    result.hierarchies.insert(definition, hierarchy);
                }
                result.definitions.insert(definition, SourceDefinitionFacts { id, header, semantics, language, bindings });
            } else if let Some((definition, filter)) = pending_clauses.pop_first() {
                if result.clauses.contains_key(&(definition, filter)) { continue; }
                let key = SourceTraitClausesKey { inventory: inventory.clone(), definition, filter };
                let value = ready!(capture(db, &mut result.retained, QueryKey::SourceTraitClauses(Box::new(key.clone()))));
                let QueryValue::SourceTraitClauses(plan) = value.as_ref() else { return QueryPoll::Ready(Err(fault("source bundle trait clause query differs"))); };
                if plan.key() != &key { return QueryPoll::Ready(Err(fault("source bundle trait clause owner differs"))); }
                pending_definitions.extend(plan.bindings().definitions.keys().copied());
                for dependency in plan.positive_dependencies() {
                    pending_definitions.insert(dependency.definition);
                    pending_clauses.insert((dependency.definition, dependency.filter));
                }
                result.clauses.insert((definition, filter), value);
            } else { break; }
        }
        // Bindings made outside a QueryValue retain exact memo guards for
        // generic Types and associated-declaration lookup (including absence,
        // blockers and expansion identity), not just their retained values.
        let mut source_roots = BTreeMap::<QueryKey, Arc<QueryValue>>::new();
        for bindings in result.bindings() {
            for value in bindings.generics.values().chain(bindings.projections.values().map(|binding| &binding.query)) {
                let key = match value.as_ref() {
                    QueryValue::DeclarationGenericTypes(plan) => QueryKey::DeclarationGenericTypes(Box::new(plan.key().clone())),
                    QueryValue::TraitAssociatedTypes(plan) => QueryKey::TraitAssociatedTypes(Box::new(plan.key().clone())),
                    _ => unreachable!("actual source binding query"),
                };
                if let Some(previous) = source_roots.insert(key, Arc::clone(value))
                    && !Arc::ptr_eq(&previous, value) {
                    return QueryPoll::Ready(Err(fault("source bundle binding roots changed during preparation")));
                }
            }
        }
        for (key, expected) in source_roots {
            let current = ready!(capture(db, &mut result.retained, key));
            if !Arc::ptr_eq(&current, &expected) {
                return QueryPoll::Ready(Err(fault("source bundle binding memo identity differs")));
            }
        }
        QueryPoll::Ready(Ok(result))
    }
    pub(super) fn request(&self) -> &SourceFactRequest { &self.request }
    pub(super) fn catalog_input(&self) -> &Arc<InputValue> { &self.catalog }
    pub(super) fn membership_input(&self) -> &Arc<QueryValue> { &self.membership }
    pub(super) fn candidate_catalog(&self) -> &PreparedImplTraitCatalog {
        let QueryValue::ImplTraitCatalog(plan) = self.membership.as_ref() else { unreachable!("retained membership catalog") }; plan
    }
    pub(super) fn candidates(&self, definition: Digest) -> Result<&[(Digest, Digest)], SourceFactBundlePending> {
        if !self.request.traits.contains(&definition) { return Err(SourceFactBundlePending::CandidateNotDemanded(definition)); }
        self.candidate_catalog().candidates(definition).ok_or(SourceFactBundlePending::CandidateMembership(definition))
    }
    pub(super) fn definitions(&self) -> &BTreeMap<Digest, SourceDefinitionFacts> { &self.definitions }
    pub(super) fn definition_id(&self, definition: Digest) -> Option<DefinitionId> { self.definitions.get(&definition).map(SourceDefinitionFacts::id) }
    pub(super) fn implementations(&self) -> &BTreeMap<Digest, SourceImplFacts> { &self.implementations }
    pub(super) fn traits(&self) -> impl Iterator<Item = (Digest, &PreparedSourceTraitSemantics)> {
        self.traits.iter().map(|(definition, value)| {
            let QueryValue::SourceTraitSemantics(plan) = value.as_ref() else { unreachable!("retained trait flags") }; (*definition, plan.as_ref())
        })
    }
    pub(super) fn trait_associated_types(&self, definition: Digest) -> Option<&PreparedTraitAssociatedTypes> {
        let QueryValue::TraitAssociatedTypes(plan) = self.associated_types.get(&definition)?.as_ref() else { unreachable!("retained associated type identities") };
        Some(plan)
    }
    pub(super) fn alias_layouts(&self, definition: Digest) -> Option<&PreparedSourceAliasLayouts> {
        let QueryValue::SourceAliasLayouts(plan) = self.aliases.get(&definition)?.as_ref() else { unreachable!("retained alias argument layouts") };
        Some(plan)
    }
    pub(super) fn trait_clauses(&self) -> impl Iterator<Item = (&(Digest, SourceTraitClauseFilter), &PreparedSourceTraitClauses)> {
        self.clauses.iter().map(|(key, value)| {
            let QueryValue::SourceTraitClauses(plan) = value.as_ref() else { unreachable!("retained filtered clauses") }; (key, plan.as_ref())
        })
    }
    pub(super) fn own_clauses(&self, definition: Digest) -> Option<&PreparedSourceDeclarationClauses> {
        let QueryValue::SourceDeclarationClauses(plan) = self.own_clauses.get(&definition)?.as_ref() else { unreachable!("retained own clauses") }; Some(plan)
    }
    pub(super) fn hierarchy(&self, definition: Digest) -> Option<&PreparedSourceDeclarationClauseHierarchy> {
        let QueryValue::SourceDeclarationClauseHierarchy(plan) = self.hierarchies.get(&definition)?.as_ref() else { unreachable!("retained clause hierarchy") }; Some(plan)
    }
    pub(super) fn variance_queries(&self, definition: Digest) -> Option<&super::source_variance_queries::PreparedSourceVarianceQueries> {
        let QueryValue::SourceVarianceQueries(plan) = self.variances.get(&definition)?.as_ref() else { unreachable!("retained variance queries") };
        Some(plan)
    }
    pub(super) fn adts(&self) -> impl Iterator<Item = (Digest, &PreparedSourceAdtFacts)> {
        self.adts.iter().map(|(definition, value)| {
            let QueryValue::SourceAdtFacts(plan) = value.as_ref() else { unreachable!("retained ADT declaration") }; (*definition, plan.as_ref())
        })
    }
    fn bindings(&self) -> Vec<&SourceBindings> {
        let mut bindings = vec![&self.input_bindings];
        bindings.extend(self.definitions.values().map(|facts| &facts.bindings));
        for facts in self.implementations.values() {
            bindings.push(facts.types().bindings());
            bindings.push(facts.predicates().bindings());
            bindings.push(facts.associated_bindings());
            if let Ok(actual) = facts.semantics().full_clause_bindings() {
                bindings.push(actual);
            }
        }
        for value in self.clauses.values() {
            let QueryValue::SourceTraitClauses(plan) = value.as_ref() else { unreachable!("retained trait clauses") };
            bindings.push(plan.bindings());
        }
        for value in self.own_clauses.values() {
            let QueryValue::SourceDeclarationClauses(plan) = value.as_ref() else { unreachable!("retained own clauses") };
            if let Ok(actual) = plan.bindings() { bindings.push(actual); }
        }
        bindings
    }
    /// This proves argument layouts only. Pending clause/boolean/ADT fields
    /// remain unavailable in their individual retained plans.
    pub(super) fn generic_layouts(&self) -> Result<SourceGenericLayouts, SourceFactBundlePending> {
        let bindings = self.bindings();
        for actual in &bindings {
            if let Some(pending) = actual.pending {
                return Err(SourceFactBundlePending::SourceGenerics { definition: None, pending });
            }
        }
        SourceGenericLayouts::from_bindings(Arc::clone(&self.catalog), bindings).map_err(SourceFactBundlePending::GenericLayout)
    }
    pub(super) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        for query in self.retained.values() { query.guard.require_current(db)?; }
        let InputValue::DiscoverySeal(catalog) = self.catalog.as_ref() else { unreachable!("validated catalog") };
        if !db.published_input(&InputKey::DiscoverySeal(catalog.seal.key()))
            .is_some_and(|(_, fingerprint)| fingerprint == self.catalog.fingerprint()) {
            return Err(fault("source bundle catalog changed"));
        }
        Ok(())
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into()) }
