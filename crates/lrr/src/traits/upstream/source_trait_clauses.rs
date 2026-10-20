//! Immediate filtered trait clauses through the shared source collector.
//! Recursive compiler query forcing/elaboration remains a separate dependency.
use super::{
    configuration::PreparedSolverConfiguration,
    model::{Clause, Span},
    parameters::{DeclarationParameters, DeclaredParameterPreparation},
    prepared::{self, Cx},
    source_bindings::{DefinitionKind, SourceBindings, SourceRequirements},
    source_default_bounds::{DefaultBoundsContext, DefaultBoundsPending},
    source_definition_headers::{
        DefinitionHeaderKey, DefinitionHeaderRole, PreparedDefinitionHeader,
    },
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
        seals::SealKey,
        value::QueryValue,
    },
    frontend::{
        declaration_source_predicates::DeclarationSourcePredicatesKey,
        source_predicates::{
            PreparedWrittenBounds, SourceBoundFilter, prepare_filtered_trait_bounds,
        },
        source_types::WrittenTypeDeclaration,
        type_name_namespace::SourceTypeOwner,
    },
    syntax::{outline::AttributeClass, signatures::GenericKind, spans::SourceRange},
    traits::{
        predicate::{PredicatePending, PredicateResult, SourcePredicateRecipe},
        term::{BoundKind, TyTerm},
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::{collections::BTreeMap, sync::Arc};

#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[repr(u32)]
pub enum SourceTraitClauseFilter {
    Super = 0,
    Implied = 1,
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct SourceTraitClausesKey {
    pub inventory: SealKey,
    pub definition: Digest,
    pub filter: SourceTraitClauseFilter,
}
impl StableEncode for SourceTraitClausesKey {
    const DOMAIN: &'static str = "lrr/v1/source-trait-clauses-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.inventory);
        e.digest(self.definition);
        e.u32(self.filter as u32);
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SourceTraitClausesPending {
    Header,
    NotTrait,
    Parameters,
    Environment,
    AttributeExpansion,
    /// Alias All also adds defaults for each own type parameter. The written
    /// rows and TraitSelf policy are retained, but cannot complete that query.
    AliasParameterDefaults,
    Defaults(DefaultBoundsPending),
    Recipe(SourcePredicatePending),
}
/// Actual positive immediate trait clause. This records the recursive compiler
/// query demand; it does not prove that query's value, closure or cycle result.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct TraitClauseDependency {
    pub(crate) definition: Digest,
    pub(crate) filter: SourceTraitClauseFilter,
    pub(crate) origin: Digest,
    pub(crate) source: SourceRange,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct PendingTraitClauseDependency {
    pub(crate) origin: Digest,
    pub(crate) source: SourceRange,
    pub(crate) pending: PredicatePending,
}
#[derive(Debug)]
pub struct PreparedSourceTraitClauses {
    key: SourceTraitClausesKey,
    header: Arc<QueryValue>,
    environment: Arc<QueryValue>,
    defaults: Option<Arc<QueryValue>>,
    written: Option<PreparedWrittenBounds>,
    recipes: Vec<SourcePredicateRecipe>,
    dependencies: Vec<TraitClauseDependency>,
    pending_dependencies: Vec<PendingTraitClauseDependency>,
    bindings: SourceBindings,
    source_pending: Option<SourceTraitClausesPending>,
    default_pending: Option<DefaultBoundsPending>,
    recipe_pending: Option<SourcePredicatePending>,
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
impl PreparedSourceTraitClauses {
    pub fn key(&self) -> &SourceTraitClausesKey {
        &self.key
    }
    pub fn source_pending(&self) -> Option<SourceTraitClausesPending> {
        self.source_pending
    }
    pub fn default_pending(&self) -> Option<DefaultBoundsPending> {
        self.default_pending
    }
    pub fn pending(&self) -> Option<SourceTraitClausesPending> {
        self.source_pending
            .or_else(|| {
                self.default_pending
                    .map(SourceTraitClausesPending::Defaults)
            })
            .or_else(|| self.recipe_pending.map(SourceTraitClausesPending::Recipe))
    }
    pub(crate) fn header(&self) -> &PreparedDefinitionHeader {
        let QueryValue::DefinitionHeaderPlan(header) = self.header.as_ref() else {
            unreachable!("captured filtered trait source header")
        };
        header
    }
    /// Retained immediate rows, including Pending rows. A partial list is not a
    /// successful compiler query answer; inspect pending() before encoding.
    pub(crate) fn recipes(&self) -> &[SourcePredicateRecipe] {
        &self.recipes
    }
    pub(crate) fn written(&self) -> Option<&PreparedWrittenBounds> {
        self.written.as_ref()
    }
    pub(crate) fn positive_dependencies(&self) -> &[TraitClauseDependency] {
        &self.dependencies
    }
    pub(crate) fn pending_dependencies(&self) -> &[PendingTraitClauseDependency] {
        &self.pending_dependencies
    }
    /// Retained actual inputs for a common source-context metadata closure.
    /// Availability remains pending()'s contract; this is not query admission.
    pub(super) fn bindings(&self) -> &SourceBindings {
        &self.bindings
    }
    pub(super) fn generic_layouts(&self) -> Result<SourceGenericLayouts, Diagnostic> {
        if let Some(why) = self.pending() {
            return Err(fault(&format!(
                "filtered trait source remains pending: {why:?}"
            )));
        }
        SourceGenericLayouts::from_bindings(Arc::clone(&self.bindings.catalog), [&self.bindings])
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceTraitClausesKey) -> QueryPoll<Self> {
        let header = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::DefinitionHeaderPlan(Box::new(DefinitionHeaderKey {
                inventory: key.inventory.clone(),
                definition: key.definition,
            }))
        ))
        .value;
        let environment = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(key.inventory.clone())
        ))
        .value;
        let Some(input) = db.input(&InputKey::DiscoverySeal(key.inventory.clone())) else {
            return QueryPoll::Ready(Err(fault("filtered trait source catalog absent")));
        };
        let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
            return QueryPoll::Ready(Err(fault("filtered trait source catalog variant differs")));
        };
        if let Err(error) = prepared::catalog_world(catalog) {
            return QueryPoll::Ready(Err(error));
        }
        let mut result = Self {
            key: key.clone(),
            header,
            environment,
            defaults: None,
            written: None,
            recipes: Vec::new(),
            dependencies: Vec::new(),
            pending_dependencies: Vec::new(),
            bindings: SourceBindings::empty(Arc::clone(&input)),
            source_pending: None,
            default_pending: None,
            recipe_pending: None,
        };
        macro_rules! pending {
            ($why:expr) => {{
                result.source_pending = Some($why);
                return QueryPoll::Ready(Ok(result));
            }};
        }
        let header_root = Arc::clone(&result.header);
        let QueryValue::DefinitionHeaderPlan(header) = header_root.as_ref() else {
            unreachable!()
        };
        if header.key().inventory != key.inventory || header.key().definition != key.definition {
            return QueryPoll::Ready(Err(fault("filtered trait header key differs")));
        }
        if header.pending().is_some() {
            pending!(SourceTraitClausesPending::Header);
        }
        if header.role() != DefinitionHeaderRole::Trait {
            pending!(SourceTraitClausesPending::NotTrait);
        }
        if header
            .attributes()
            .iter()
            .any(|attribute| attribute.class == AttributeClass::TransformUnsupported)
        {
            pending!(SourceTraitClausesPending::AttributeExpansion);
        }
        let declaration = header.declaration();
        let parameters = match DeclarationParameters::prepare(declaration) {
            Ok(DeclaredParameterPreparation::Ready(parameters)) => parameters,
            Ok(DeclaredParameterPreparation::Pending(_)) => {
                pending!(SourceTraitClausesPending::Parameters)
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if !declaration.has_trait_self() || parameters.count() == 0 {
            pending!(SourceTraitClausesPending::Parameters);
        }
        let self_parameter = match parameters.from_upstream(0) {
            Ok(parameter)
                if parameter.kind() == BoundKind::Type
                    && parameter.source().definition == key.definition =>
            {
                parameter
            }
            Ok(_) => pending!(SourceTraitClausesPending::Parameters),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let self_ty = TyTerm::Param(self_parameter.source());
        let Some((parse, recipe, context, parsed)) = declaration.source_parts() else {
            pending!(SourceTraitClausesPending::Header);
        };
        let syntax = match parsed.syntax_at(context.range, |node| ast::Trait::can_cast(node.kind()))
        {
            Ok(Some(syntax)) => syntax,
            Ok(None) => pending!(SourceTraitClausesPending::Header),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(trait_header) = header.trait_header() else {
            pending!(SourceTraitClausesPending::Header);
        };
        // Pinned clauses_of.rs:618-652. Trait alias Implied is All, ordinary
        // trait Implied includes Self's associated subjects, Super is SelfOnly.
        let filter = match key.filter {
            SourceTraitClauseFilter::Super => SourceBoundFilter::SelfOnly,
            SourceTraitClauseFilter::Implied if trait_header.is_alias() => SourceBoundFilter::All,
            SourceTraitClauseFilter::Implied => SourceBoundFilter::SelfAndAssociatedTypeBounds,
        };
        let alias_parameter_defaults = key.filter == SourceTraitClauseFilter::Implied
            && trait_header.is_alias()
            && declaration
                .parameters()
                .iter()
                .any(|parameter| parameter.kind == GenericKind::Type);
        let environment_root = Arc::clone(&result.environment);
        let QueryValue::PreparedScopeImplHeaders(inventory) = environment_root.as_ref() else {
            return QueryPoll::Ready(Err(fault("filtered trait environment query differs")));
        };
        if inventory.key() != &key.inventory {
            return QueryPoll::Ready(Err(fault("filtered trait inventory key differs")));
        }
        let Some(environment) = inventory.inventory_root().environment() else {
            pending!(SourceTraitClausesPending::Environment);
        };
        let owner = SourceTypeOwner {
            definition: key.definition,
            scope: &context.scope,
            header: recipe,
        };
        let written = WrittenTypeDeclaration::declaration(
            owner,
            declaration.parameters(),
            &syntax,
            Some(&self_ty),
        );
        let sources = environment.sources().iter().collect::<Vec<_>>();
        let mut view = ready!(prepare_filtered_trait_bounds(
            db,
            environment.names(),
            &sources,
            &written,
            &syntax,
            environment.language_item_core(&parse.profile),
            &self_ty,
            filter
        ));
        let defaults = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SourceDefaultBounds(Box::new(DeclarationSourcePredicatesKey {
                inventory: key.inventory.clone(),
                definition: key.definition,
            }))
        ))
        .value;
        let QueryValue::SourceDefaultBounds(policy) = defaults.as_ref() else {
            return QueryPoll::Ready(Err(fault("filtered trait default policy differs")));
        };
        if policy.key().inventory != key.inventory || policy.key().definition != key.definition {
            return QueryPoll::Ready(Err(fault("filtered trait default policy key differs")));
        }
        if let Err(why) = view.order_explicit_sized(policy) {
            result.default_pending = Some(why);
        }
        // Compiler order: implicit/default TraitSelf rows, lowered colon
        // bounds, then matching where bounds. The shared view retains sites;
        // this module must not infer their provenance from lowered subjects.
        let implicit = policy.apply(
            &self_ty,
            (parse.content, context.range),
            DefaultBoundsContext::TraitSelf,
            view.default_inputs(DefaultBoundsContext::TraitSelf),
        );
        result.recipes.extend_from_slice(view.recipes());
        match implicit {
            Ok(defaults) if result.default_pending.is_none() => {
                // Shared view inserts TraitSelf defaults before colon/where.
                result.recipes = view.with_defaults(DefaultBoundsContext::TraitSelf, defaults);
            }
            Ok(_) => {} // Ordering is held; retain every actual written row.
            Err(why) => {
                result.default_pending.get_or_insert(why);
            }
        }
        result.written = Some(view);
        result.defaults = Some(defaults);
        if alias_parameter_defaults {
            result.source_pending = Some(SourceTraitClausesPending::AliasParameterDefaults);
        }
        for recipe in &result.recipes {
            if let SourcePredicateRecipe::Explicit {
                source,
                result: PredicateResult::Implemented(reference),
            } = recipe
            {
                result.dependencies.push(TraitClauseDependency {
                    definition: reference.trait_key,
                    filter: key.filter,
                    origin: parse.content,
                    source: *source,
                });
            }
            if let SourcePredicateRecipe::Explicit {
                source,
                result: PredicateResult::Pending(pending),
            } = recipe
            {
                result
                    .pending_dependencies
                    .push(PendingTraitClauseDependency {
                        origin: parse.content,
                        source: *source,
                        pending: *pending,
                    });
            }
        }
        let mut required =
            SourceRequirements::from(BTreeMap::from([(key.definition, DefinitionKind::Trait)]));
        for recipe in &result.recipes {
            if let Err(why) = require_recipe(&mut required, recipe) {
                result.recipe_pending.get_or_insert(why);
            }
        }
        // Even when another row is pending, retain authentic bindings for
        // known selected rows. Their presence grants no complete query result.
        result.bindings = ready!(SourceBindings::prepare(db, Arc::clone(&input), required));
        if let Some(why) = result.bindings.pending {
            result
                .recipe_pending
                .get_or_insert(SourcePredicatePending::Source(why));
        }
        QueryPoll::Ready(Ok(result))
    }
    /// Immediate rows only. No recursive query forcing or elaboration is
    /// performed or certified by successful representation encoding.
    pub(super) fn encode<'cx>(
        &self,
        cx: Cx<'cx>,
        configuration: &PreparedSolverConfiguration,
    ) -> Result<Vec<(Clause<'cx, prepared::PreparedProvider<'cx>>, Span)>, SourceTraitClausesPending>
    {
        if let Some(why) = self.pending() {
            return Err(why);
        }
        let InputValue::DiscoverySeal(catalog) = self.bindings.catalog.as_ref() else {
            unreachable!()
        };
        if configuration.source()
            != catalog
                .impl_catalog
                .as_ref()
                .expect("checked source catalog")
                .root()
            || cx.services().source_world() != catalog.seal.fingerprint()
        {
            return Err(SourceTraitClausesPending::Recipe(
                SourcePredicatePending::Source(SourceTypePending::SourceDefinition),
            ));
        }
        let declaration = self.header().declaration();
        let Some((parse, recipe, context, _)) = declaration.source_parts() else {
            return Err(SourceTraitClausesPending::Header);
        };
        let id = self.bindings.definitions[&self.key.definition];
        let QueryValue::DeclarationGenericTypes(generic_types) =
            self.bindings.generics[&id].as_ref()
        else {
            unreachable!()
        };
        if !generic_types
            .declaration()
            .matches_metadata(parse, recipe, context)
        {
            return Err(SourceTraitClausesPending::Parameters);
        }
        let mut clauses = Vec::new();
        for recipe in &self.recipes {
            if let Some(clause) = encode_recipe(
                cx,
                configuration,
                self.bindings.type_bindings(),
                recipe,
                parse.content,
                |_| None,
            )
            .map_err(SourceTraitClausesPending::Recipe)?
            {
                clauses.push(clause);
            }
        }
        Ok(clauses)
    }
}
impl StableEncode for PreparedSourceTraitClauses {
    const DOMAIN: &'static str = "lrr/v1/source-trait-clauses";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.header());
        e.digest(self.environment.fingerprint());
        e.option(&self.defaults, |e, root| {
            let QueryValue::SourceDefaultBounds(policy) = root.as_ref() else {
                unreachable!()
            };
            e.record(policy.as_ref());
        });
        e.option(&self.written, |e, view| e.record(view));
        e.sequence(&self.recipes, |e, recipe| e.record(recipe));
        e.sequence(&self.dependencies, |e, dependency| {
            e.digest(dependency.definition);
            e.u32(dependency.filter as u32);
            e.digest(dependency.origin);
            e.u32(dependency.source.start);
            e.u32(dependency.source.end);
        });
        e.sequence(&self.pending_dependencies, |e, dependency| {
            e.digest(dependency.origin);
            e.u32(dependency.source.start);
            e.u32(dependency.source.end);
            encode_bound_pending(e, dependency.pending);
        });
        e.record(&self.bindings);
        e.option(&self.source_pending, |e, why| encode_pending(e, *why));
        e.option(&self.default_pending, |e, why| e.u32(*why as u32));
        e.option(&self.recipe_pending, |e, why| {
            encode_recipe_pending(e, *why)
        });
    }
}
fn encode_recipe_pending(e: &mut StableEncoder, why: SourcePredicatePending) {
    match why {
        SourcePredicatePending::Head => e.u32(0),
        SourcePredicatePending::Sized => e.u32(1),
        SourcePredicatePending::Source(why) => {
            e.u32(2);
            e.u32(why as u32);
        }
        SourcePredicatePending::Bound(why) => {
            e.u32(3);
            encode_bound_pending(e, why);
        }
    }
}
fn encode_bound_pending(e: &mut StableEncoder, why: PredicatePending) {
    e.u32(match why {
        PredicatePending::Subject => 0,
        PredicatePending::TraitName => 1,
        PredicatePending::SymbolicType => 2,
        PredicatePending::HigherRanked => 3,
        PredicatePending::AssociatedIdentity => 4,
        PredicatePending::QualifiedBound => 5,
        PredicatePending::LifetimeName => 6,
        PredicatePending::CallableArguments => 7,
    });
}
fn encode_pending(e: &mut StableEncoder, why: SourceTraitClausesPending) {
    match why {
        SourceTraitClausesPending::Header => e.u32(0),
        SourceTraitClausesPending::NotTrait => e.u32(1),
        SourceTraitClausesPending::Parameters => e.u32(2),
        SourceTraitClausesPending::Environment => e.u32(3),
        SourceTraitClausesPending::AttributeExpansion => e.u32(4),
        SourceTraitClausesPending::AliasParameterDefaults => e.u32(7),
        SourceTraitClausesPending::Defaults(why) => {
            e.u32(5);
            e.u32(why as u32);
        }
        SourceTraitClausesPending::Recipe(why) => {
            e.u32(6);
            encode_recipe_pending(e, why);
        }
    }
}
pub(crate) struct CurrentSourceTraitClauses {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceTraitClauses {
    pub(crate) fn capture(db: &mut Database, key: &SourceTraitClausesKey) -> QueryPoll<Self> {
        let query = QueryKey::SourceTraitClauses(Box::new(key.clone()));
        let outcome = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !outcome.cacheable
            || !matches!(outcome.value.as_ref(),QueryValue::SourceTraitClauses(plan) if plan.key()==key)
        {
            return QueryPoll::Ready(Err(fault("filtered trait memo key differs")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, outcome.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: outcome.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceTraitClauses {
        let QueryValue::SourceTraitClauses(plan) = self.value.as_ref() else {
            unreachable!()
        };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
}

/// Observe the production encoder on actual source plans. A typed refusal is
/// returned as Err(pending), never counted as an empty encoded clause list.
/// No root goal or completed fact provider is constructed by this control.
#[cfg(test)]
pub(crate) fn check_actual_filtered_trait_encoding(
    db: &mut Database,
    key: &SourceTraitClausesKey,
) -> Result<
    (
        CurrentSourceTraitClauses,
        Result<usize, SourceTraitClausesPending>,
    ),
    Diagnostic,
> {
    use super::{configuration::SolverConfigurationKey, model};
    use rustc_type_ir::{self as ty, inherent::*};
    let current = match CurrentSourceTraitClauses::capture(db, key) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => {
            return Err(fault("actual filtered encoding needs source input"));
        }
    };
    current.require_current(db)?;
    let plan = current.plan();
    let input = Arc::clone(&plan.bindings.catalog);
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
        unreachable!("actual filtered encoding catalog")
    };
    let configuration = match db.fetch(
        &SyntaxProvider,
        QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(
            catalog
                .impl_catalog
                .as_ref()
                .expect("checked actual catalog")
                .root(),
        ))),
    ) {
        QueryPoll::Ready(Ok(value)) => value.value,
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(_) => {
            return Err(fault("actual filtered encoding configuration needs input"));
        }
    };
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else {
        unreachable!("actual filtered encoding configuration")
    };
    let pending = plan.pending();
    let layouts = if pending.is_none() {
        Some(plan.generic_layouts()?)
    } else {
        None
    };
    let before = db.trace.events.len();
    let observed = configuration.with_source_context(db, input, layouts.as_ref(), |cx, _| {
        let encoded = plan.encode(cx, configuration);
        if let Some(expected) = pending {
            assert!(
                matches!(encoded, Err(actual) if actual==expected),
                "actual pending filtered query must refuse before construction/callbacks"
            );
            return Err(expected);
        }
        let encoded = encoded.expect("actual ready filtered source encodes");
        let (parse, _, _, _) = plan.header().declaration().source_parts().unwrap();
        let expected = plan
            .recipes
            .iter()
            .filter_map(|recipe| match recipe {
                SourcePredicateRecipe::Explicit {
                    result: PredicateResult::Relaxed { .. },
                    ..
                } => None,
                SourcePredicateRecipe::Explicit {
                    source,
                    result: PredicateResult::Implemented(reference),
                } => Some((source, reference)),
                other => panic!("actual positive Self-clause control shape differs: {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(encoded.len(), expected.len());
        for ((clause, span), (source, reference)) in encoded.iter().zip(expected) {
            let ty::ClauseKind::Trait(predicate) = clause.kind().skip_binder() else {
                panic!("actual filtered positive recipe lost its trait clause")
            };
            assert_eq!(predicate.polarity, ty::ClausePolarity::Positive);
            assert_eq!(
                predicate.trait_ref.def_id,
                plan.bindings.definitions[&reference.trait_key]
            );
            assert_eq!(predicate.trait_ref.def_id.source(), reference.trait_key);
            let TyTerm::Param(source_self) = reference.self_ty else {
                panic!("actual selected control must retain declaration Self parameter")
            };
            let [model::Arg::Type(self_ty), arguments @ ..] = predicate.trait_ref.args.items()
            else {
                panic!("actual filtered trait clause lost its Self argument")
            };
            let ty::TyKind::Param(parameter) = self_ty.kind() else {
                panic!("actual declaration Self lost source parameter identity")
            };
            assert_eq!(parameter.source(), source_self);
            assert_eq!(
                parameter.index(),
                0,
                "actual trait Self uses upstream parameter zero"
            );
            assert_eq!(source_self.definition, key.definition);
            assert_eq!(arguments.len(), reference.args.len());
            assert_eq!(
                *span,
                model::Span {
                    origin: parse.content,
                    start: source.start,
                    end: source.end
                }
            );
        }
        Ok(encoded.len())
    })?;
    assert_eq!(
        db.trace.events.len(),
        before,
        "actual filtered encoding must perform no source query"
    );
    current.require_current(db)?;
    Ok((current, observed))
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-TRAIT-CLAUSES", message.into())
}
