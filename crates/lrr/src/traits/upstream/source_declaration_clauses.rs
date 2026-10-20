//! Authenticated own source requirements and their actual associated parents.
//! These are not complete compiler clauses_of/item bounds or solver readiness.
use super::{
    configuration::PreparedSolverConfiguration,
    generic_clauses::{ClauseParents, ExplicitOwnClauses, GenericClauses},
    identity::DefinitionId,
    model::{Clause, Span},
    predicate_emission::PreparedClauseEmission,
    prepared::{self, Cx},
    source_bindings::{DefinitionKind, SourceBindings, SourceRequirements},
    source_generics::SourceGenericLayouts,
    source_predicates::{SourcePredicatePending, encode_recipe, require_recipe},
    source_terms::SourceTypePending,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::declaration_source_predicates::{
        DeclarationSourcePredicatesKey, DeclarationSourcePredicatesPending,
        PreparedDeclarationSourcePredicates,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub type SourceDeclarationClausesKey = DeclarationSourcePredicatesKey;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SourceDeclarationClausesPending {
    Source(DeclarationSourcePredicatesPending),
    Recipe(SourcePredicatePending),
    Parent,
}
#[derive(Debug)]
pub struct PreparedSourceDeclarationClauses {
    key: SourceDeclarationClausesKey,
    source: Arc<QueryValue>,
    bindings: SourceBindings,
    emission: Option<PreparedClauseEmission>,
    pending: Option<SourceDeclarationClausesPending>,
}
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
impl PreparedSourceDeclarationClauses {
    pub(crate) fn key(&self) -> &SourceDeclarationClausesKey {
        &self.key
    }
    pub(crate) fn pending(&self) -> Option<SourceDeclarationClausesPending> {
        self.pending
    }
    pub(crate) fn source(&self) -> &PreparedDeclarationSourcePredicates {
        let QueryValue::DeclarationSourcePredicates(source) = self.source.as_ref() else {
            unreachable!("retained source requirement query")
        };
        source
    }
    pub(super) fn bindings(&self) -> Result<&SourceBindings, SourceDeclarationClausesPending> {
        self.pending.map_or(Ok(&self.bindings), Err)
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceDeclarationClausesKey) -> QueryPoll<Self> {
        let source = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::DeclarationSourcePredicates(Box::new(key.clone()))
        ))
        .value;
        let Some(input) = db.input(&InputKey::DiscoverySeal(key.inventory.clone())) else {
            return QueryPoll::Ready(Err(fault("declaration clause catalog is absent")));
        };
        let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
            return QueryPoll::Ready(Err(fault("declaration clause catalog input differs")));
        };
        if let Err(error) = prepared::catalog_world(catalog) {
            return QueryPoll::Ready(Err(error));
        }
        let mut result = Self {
            key: key.clone(),
            source,
            bindings: SourceBindings::empty(Arc::clone(&input)),
            emission: None,
            pending: None,
        };
        if result.source().key() != key {
            return QueryPoll::Ready(Err(fault("declaration clause source identity differs")));
        }
        let recipes = match result.source().recipes() {
            Ok(recipes) => recipes,
            Err(why) => {
                result.pending = Some(SourceDeclarationClausesPending::Source(why));
                return QueryPoll::Ready(Ok(result));
            }
        };
        let mut required = SourceRequirements::from(BTreeMap::from([(
            key.definition,
            DefinitionKind::Declaration,
        )]));
        for recipe in recipes {
            if let Err(why) = require_recipe(&mut required, recipe) {
                result.pending = Some(SourceDeclarationClausesPending::Recipe(why));
                return QueryPoll::Ready(Ok(result));
            }
        }
        let parent_definition = result.source().parent_definition();
        if let Some(parent) = parent_definition {
            if parent == key.definition {
                return QueryPoll::Ready(Err(fault(
                    "declaration cannot be its own generic parent",
                )));
            }
            required.insert(parent, DefinitionKind::Declaration);
        }
        result.bindings = ready!(SourceBindings::prepare(db, Arc::clone(&input), required));
        if let Some(why) = result.bindings.pending {
            result.pending = Some(SourceDeclarationClausesPending::Recipe(
                SourcePredicatePending::Source(why),
            ));
            return QueryPoll::Ready(Ok(result));
        }
        let emission = ready!(PreparedClauseEmission::prepare(
            db,
            Arc::clone(&result.bindings.catalog),
            result.source().recipes().expect("ready source rows")
        ));
        result.pending = emission.pending().map(|why| {
            SourceDeclarationClausesPending::Recipe(SourcePredicatePending::Emission(why))
        });
        result.emission = Some(emission);
        QueryPoll::Ready(Ok(result))
    }
    /// Run only during context construction. No source query, execution,
    /// normalization or full-clause certification occurs in this transport.
    pub(super) fn encode<'cx>(
        &self,
        cx: Cx<'cx>,
        configuration: &PreparedSolverConfiguration,
    ) -> Result<
        Vec<(Clause<'cx, prepared::PreparedProvider<'cx>>, Span)>,
        SourceDeclarationClausesPending,
    > {
        self.bindings()?;
        let InputValue::DiscoverySeal(catalog) = self.bindings.catalog.as_ref() else {
            unreachable!("retained actual catalog")
        };
        if configuration.source()
            != catalog
                .impl_catalog
                .as_ref()
                .expect("checked catalog")
                .root()
            || cx.services().source_world() != catalog.seal.fingerprint()
        {
            return Err(SourceDeclarationClausesPending::Recipe(
                SourcePredicatePending::Source(SourceTypePending::SourceDefinition),
            ));
        }
        let source = self.source();
        let Some((parse, header, context, _)) = source.declaration().source_parts() else {
            return Err(SourceDeclarationClausesPending::Recipe(
                SourcePredicatePending::Source(SourceTypePending::SourceDefinition),
            ));
        };
        let id = self.bindings.definitions[&self.key.definition];
        let QueryValue::DeclarationGenericTypes(actual) = self.bindings.generics[&id].as_ref()
        else {
            unreachable!("actual declaration generic query")
        };
        if !actual
            .declaration()
            .matches_metadata(parse, header, context)
        {
            return Err(SourceDeclarationClausesPending::Recipe(
                SourcePredicatePending::Source(SourceTypePending::ParameterLayout),
            ));
        }
        let recipes = source
            .recipes()
            .map_err(SourceDeclarationClausesPending::Source)?;
        let mut clauses = Vec::new();
        for recipe in recipes {
            if let Some(clause) = encode_recipe(
                cx,
                configuration,
                self.bindings.type_bindings(),
                self.emission
                    .as_ref()
                    .expect("ready source plan retains emission inputs"),
                recipe,
                parse.content,
                |parameter| source.parameter_range(parameter),
            )
            .map_err(SourceDeclarationClausesPending::Recipe)?
            {
                clauses.push(clause);
            }
        }
        Ok(clauses)
    }
    pub(super) fn generic_layouts(&self) -> Result<SourceGenericLayouts, Diagnostic> {
        let bindings = self
            .bindings()
            .map_err(|why| fault(&format!("own source bindings pending: {why:?}")))?;
        SourceGenericLayouts::from_bindings(Arc::clone(&self.bindings.catalog), [bindings])
    }
}

/// Parent closure is a separate preparation demand. Requesting an own bundle
/// must not inspect or force any unused parent requirement recipes.
#[derive(Debug)]
pub struct PreparedSourceDeclarationClauseHierarchy {
    key: SourceDeclarationClausesKey,
    own: Arc<QueryValue>,
    parent: Option<Arc<QueryValue>>,
    pending: Option<SourceDeclarationClausesPending>,
}
impl PreparedSourceDeclarationClauseHierarchy {
    pub(crate) fn key(&self) -> &SourceDeclarationClausesKey {
        &self.key
    }
    pub(crate) fn pending(&self) -> Option<SourceDeclarationClausesPending> {
        self.pending
    }
    pub(super) fn own(&self) -> &PreparedSourceDeclarationClauses {
        let QueryValue::SourceDeclarationClauses(own) = self.own.as_ref() else {
            unreachable!("retained own source bundle")
        };
        own
    }
    fn parent(&self) -> Option<&Self> {
        self.parent.as_ref().map(|parent| {
            let QueryValue::SourceDeclarationClauseHierarchy(parent) = parent.as_ref() else {
                unreachable!("retained parent source hierarchy")
            };
            parent.as_ref()
        })
    }
    fn plans(&self) -> impl Iterator<Item = &Self> {
        std::iter::successors(Some(self), |current| current.parent())
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceDeclarationClausesKey) -> QueryPoll<Self> {
        let own = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SourceDeclarationClauses(Box::new(key.clone()))
        ))
        .value;
        let mut result = Self {
            key: key.clone(),
            own,
            parent: None,
            pending: None,
        };
        if let Some(pending) = result.own().pending() {
            result.pending = Some(pending);
            return QueryPoll::Ready(Ok(result));
        }
        if let Some(definition) = result.own().source().parent_definition() {
            let parent = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::SourceDeclarationClauseHierarchy(Box::new(SourceDeclarationClausesKey {
                    inventory: key.inventory.clone(),
                    definition
                },))
            ))
            .value;
            let QueryValue::SourceDeclarationClauseHierarchy(plan) = parent.as_ref() else {
                return QueryPoll::Ready(Err(fault("actual parent hierarchy query differs")));
            };
            if plan.key.inventory != key.inventory || plan.key.definition != definition {
                return QueryPoll::Ready(Err(fault("actual parent hierarchy identity differs")));
            }
            if plan.pending.is_some() {
                result.pending = Some(SourceDeclarationClausesPending::Parent);
            }
            result.parent = Some(parent);
        }
        QueryPoll::Ready(Ok(result))
    }
    pub(super) fn generic_layouts(&self) -> Result<SourceGenericLayouts, Diagnostic> {
        if let Some(why) = self.pending {
            return Err(fault(&format!("source hierarchy pending: {why:?}")));
        }
        let bindings = self
            .plans()
            .map(|plan| {
                plan.own()
                    .bindings()
                    .map_err(|why| fault(&format!("source hierarchy bindings pending: {why:?}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        SourceGenericLayouts::from_bindings(Arc::clone(&self.own().bindings.catalog), bindings)
    }
    pub(super) fn encode<'cx>(
        &self,
        cx: Cx<'cx>,
        configuration: &PreparedSolverConfiguration,
    ) -> Result<EncodedSourceClauseHierarchy<'cx>, SourceDeclarationClausesPending> {
        if let Some(pending) = self.pending {
            return Err(pending);
        }
        let mut rows = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for node in self.plans() {
            let plan = node.own();
            plan.bindings()?;
            let definition = plan.bindings.definitions[&plan.key.definition];
            if !seen.insert(definition) {
                return Err(SourceDeclarationClausesPending::Parent);
            }
            let parent = node.parent().map(|parent| {
                let parent = parent.own();
                parent.bindings.definitions[&parent.key.definition]
            });
            rows.insert(
                definition,
                EncodedOwnSourceClauses {
                    parent,
                    clauses: plan.encode(cx, configuration)?,
                },
            );
        }
        let root = self.own().bindings.definitions[&self.key.definition];
        Ok(EncodedSourceClauseHierarchy { root, rows })
    }
}

struct EncodedOwnSourceClauses<'cx> {
    parent: Option<DefinitionId>,
    clauses: Vec<(Clause<'cx, prepared::PreparedProvider<'cx>>, Span)>,
}
/// Owns context-bound vectors; borrowed compiler views never outlive this map.
/// The explicit-source marker prevents use as a full compiler query answer.
pub(super) struct EncodedSourceClauseHierarchy<'cx> {
    root: DefinitionId,
    rows: BTreeMap<DefinitionId, EncodedOwnSourceClauses<'cx>>,
}
impl<'cx> EncodedSourceClauseHierarchy<'cx> {
    pub(super) fn root(&self) -> DefinitionId {
        self.root
    }
    pub(super) fn root_clauses(&self) -> GenericClauses<'_, 'cx, ExplicitOwnClauses> {
        self.clauses_of(self.root)
    }
}
impl<'data, 'cx> ClauseParents<'data, 'cx> for &'data EncodedSourceClauseHierarchy<'cx> {
    type Query = ExplicitOwnClauses;
    fn clauses_of(
        self,
        definition: DefinitionId,
    ) -> GenericClauses<'data, 'cx, ExplicitOwnClauses> {
        let row = self
            .rows
            .get(&definition)
            .expect("encoded actual source parent closure");
        GenericClauses::from_source(row.parent, &row.clauses)
    }
}

#[derive(Debug)]
pub(crate) struct CurrentSourceDeclarationClauses {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceDeclarationClauses {
    pub(crate) fn capture(db: &mut Database, key: &SourceDeclarationClausesKey) -> QueryPoll<Self> {
        let query = QueryKey::SourceDeclarationClauses(Box::new(key.clone()));
        let outcome = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !outcome.cacheable
            || !matches!(outcome.value.as_ref(), QueryValue::SourceDeclarationClauses(plan) if plan.key() == key)
        {
            return QueryPoll::Ready(Err(fault("declaration clause memo differs")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, outcome.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: outcome.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceDeclarationClauses {
        let QueryValue::SourceDeclarationClauses(plan) = self.value.as_ref() else {
            unreachable!("captured source clause query")
        };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
}
impl StableEncode for PreparedSourceDeclarationClauses {
    const DOMAIN: &'static str = "lrr/v1/source-own-declaration-clauses";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.source());
        e.record(&self.bindings);
        e.option(&self.emission, |e, value| e.record(value));
        // Exact pending source/recipe reason is retained in Q78/bindings.
        e.option(&self.pending, |e, pending| {
            e.u32(match pending {
                SourceDeclarationClausesPending::Source(_) => 0,
                SourceDeclarationClausesPending::Recipe(_) => 1,
                SourceDeclarationClausesPending::Parent => 2,
            })
        });
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-CLAUSES", message.into())
}

/// Supplemental observations of the real impl/GAT declaration pair. All clause
/// construction above is production code; this checks identities and the exact
/// pinned parent/own traversal instead of supplying solver facts.
#[cfg(test)]
pub(crate) fn check_actual_parent_source_clauses(
    db: &mut Database,
    key: &SourceDeclarationClausesKey,
    expected_parent: Digest,
) -> Result<(CurrentSourceDeclarationClauseHierarchy, usize, usize), Diagnostic> {
    use super::{configuration::SolverConfigurationKey, model, parameters::SourceParameter};
    use rustc_type_ir::{self as ty, Interner, inherent::*};
    let current = match CurrentSourceDeclarationClauseHierarchy::capture(db, key) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => return Err(fault("actual source hierarchy needs input")),
    };
    current.require_current(db)?;
    let plan = current.plan();
    assert_eq!(plan.pending(), None);
    assert_eq!(
        plan.own().source().parent_definition(),
        Some(expected_parent)
    );
    let parent = plan.parent().expect("actual impl parent");
    assert_eq!(parent.key.definition, expected_parent);
    assert!(parent.parent().is_none());
    let bindings = plan.own().bindings().unwrap();
    let input = Arc::clone(&bindings.catalog);
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
        unreachable!()
    };
    let configuration = match db.fetch(
        &SyntaxProvider,
        QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(
            catalog.impl_catalog.as_ref().unwrap().root(),
        ))),
    ) {
        QueryPoll::Ready(Ok(value)) => value.value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => {
            return Err(fault("actual source hierarchy configuration needs input"));
        }
    };
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else {
        unreachable!()
    };
    let layouts = plan.generic_layouts()?;
    let before = db.trace.events.len();
    let counts = configuration.with_source_context(db, input, Some(&layouts), |cx, _| {
        let encoded = plan.encode(cx, configuration).unwrap();
        let root = encoded.root();
        let parent = bindings.definitions[&expected_parent];
        let root_row = &encoded.rows[&root];
        let parent_row = &encoded.rows[&parent];
        assert_eq!(root_row.parent, Some(parent));
        assert_eq!((parent_row.clauses.len(), root_row.clauses.len()), (2, 1));
        let view = encoded.root_clauses();
        let identity = view.instantiate_identity(cx, &encoded);
        let args = model::Args::identity_for_item(cx, root);
        let instantiated = view.instantiate(cx, args, &encoded);
        assert_eq!(identity.clauses.len(), 3);
        assert_eq!(instantiated.clauses.len(), 3);
        let mut expected_identity = parent_row.clauses.clone();
        expected_identity.extend_from_slice(&root_row.clauses);
        assert_eq!(identity.iter().map(|(c,s)| (c.skip_norm_wip(),s)).collect::<Vec<_>>(), expected_identity);
        let parent_key = crate::traits::term::ParamKey { definition: expected_parent, ordinal: 0 };
        let child_key = plan.own().source().rebase_parent_parameter(parent_key).expect("actual parent binder rebased into child");
        assert_eq!(child_key.definition, key.definition);
        assert_eq!(child_key.ordinal, 1);
        fn parameter<'cx>(clause: model::Clause<'cx, prepared::PreparedProvider<'cx>>) -> SourceParameter {
            let ty::ClauseKind::Trait(reference) = clause.kind().skip_binder() else { panic!("actual impl bound must be a trait clause") };
            let [model::Arg::Type(self_ty), ..] = reference.trait_ref.args.items() else { panic!("actual impl bound Self argument missing") };
            let ty::TyKind::Param(parameter) = self_ty.kind() else { panic!("actual impl bound Self lost parameter identity") };
            parameter
        }
        for index in 0..2 {
            assert_eq!(parameter(identity.clauses[index].skip_norm_wip()).source(), parent_key);
            assert_eq!(parameter(instantiated.clauses[index].skip_norm_wip()).source(), child_key);
            assert_eq!(identity.spans[index], parent_row.clauses[index].1);
            assert_eq!(instantiated.spans[index], parent_row.clauses[index].1);
        }
        let ty::ClauseKind::TypeOutlives(ty::OutlivesClause(subject, region)) = instantiated.clauses[2].skip_norm_wip().kind().skip_binder() else {
            panic!("actual GAT own requirement must remain Self: 'a")
        };
        let ty::TyKind::Adt(_, arguments) = subject.kind() else { panic!("actual GAT Self lost nominal impl identity") };
        assert!(matches!(arguments.items(), [model::Arg::Type(ty)]
            if matches!(ty.kind(), ty::TyKind::Param(parameter) if parameter.source()==child_key)));
        assert!(matches!(region.kind(), ty::RegionKind::ReEarlyParam(parameter)
            if parameter.parameter.source()==(crate::traits::term::ParamKey { definition:key.definition, ordinal:0 })));
        assert_eq!(instantiated.spans[2], root_row.clauses[0].1);
        let own_identity = view.instantiate_own_identity().map(|(c,s)| (c.skip_norm_wip(),s)).collect::<Vec<_>>();
        let own = view.instantiate_own(cx,args).map(|(c,s)| (c.skip_norm_wip(),s)).collect::<Vec<_>>();
        assert_eq!(own_identity, root_row.clauses);
        assert_eq!(own, root_row.clauses);
        assert_eq!(view.instantiate_own(cx,args).rev().count(), 1);
        assert_eq!(view.instantiate_own_identity().rev().count(), 1);
        assert!(cx.services().ready(cx).is_err(), "explicit source clauses do not make a complete provider");
        (parent_row.clauses.len(), root_row.clauses.len())
    })?;
    assert_eq!(
        before,
        db.trace.events.len(),
        "pinned clause instantiation must do no source preparation"
    );
    current.require_current(db)?;
    Ok((current, counts.0, counts.1))
}

impl StableEncode for PreparedSourceDeclarationClauseHierarchy {
    const DOMAIN: &'static str = "lrr/v1/source-declaration-clause-hierarchy";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.own());
        e.option(&self.parent, |e, parent| {
            let QueryValue::SourceDeclarationClauseHierarchy(parent) = parent.as_ref() else {
                unreachable!("retained actual parent hierarchy")
            };
            e.record(parent.as_ref());
        });
        e.bool(self.pending.is_some()); // exact cause is retained in own/parent results
    }
}
#[derive(Debug)]
pub(crate) struct CurrentSourceDeclarationClauseHierarchy {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceDeclarationClauseHierarchy {
    pub(crate) fn capture(db: &mut Database, key: &SourceDeclarationClausesKey) -> QueryPoll<Self> {
        let query = QueryKey::SourceDeclarationClauseHierarchy(Box::new(key.clone()));
        let outcome = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !outcome.cacheable
            || !matches!(outcome.value.as_ref(), QueryValue::SourceDeclarationClauseHierarchy(plan) if plan.key() == key)
        {
            return QueryPoll::Ready(Err(fault("declaration hierarchy memo differs")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, outcome.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: outcome.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceDeclarationClauseHierarchy {
        let QueryValue::SourceDeclarationClauseHierarchy(plan) = self.value.as_ref() else {
            unreachable!("captured source hierarchy query")
        };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
}

/// Observe the ordinary production encoder with actual source-selected trait
/// identities. This does not supply clauses or create a solver root.
#[cfg(test)]
pub(crate) fn check_actual_declaration_clause_emission(
    db: &mut Database,
    key: &SourceDeclarationClausesKey,
) -> Result<Vec<Digest>, Diagnostic> {
    use super::configuration::SolverConfigurationKey;
    use rustc_type_ir::{self as ty, inherent::*};
    fn poll<T>(value: QueryPoll<T>) -> Result<T, Diagnostic> {
        match value {
            QueryPoll::Ready(result) => result,
            QueryPoll::NeedInput(_) => Err(fault("actual clause emission requires input")),
        }
    }
    let current = poll(CurrentSourceDeclarationClauses::capture(db, key))?;
    current.require_current(db)?;
    let plan = current.plan();
    let layouts = plan.generic_layouts()?;
    let input = Arc::clone(&plan.bindings.catalog);
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
        unreachable!()
    };
    let configuration = poll(
        db.fetch(
            &SyntaxProvider,
            QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(
                catalog
                    .impl_catalog
                    .as_ref()
                    .expect("actual catalog")
                    .root(),
            ))),
        ),
    )?
    .value;
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else {
        unreachable!()
    };
    let source = plan.source();
    let recipes = source
        .recipes()
        .map_err(|why| fault(&format!("actual source clauses pending: {why:?}")))?;
    let origin = source
        .declaration()
        .source_parts()
        .expect("actual source parts")
        .0
        .content;
    let before = db.trace.events.len();
    let observed = configuration.with_source_context(db, input, Some(&layouts), |cx, _| {
        let clauses = plan
            .encode(cx, configuration)
            .map_err(|why| fault(&format!("actual compiler clause encoding pending: {why:?}")))?;
        let emission = plan.emission.as_ref().expect("ready source emitter");
        let mut expected = Vec::new();
        for recipe in recipes {
            let (reference, span) = match recipe {
                crate::traits::predicate::SourcePredicateRecipe::Explicit {
                    source,
                    result: crate::traits::predicate::PredicateResult::Implemented(reference),
                } => (reference, *source),
                crate::traits::predicate::SourcePredicateRecipe::ResolvedSizedDefault(
                    reference,
                ) => {
                    let crate::traits::term::TyTerm::Param(parameter) = reference.self_ty else {
                        panic!("actual default subject")
                    };
                    (
                        reference,
                        source
                            .parameter_range(parameter)
                            .expect("actual default parameter"),
                    )
                }
                crate::traits::predicate::SourcePredicateRecipe::Explicit {
                    result: crate::traits::predicate::PredicateResult::Relaxed { .. },
                    ..
                } => continue,
                other => {
                    panic!("selected actual emission control needs positive trait rows: {other:?}")
                }
            };
            if matches!(
                emission
                    .require(reference.trait_key)
                    .expect("actual role input"),
                super::predicate_emission::ClauseEmission::Emit
            ) {
                expected.push((reference, span));
            }
        }
        assert_eq!(clauses.len(), expected.len());
        let mut observed = Vec::new();
        for ((clause, span), (reference, source)) in clauses.iter().zip(expected) {
            let ty::ClauseKind::Trait(encoded) = clause.kind().skip_binder() else {
                panic!("actual emitted trait clause")
            };
            assert_eq!(encoded.polarity, ty::ClausePolarity::Positive);
            assert_eq!(
                encoded.trait_ref.def_id,
                plan.bindings.definitions[&reference.trait_key]
            );
            assert_eq!(
                (span.origin, span.start, span.end),
                (origin, source.start, source.end)
            );
            let expected = super::source_terms::encode_predicate(
                cx,
                configuration,
                plan.bindings.type_bindings(),
                &crate::traits::predicate::PredicateResult::Implemented(reference.clone()),
            )
            .expect("actual source predicate encoding");
            assert_eq!(
                *clause, expected,
                "same source Self/arguments/binder as ordinary common encoder"
            );
            observed.push(encoded.trait_ref.def_id.source());
        }
        Ok::<_, Diagnostic>(observed)
    })??;
    assert_eq!(
        db.trace.events.len(),
        before,
        "clause encoding must not query source"
    );
    current.require_current(db)?;
    Ok(observed)
}
