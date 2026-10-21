//! Materialize actual retained source queries in their one owning IR context.
//! This supplies individual query values, never a root-readiness certificate.
use super::{
    configuration::PreparedSolverConfiguration,
    identity::DefinitionId,
    lang_names,
    model::{AllocateList, Clause, List, Symbol},
    prepared::{self, AdtFacts, Cx, DefinitionFacts, ImplFacts, LangItems, PreparedFacts, TraitFacts},
    query_facts::QueryFact,
    source_definition_headers::DefinitionHeaderRole,
    source_definition_semantics::{DefinitionBooleanQuery as D, SourceSemantic},
    source_fact_bundle::SourceFactBundle,
    source_generics::SourceGenericLayouts,
    source_impl_semantics::ImplSemantic,
    source_trait_clauses::SourceTraitClauseFilter,
    source_trait_semantics::{TraitBooleanQuery as T, TraitSemantic},
};
use crate::core::{diagnostic::Diagnostic, digest::{Digest, StableEncode}};
use rustc_type_ir::{self as ty, inherent::*, EarlyBinder, Interner};
use rustc_type_ir::lang_items::{SolverAdtLangItem, SolverProjectionLangItem, SolverTraitLangItem};
use std::collections::BTreeMap;

/// An unavailable query retains its owner and reason. It is not NoSolution,
/// and does not make available neighboring query values disappear.
#[derive(Debug)]
pub(super) struct UnpreparedSourceQuery {
    pub(super) definition: Digest,
    pub(super) query: &'static str,
    pub(super) reason: String,
}
pub(super) struct MaterializedSourceFacts<'cx> {
    pub(super) facts: PreparedFacts<'cx>,
    /// Encoding holds only, not a census of every unprepared compiler query.
    /// Root admission must inspect its demanded actual QueryFact slots.
    pub(super) unprepared: Vec<UnpreparedSourceQuery>,
}
type C<'cx> = Clause<'cx, prepared::PreparedProvider<'cx>>;

fn scalar<V>(value: SourceSemantic<V>) -> QueryFact<V> {
    match value {
        SourceSemantic::Known(value) => QueryFact::Known(value),
        SourceSemantic::NotApplicable => QueryFact::NotApplicable,
        SourceSemantic::Pending(_) => QueryFact::Unprepared,
    }
}
fn trait_scalar<V>(value: TraitSemantic<V>) -> QueryFact<V> {
    match value {
        TraitSemantic::Known(value) => QueryFact::Known(value),
        TraitSemantic::NotApplicable => QueryFact::NotApplicable,
        TraitSemantic::Pending(_) => QueryFact::Unprepared,
    }
}
fn impl_scalar<V>(value: ImplSemantic<V>) -> QueryFact<V> {
    match value {
        ImplSemantic::Known(value) => QueryFact::Known(value),
        ImplSemantic::NotApplicable => QueryFact::NotApplicable,
        ImplSemantic::Pending(_) => QueryFact::Unprepared,
    }
}
fn hold(
    holds: &mut Vec<UnpreparedSourceQuery>,
    definition: Digest,
    query: &'static str,
    reason: impl std::fmt::Debug,
) {
    holds.push(UnpreparedSourceQuery { definition, query, reason: format!("{reason:?}") });
}
fn spanned<'cx>(cx: Cx<'cx>, clauses: Vec<(C<'cx>, super::model::Span)>)
    -> EarlyBinder<Cx<'cx>, &'cx [(C<'cx>, super::model::Span)]>
{
    let retained: List<'cx, (C<'cx>, super::model::Span)> = cx.list(clauses);
    EarlyBinder::bind(cx, retained).map_bound(|list| list.items())
}

/// Only pure encoding and pinned pure elaboration occur here. The source bundle
/// and configuration must have been prepared/currentness-checked outside the
/// compiler callback. Every returned arena value belongs to this exact Cx.
pub(super) fn materialize<'cx>(
    cx: Cx<'cx>,
    bundle: &SourceFactBundle,
    layouts: &SourceGenericLayouts,
    configuration: &PreparedSolverConfiguration,
) -> Result<MaterializedSourceFacts<'cx>, Diagnostic> {
    if !layouts.matches_catalog(bundle.catalog_input())
        || cx.services().source_configuration() != configuration.fingerprint()
    {
        return Err(fault("source factory generic/configuration owner differs"));
    }
    let mut definitions = BTreeMap::new();
    let mut unprepared = Vec::new();
    for (&source, actual) in bundle.definitions() {
        let header = actual.header();
        // Metadata is unconditional in DefinitionFacts. Refuse materialization
        // rather than invent a declaration or a generic count if it is absent.
        let encoded = header.encode_header(cx).map_err(|why| {
            fault(&format!("source factory header {source:?} is unavailable: {why:?}"))
        })?;
        if encoded.definition != actual.id()
            || layouts.get(actual.id()).is_none_or(|layout| !header.receipt().matches_generics(layout))
        {
            return Err(fault("source factory declaration/generic receipt differs"));
        }
        let name = match (encoded.name, encoded.role) {
            (Some(name), _) => name,
            (None, DefinitionHeaderRole::Impl) => <Symbol<'cx> as rustc_type_ir::inherent::Symbol<Cx<'cx>>>::SYM_ANON,
            _ => return Err(fault("source factory named declaration has no actual name")),
        };
        let mut row = DefinitionFacts::metadata(
            header.receipt().clone(), name, encoded.span, encoded.generics,
        );
        let semantic = actual.semantics();
        row.requires_sized_self = scalar(semantic.boolean(D::RequiresSizedSelf));
        row.constant_function = scalar(semantic.boolean(D::FnIsConst));
        row.constant_closure = scalar(semantic.boolean(D::ClosureIsConst));
        row.has_const_conditions = scalar(semantic.boolean(D::AliasHasConstConditions));
        row.conditionally_const = scalar(semantic.boolean(D::IsConditionallyConst));
        row.target_features = scalar(semantic.boolean(D::HasTargetFeatures));
        row.impl_trait_in_trait = scalar(semantic.boolean(D::IsImplTraitInTrait));
        row.has_definition = scalar(semantic.boolean(D::HasItemDefinition));
        row.alias_layouts = match super::source_associated_facts::alias_layouts(cx, bundle, source) {
            Ok(value) => value,
            Err(why) => {
                hold(&mut unprepared, source, "alias_layouts", why);
                QueryFact::Unprepared
            }
        };
        row.parent = match semantic.parent() {
            SourceSemantic::Known(parent) => match bundle.definition_id(parent) {
                Some(parent) => QueryFact::Known(Some(parent)),
                None => {
                    hold(&mut unprepared, source, "parent", parent);
                    QueryFact::Unprepared
                }
            },
            SourceSemantic::NotApplicable => QueryFact::NotApplicable,
            SourceSemantic::Pending(why) => {
                hold(&mut unprepared, source, "parent", why);
                QueryFact::Unprepared
            }
        };
        // Nominal type_of only constructs identity arguments. It does not
        // inspect fields, normalize aliases, check traits or execute a body.
        if matches!(encoded.role, DefinitionHeaderRole::Struct | DefinitionHeaderRole::Enum | DefinitionHeaderRole::Union) {
            row.ty = QueryFact::Known(encoded.nominal_type(cx).map_err(|why| {
                fault(&format!("source factory nominal type unavailable: {why:?}"))
            })?);
        }
        row.predicates.const_conditions = match semantic.const_conditions() {
            SourceSemantic::NotApplicable => QueryFact::NotApplicable,
            SourceSemantic::Pending(why) => {
                hold(&mut unprepared, source, "const_conditions", why);
                QueryFact::Unprepared
            }
            SourceSemantic::Known(_) => {
                hold(&mut unprepared, source, "const_conditions", "condition encoding not supplied");
                QueryFact::Unprepared
            }
        };
        row.opaques = match semantic.opaque_types_defined_by() {
            SourceSemantic::NotApplicable => QueryFact::NotApplicable,
            SourceSemantic::Pending(why) => {
                hold(&mut unprepared, source, "opaque_types_defined_by", why);
                QueryFact::Unprepared
            }
            SourceSemantic::Known(_) => return Err(fault("unexpected unencoded opaque identity producer")),
        };
        row.opaques_and_coroutines = match semantic.opaque_types_and_coroutines_defined_by() {
            SourceSemantic::NotApplicable => QueryFact::NotApplicable,
            SourceSemantic::Pending(why) => {
                hold(&mut unprepared, source, "opaque_types_and_coroutines_defined_by", why);
                QueryFact::Unprepared
            }
            SourceSemantic::Known(_) => return Err(fault("unexpected unencoded coroutine identity producer")),
        };
        definitions.insert(actual.id(), row);
    }

    // These are exactly Q88's filtered query values. Q80/Q81 explicit source
    // rows cannot certify the full clauses_of or own_clauses_of query domains.
    for (&(source, filter), plan) in bundle.trait_clauses() {
        let id = bundle.definition_id(source).ok_or_else(|| fault("filtered clause owner absent"))?;
        let row = definitions.get_mut(&id).ok_or_else(|| fault("filtered clause metadata absent"))?;
        let value = match plan.encode(cx, configuration) {
            Ok(clauses) => QueryFact::Known(spanned(cx, clauses)),
            Err(why) => {
                hold(&mut unprepared, source, match filter {
                    SourceTraitClauseFilter::Super => "explicit_super_clauses_of",
                    SourceTraitClauseFilter::Implied => "explicit_implied_clauses_of",
                }, why);
                QueryFact::Unprepared
            }
        };
        match filter {
            SourceTraitClauseFilter::Super => row.predicates.explicit_super = value,
            SourceTraitClauseFilter::Implied => row.predicates.explicit_implied = value,
        }
    }

    let mut implementations = BTreeMap::new();
    let mut impl_declarations = BTreeMap::new();
    for (&raw, source) in bundle.implementations() {
        let id = bundle.definition_id(source.canonical()).ok_or_else(|| fault("impl definition absent"))?;
        let mut row = ImplFacts::unprepared();
        let semantics = source.semantics();
        row.polarity = impl_scalar(semantics.polarity());
        row.constant = impl_scalar(semantics.is_const());
        row.default_impl = impl_scalar(semantics.is_default());
        row.trait_ref = match semantics.encode_head(cx, configuration) {
            Ok(head) => QueryFact::Known(head.trait_arguments.map(|head| EarlyBinder::bind(cx, head.construct(cx)))),
            Err(why) => {
                hold(&mut unprepared, source.canonical(), "impl_trait_ref", why);
                QueryFact::Unprepared
            }
        };
        match semantics.encode_complete_clauses(cx, configuration) {
            Ok((actual, clauses)) if actual == id => {
                // Impl generics have no parent. Both query families retain
                // the same complete compiler-ordered, source-derived clauses;
                // explicit source rows alone never certify these domains.
                let retained: List<'cx, C<'cx>> = cx.list(clauses.into_iter().map(|(clause, _)| clause).collect());
                let definition = definitions.get_mut(&id).ok_or_else(|| fault("impl metadata absent"))?;
                definition.predicates.all = QueryFact::Known(EarlyBinder::bind(cx, retained));
                definition.predicates.own = QueryFact::Known(EarlyBinder::bind(cx, retained));
            }
            Ok(_) => return Err(fault("complete impl clauses belong to another definition")),
            Err(why) => hold(&mut unprepared, source.canonical(), "clauses_of", why),
        }
        row.super_outlives = match cx.services().prepare_impl_super_outlives(cx, source.outlives()) {
            Ok(actual) if actual.implementation == id => QueryFact::Known(actual.clauses),
            Ok(_) => return Err(fault("elaborated impl owner differs")),
            Err(why) => {
                hold(&mut unprepared, source.canonical(), "impl_super_outlives", why);
                QueryFact::Unprepared
            }
        };
        // Reflection, unsized guarantees and specialization ancestry each need
        // their own actual query producer; none follows merely from a head.
        implementations.insert(id, row);
        impl_declarations.insert(raw, id);
    }

    let mut traits = BTreeMap::new();
    for (source, semantics) in bundle.traits() {
        let id = bundle.definition_id(source).ok_or_else(|| fault("trait definition absent"))?;
        let mut row = TraitFacts::unprepared();
        row.automatic = trait_scalar(semantics.boolean(T::Auto));
        row.marker = trait_scalar(semantics.boolean(T::Marker));
        row.coinductive = trait_scalar(semantics.boolean(T::Coinductive));
        row.alias = trait_scalar(semantics.boolean(T::Alias));
        row.fundamental = trait_scalar(semantics.boolean(T::Fundamental));
        row.unsafe_trait = trait_scalar(semantics.boolean(T::Unsafe));
        row.default_trait = trait_scalar(semantics.boolean(T::DefaultTrait));
        row.sizedness_trait = trait_scalar(semantics.boolean(T::SizednessTrait));
        row.dyn_compatible = trait_scalar(semantics.dyn_compatible());
        row.associated_types = match super::source_associated_facts::associated_types(cx, bundle, source) {
            Ok(value) => value,
            Err(why) => {
                hold(&mut unprepared, source, "associated_types", why);
                QueryFact::Unprepared
            }
        };
        if bundle.request().traits.contains(&source) {
            match bundle.candidates(source) {
                Ok(candidates) => {
                    let mut ids = Vec::with_capacity(candidates.len());
                    let mut heads_ready = true;
                    for (raw, canonical) in candidates {
                        let id = *impl_declarations.get(raw).ok_or_else(|| fault("complete candidate was omitted"))?;
                        if id.source() != *canonical {
                            return Err(fault("complete candidate canonical identity differs"));
                        }
                        heads_ready &= implementations[&id].trait_ref.known().is_some();
                        ids.push(id);
                    }
                    if heads_ready {
                        ids.sort_unstable();
                        let retained: List<'cx, DefinitionId> = cx.list(ids);
                        row.implementations = QueryFact::Known(retained.items());
                    } else {
                        hold(&mut unprepared, source, "relevant_impls", "actual candidate head unavailable");
                    }
                }
                Err(why) => hold(&mut unprepared, source, "relevant_impls", why),
            }
        }
        traits.insert(id, row);
    }

    let mut adts = BTreeMap::new();
    for (source, plan) in bundle.adts() {
        let id = bundle.definition_id(source).ok_or_else(|| fault("ADT definition absent"))?;
        match (plan.encode(cx, configuration), plan.source_variants()) {
            (Ok(info), Ok(source_variants)) => {
                if plan.definition_id().map_err(|why| fault(&format!("ADT identity unavailable: {why:?}")))? != id {
                    return Err(fault("ADT source identity differs"));
                }
                adts.insert(id, AdtFacts { info, source_variants, unsizing: QueryFact::Unprepared });
            }
            (Err(why), _) | (_, Err(why)) => hold(&mut unprepared, source, "adt_declaration", why),
        }
    }
    let lang = language_items(bundle)?;
    Ok(MaterializedSourceFacts {
        facts: PreparedFacts::new(cx, bundle.membership_input().clone(), definitions, traits,
            implementations, BTreeMap::new(), impl_declarations, adts, BTreeMap::new(), lang,
            QueryFact::Unprepared, QueryFact::Unprepared, BTreeMap::new(), BTreeMap::new()),
        unprepared,
    })
}

/// Per-definition Q76 classification is complete even when the declaration is
/// not a solver lang item. Forward role absence is a different query: only
/// observed positive roles enter those maps, never unobserved None entries.
fn language_items(bundle: &SourceFactBundle) -> Result<LangItems, Diagnostic> {
    let mut traits = ty::data_structures::HashMap::default();
    let mut projections = ty::data_structures::HashMap::default();
    let mut adts = ty::data_structures::HashMap::default();
    let mut trait_by_definition = BTreeMap::new();
    let mut projection_by_definition = BTreeMap::new();
    let mut adt_by_definition = BTreeMap::new();
    let mut const_param_ty = QueryFact::Unprepared;
    let mut field_representing_type = QueryFact::Unprepared;
    for actual in bundle.definitions().values() {
        let Ok(classification) = actual.language().classification() else { continue; };
        let name = classification.map(|(name, _)| name);
        let id = actual.id();
        let trait_role = trait_role(name);
        let projection_role = projection_role(name);
        let adt_role = adt_role(name);
        trait_by_definition.insert(id, trait_role);
        projection_by_definition.insert(id, projection_role);
        adt_by_definition.insert(id, adt_role);
        if let Some(role) = trait_role {
            if traits.insert(role, Some(id)).is_some_and(|previous| previous != Some(id)) {
                return Err(fault("multiple actual declarations share a solver trait lang role"));
            }
        }
        if let Some(role) = projection_role {
            if projections.insert(role, Some(id)).is_some_and(|previous| previous != Some(id)) {
                return Err(fault("multiple actual declarations share a solver projection lang role"));
            }
        }
        if let Some(role) = adt_role {
            if adts.insert(role, Some(id)).is_some_and(|previous| previous != Some(id)) {
                return Err(fault("multiple actual declarations share a solver ADT lang role"));
            }
        }
        if name == lang_names::compiler_name("ConstParamTy") {
            if const_param_ty.known().is_some_and(|previous| *previous != Some(id)) {
                return Err(fault("multiple actual const parameter lang declarations"));
            }
            const_param_ty = QueryFact::Known(Some(id));
        }
        if name == lang_names::compiler_name("FieldRepresentingType") {
            if field_representing_type.known().is_some_and(|previous| *previous != Some(id)) {
                return Err(fault("multiple actual field representation lang declarations"));
            }
            field_representing_type = QueryFact::Known(Some(id));
        }
    }
    LangItems::new(const_param_ty, field_representing_type, traits, projections, adts,
        trait_by_definition, projection_by_definition, adt_by_definition,
        ty::data_structures::HashMap::default())
}

// Representation-only mapping of the exact pinned rustc_type_ir enum domains
// through the compiler's actual lang-item vocabulary (lang_names is audited).
macro_rules! roles {
    ($function:ident, $kind:ident, [$($variant:ident),+ $(,)?]) => {
        pub(super) fn $function(name: Option<&str>) -> Option<$kind> {
            let name = name?;
            $(if Some(name) == lang_names::compiler_name(stringify!($variant)) {
                return Some($kind::$variant);
            })+
            None
        }
    };
}
roles!(trait_role, SolverTraitLangItem, [AsyncFn, AsyncFnKindHelper, AsyncFnMut,
    AsyncFnOnce, AsyncIterator, BikeshedGuaranteedNoDrop, Clone, Copy, Coroutine,
    Destruct, DiscriminantKind, Drop, Field, Fn, FnMut, FnOnce, FnPtrTrait,
    FusedIterator, Future, Iterator, MetaSized, PointeeSized, PointeeTrait, Sized,
    TransmuteTrait, TrivialClone, TryAsDyn, Tuple, Unpin, Unsize]);
roles!(projection_role, SolverProjectionLangItem, [AsyncFnKindUpvars,
    AsyncFnOnceOutput, CallOnceFuture, CallRefFuture, CoroutineReturn,
    CoroutineYield, FieldBase, FieldType, FutureOutput, Metadata]);
roles!(adt_role, SolverAdtLangItem, [DynMetadata, Option, OwnedBox, Poll]);

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}

#[cfg(test)]
pub(crate) struct CurrentSourceFactoryInputs {
    bundle: SourceFactBundle,
}
#[cfg(test)]
impl CurrentSourceFactoryInputs {
    pub(crate) fn require_current(&self, db: &crate::db::memo::Database) -> Result<(), Diagnostic> {
        self.bundle.require_current(db)
    }
}
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct SourceFactoryObservation {
    pub(crate) definitions: usize,
    pub(crate) traits: usize,
    pub(crate) implementations: usize,
    pub(crate) covered_buckets: usize,
    pub(crate) candidate_buckets: BTreeMap<Digest, Vec<Digest>>,
    pub(crate) impl_empty_clauses: usize,
    pub(crate) impl_complete_clauses: usize,
    pub(crate) impl_super_outlives: usize,
    pub(crate) adts: usize,
    pub(crate) enum_reprs: usize,
    pub(crate) input_enum_reprs: Vec<Digest>,
    pub(crate) encoding_holds: usize,
    pub(crate) root_held: bool,
}

/// Observe the same production source/IR factory with actual upstream source
/// identities. There is no alternate fixture-specific fact producer or goal.
#[cfg(test)]
pub(crate) fn check_source_factory(
    db: &mut crate::db::memo::Database,
    inventory: &crate::db::seals::SealKey,
    caller: Digest,
    callee: Digest,
    candidate_trait: Digest,
    input_types: &[crate::traits::term::TyTerm],
) -> Result<(CurrentSourceFactoryInputs, SourceFactoryObservation), Diagnostic> {
    use crate::db::{dependency::{InputKey, InputValue}, poll::QueryPoll};
    use std::{collections::BTreeSet, sync::Arc};
    macro_rules! ready {
        ($result:expr) => { match $result {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(input) => return Err(fault(&format!("source factory needs actual input: {input:?}"))),
        } };
    }
    let catalog = db.input(&InputKey::DiscoverySeal(inventory.clone()))
        .ok_or_else(|| fault("source factory control lacks actual catalog"))?;
    if !matches!(catalog.as_ref(), InputValue::DiscoverySeal(_)) {
        return Err(fault("source factory control catalog variant differs"));
    }
    let caller_plan = ready!(super::source_caller_clauses::PreparedSourceCallerClauses::prepare(db,
        &super::source_definition_headers::DefinitionHeaderKey { inventory: inventory.clone(), definition: caller }));
    let configuration = caller_plan.configuration().ok_or_else(|| fault("source factory caller configuration unavailable"))?;
    let bundle = ready!(SourceFactBundle::prepare(db, catalog, super::source_fact_bundle::SourceFactRequest {
        caller, callee, traits: BTreeSet::from([candidate_trait]), input_types: input_types.to_vec(),
    }));
    bundle.require_current(db)?;
    let layouts = bundle.generic_layouts().map_err(|why| fault(&format!("source factory generic layout unavailable: {why:?}")))?;
    // The production source plan already retains these memoized heads. Keep
    // source-location/inner-reason diagnostics in this observer only; a held
    // candidate remains in its complete demanded membership bucket.
    for source in bundle.implementations().values() {
        let Some(pending) = source.types().pending() else { continue; };
        let head = ready!(db.fetch(&crate::db::provider::SyntaxProvider,
            crate::db::key::QueryKey::ImplHeadTypes(Box::new(source.types().key().clone())))).value;
        let crate::db::value::QueryValue::ImplHeadTypes(head) = head.as_ref() else {
            return Err(fault("source factory control retained head variant differs"));
        };
        let syntax = head.syntax();
        eprintln!(
            "actual source factory pending implementation canonical={:?} raw={:?} path={} self_range={:?} trait_range={:?} source_pending={:?} head_result={:?}",
            source.canonical(), source.types().key().implementation,
            syntax.path().as_str(), syntax.self_type(), syntax.trait_type(),
            pending, head.result(),
        );
    }
    let observation = configuration.with_source_context(db, Arc::clone(bundle.catalog_input()), Some(&layouts), |cx, provider| {
        let materialized = materialize(cx, &bundle, &layouts, configuration)?;
        for pending in &materialized.unprepared {
            eprintln!("actual source factory hold {:?} {}: {}", pending.definition, pending.query, pending.reason);
        }
        let facts = &materialized.facts;
        let mut observation = SourceFactoryObservation {
            definitions: facts.definitions.len(),
            traits: facts.traits.len(),
            implementations: facts.implementations.len(),
            covered_buckets: facts.traits.values().filter(|row| row.implementations.known().is_some()).count(),
            candidate_buckets: facts.traits.iter().filter_map(|(id, row)| {
                row.implementations.known().map(|members| (id.source(),
                    members.iter().map(|member| member.source()).collect()))
            }).collect(),
            impl_empty_clauses: facts.implementations.keys().filter(|id| facts.definitions[id].predicates.all.known()
                .is_some_and(|clauses| clauses.as_ref().skip_binder().items().is_empty())).count(),
            impl_complete_clauses: facts.implementations.keys().filter(|id| facts.definitions[id].predicates.all.known().is_some()).count(),
            impl_super_outlives: facts.implementations.values().filter(|row| row.super_outlives.known().is_some()).count(),
            adts: facts.adts.len(),
            enum_reprs: facts.adts.values().filter(|row| matches!(row.info.discriminant.known(), Some(Some(_)))).count(),
            input_enum_reprs: input_types.iter().filter_map(|input| {
                let crate::traits::term::TyTerm::Nominal(source, _) = input else { return None; };
                let id = bundle.definition_id(*source)?;
                matches!(facts.adts.get(&id)?.info.discriminant.known(), Some(Some(_))).then_some(*source)
            }).collect(),
            encoding_holds: materialized.unprepared.len(),
            root_held: false,
        };
        // Runs actual source receipt/coverage and pinned candidate classifier
        // checks. Neither publication nor this observer activates a root.
        provider.publish(materialized.facts)?;
        observation.root_held = provider.ready(cx).is_err();
        Ok::<_, Diagnostic>(observation)
    })??;
    caller_plan.require_current(db)?;
    bundle.require_current(db)?;
    Ok((CurrentSourceFactoryInputs { bundle }, observation))
}
