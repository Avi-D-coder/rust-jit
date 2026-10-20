//! Concrete immutable fact reader for the shared solver. This is not the
//! source producer or an inference client: publication requires a real complete
//! impl catalog, and all nondata client functions are mandatory inputs.
use super::model::{Clause, Predicate, Span, Symbol, Ty};
use super::{
    context::Context, facts::SourceFacts, identity::DefinitionId, model::*,
    parameters::SourceParameter, storage::{Interned, ScopedCache},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        seals::{DiscoveryInput, SealDomain},
        value::QueryValue,
    },
    traits::term::{BoundKind, ParamKey},
};
use ra_ap_syntax::{AstNode, ast};
use rustc_index::bit_set::DenseBitSet;
use rustc_type_ir::fast_reject::{SimplifiedType, TreatParams, simplify_type};
use rustc_type_ir::lang_items::{SolverAdtLangItem, SolverProjectionLangItem, SolverTraitLangItem};
use rustc_type_ir::search_graph::RequiredDepth;
use rustc_type_ir::solve::{QueryResult, inspect};
use rustc_type_ir::{self as ty, inherent::*, *};
use std::{
    cell::{OnceCell, RefCell},
    collections::{BTreeMap, BTreeSet},
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

pub(super) type Cx<'cx> = Context<'cx, PreparedProvider<'cx>>;
type T<'cx> = Ty<'cx, PreparedProvider<'cx>>;
type A<'cx> = Args<'cx, PreparedProvider<'cx>>;
type C<'cx> = Clause<'cx, PreparedProvider<'cx>>;
type Early<'cx, V> = ty::EarlyBinder<Cx<'cx>, V>;
type ClauseList<'cx> = Early<'cx, List<'cx, C<'cx>>>;
type Spanned<'cx> = Early<'cx, &'cx [(C<'cx>, Span)]>;
type Conditions<'cx> = Early<'cx, &'cx [Binder<Cx<'cx>, TraitRef<Cx<'cx>>>]>;
type ProofResult<'cx> = (
    QueryResult<Cx<'cx>>,
    Box<inspect::Probe<Cx<'cx>>>,
    RequiredDepth,
);

/// Captured only by reading the actual definition-keyed preparation inputs.
/// This is a source attachment, not name selection or catalog completeness.
#[derive(Debug)]
pub(super) struct DefinitionReceipt {
    definition: Digest,
    source: Arc<InputValue>,
    header: Arc<InputValue>,
    context: Arc<InputValue>,
    parsed: Arc<QueryValue>,
    generic_declaration: Arc<QueryValue>,
    implementation: Option<bool>,
    trait_declaration: bool,
}
impl DefinitionReceipt {
    pub(super) fn definition_id(&self, world: Digest, local_unit: Digest) -> DefinitionId {
        let InputValue::DeclarationContext(context) = self.context.as_ref() else { unreachable!("captured source context") };
        DefinitionId::new(world, self.definition, context.scope.crate_key == local_unit)
    }
    pub(super) fn is_nominal(&self) -> Result<bool, Diagnostic> {
        let InputValue::DeclarationContext(context) = self.context.as_ref() else { unreachable!("captured source context") };
        let QueryValue::ParseFile(parsed) = self.parsed.as_ref() else { unreachable!("captured source parse") };
        parsed.syntax_at(context.range, |node| ast::Struct::can_cast(node.kind())
            || ast::Enum::can_cast(node.kind()) || ast::Union::can_cast(node.kind())).map(|node| node.is_some())
    }
    pub(super) fn opaque_returns(&self) -> Option<&crate::frontend::declaration_generics::OpaqueReturnHeaders> {
        let QueryValue::DeclarationGenerics(generics) = self.generic_declaration.as_ref() else { unreachable!("captured generic declaration query") };
        generics.opaque_returns()
    }
    pub(super) fn is_enum(&self) -> Result<bool, Diagnostic> {
        let InputValue::DeclarationContext(context) = self.context.as_ref() else { unreachable!("captured source context") };
        let QueryValue::ParseFile(parsed) = self.parsed.as_ref() else { unreachable!("captured source parse") };
        parsed.syntax_at(context.range, |node| ast::Enum::can_cast(node.kind())).map(|node| node.is_some())
    }
    pub(super) fn same_source_metadata(&self, other: &Self) -> bool {
        self.definition == other.definition && matches!(
            (self.source.as_ref(),other.source.as_ref(),self.header.as_ref(),other.header.as_ref(),self.context.as_ref(),other.context.as_ref()),
            (InputValue::DefinitionSource(a),InputValue::DefinitionSource(b),InputValue::DefinitionHeader(ha),InputValue::DefinitionHeader(hb),InputValue::DeclarationContext(ca),InputValue::DeclarationContext(cb))
            if a == b && ha == hb && ca == cb)
    }
    pub(super) fn matches_enum_schema(&self, schema: &crate::frontend::types::NominalSchema) -> bool {
        matches!((self.header.as_ref(),self.context.as_ref()),
            (InputValue::DefinitionHeader(header),InputValue::DeclarationContext(context))
            if schema.definition == self.definition && schema.header == *header && schema.scope == context.scope)
    }
    pub(super) fn is_trait(&self) -> bool { self.trait_declaration }
    pub(super) fn is_impl(&self) -> bool { self.implementation.is_some() }
    pub(super) fn matches_generics(&self, generics: &crate::frontend::declaration_generics::DeclarationGenerics) -> bool {
        let (InputValue::DefinitionSource(parse), InputValue::DefinitionHeader(header), InputValue::DeclarationContext(context)) =
            (self.source.as_ref(), self.header.as_ref(), self.context.as_ref()) else { return false; };
        generics.definition() == self.definition && generics.matches_metadata(parse, header, context)
    }
    pub(super) fn observe(db: &mut Database, definition: Digest) -> QueryPoll<Option<Self>> {
        let source = db.input(&InputKey::DefinitionSource(definition));
        let header = db.input(&InputKey::DefinitionHeader(definition));
        let context = db.input(&InputKey::DeclarationContext(definition));
        let (Some(source), Some(header), Some(context)) = (source, header, context) else {
            return QueryPoll::Ready(Ok(None));
        };
        let (
            InputValue::DefinitionSource(parse),
            InputValue::DefinitionHeader(recipe),
            InputValue::DeclarationContext(declaration),
        ) = (source.as_ref(), header.as_ref(), context.as_ref())
        else {
            return QueryPoll::Ready(Err(fault("definition receipt input variants differ")));
        };
        if parse.profile != recipe.profile {
            return QueryPoll::Ready(Err(fault("definition receipt profile differs")));
        }
        let parsed = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())) {
            QueryPoll::Ready(Ok(value)) => value.value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::ParseFile(tree) = parsed.as_ref() else {
            return QueryPoll::Ready(Err(fault("definition receipt parser differs")));
        };
        let generic_declaration = match db.fetch(&SyntaxProvider, QueryKey::DeclarationGenerics(definition)) {
            QueryPoll::Ready(Ok(value)) => value.value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::DeclarationGenerics(generics) = generic_declaration.as_ref() else {
            return QueryPoll::Ready(Err(fault("definition generic declaration query differs")));
        };
        if !generics.matches_metadata(parse, recipe, declaration) {
            return QueryPoll::Ready(Err(fault("definition generic declaration source differs")));
        }
        let implementation =
            match tree.syntax_at(declaration.range, |node| ast::Impl::can_cast(node.kind())) {
                Ok(Some(node)) => ast::Impl::cast(node).map(|item| item.trait_().is_some()),
                Ok(None) => None,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        let trait_declaration = match tree.syntax_at(declaration.range, |node| ast::Trait::can_cast(node.kind())) {
            Ok(node) => node.is_some(),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        QueryPoll::Ready(Ok(Some(Self {
            definition,
            source,
            header,
            context,
            parsed,
            generic_declaration,
            implementation,
            trait_declaration,
        })))
    }
}

/// Configuration must be translated from the real owning compiler/profile
/// inputs. No default limit or invented feature answers are provided here.
/// Recursion-limit source: pinned compiler/rustc_interface/src/limits.rs:33–41
/// (default128 and crate override); the producer must honor that configuration.
pub(super) struct Settings<'cx> {
    pub(super) catalog: &'cx DiscoveryInput,
    pub(super) construction_generics: Option<&'cx super::source_generics::SourceGenericLayouts>,
    pub(super) recursion_limit: usize,
    pub(super) features: FeatureSet<'cx>,
    pub(super) assumptions_on_binders: bool,
    pub(super) renormalize_rigid_aliases: bool,
    pub(super) enable_next_solver_overflow_fcw: bool,
    pub(super) disable_trait_solver_fast_paths: bool,
}

/// Required actual pinned-client functions. These cannot be loaded from guest
/// source, serialized, or defaulted to a stub. Installing the real functions is
/// a separate integration requirement, not established by this record type.
pub(super) struct Clients<'cx> {
    pub(super) solver: super::infer::delegate::SolverClients<'cx>,
    pub(super) reflective_field: fn(Cx<'cx>, DefinitionId, A<'cx>) -> Option<FieldInfo<Cx<'cx>>>,
}

pub(super) struct PredicateFacts<'cx> {
    pub(super) explicit_item: Spanned<'cx>,
    pub(super) item: ClauseList<'cx>,
    pub(super) item_self: ClauseList<'cx>,
    pub(super) item_non_self: ClauseList<'cx>,
    pub(super) all: ClauseList<'cx>,
    pub(super) own: ClauseList<'cx>,
    pub(super) explicit_super: Spanned<'cx>,
    pub(super) explicit_implied: Spanned<'cx>,
    pub(super) const_conditions: Conditions<'cx>,
    pub(super) implied_const_bounds: Conditions<'cx>,
}
pub(super) struct ConstFacts<'cx> {
    pub(super) direct: bool,
    pub(super) value: Option<Early<'cx, Const<Cx<'cx>>>>,
    pub(super) abstract_value: Result<Option<Early<'cx, Const<Cx<'cx>>>>, ErrorToken>,
    pub(super) anon_kind: ty::AnonConstKind,
}
pub(super) struct CoroutineFacts<'cx> {
    pub(super) hidden: Early<'cx, Binder<Cx<'cx>, CoroutineWitnessTypes<Cx<'cx>>>>,
    pub(super) movability: Movability,
    pub(super) general: bool,
    pub(super) asynchronous: bool,
    pub(super) generator: bool,
    pub(super) async_generator: bool,
}
/// No Default: each value/absence must follow from real completed preparation.
pub(super) struct DefinitionFacts<'cx> {
    pub(super) receipt: DefinitionReceipt,
    pub(super) name: Symbol<'cx>,
    pub(super) span: Span,
    pub(super) generics: Generics<'cx, PreparedProvider<'cx>>,
    pub(super) variances: &'cx [Variance],
    pub(super) alias_variances: Option<&'cx [Variance]>,
    pub(super) ty: Option<Early<'cx, T<'cx>>>,
    pub(super) opaque_hidden: Option<Early<'cx, T<'cx>>>,
    pub(super) constant: Option<ConstFacts<'cx>>,
    pub(super) signature: Option<Early<'cx, Binder<Cx<'cx>, FnSig<Cx<'cx>>>>>,
    pub(super) coroutine: Option<CoroutineFacts<'cx>>,
    pub(super) closure_coroutine: Option<DefinitionId>,
    pub(super) parent: Option<DefinitionId>,
    pub(super) predicates: PredicateFacts<'cx>,
    pub(super) requires_sized_self: bool,
    pub(super) constant_function: bool,
    pub(super) constant_closure: bool,
    pub(super) has_const_conditions: bool,
    pub(super) conditionally_const: bool,
    pub(super) target_features: bool,
    pub(super) impl_trait_in_trait: bool,
    pub(super) has_definition: bool,
    pub(super) opaques: List<'cx, DefinitionId>,
    pub(super) opaques_and_coroutines: List<'cx, DefinitionId>,
    /// Actual aliases may have distinct Self/Impl argument layouts.
    pub(super) alias_layouts: Vec<(AliasTermKind<Cx<'cx>>, Vec<BoundKind>)>,
}
pub(super) struct TraitFacts<'cx> {
    pub(super) automatic: bool,
    pub(super) marker: bool,
    pub(super) coinductive: bool,
    pub(super) alias: bool,
    pub(super) dyn_compatible: bool,
    pub(super) fundamental: bool,
    pub(super) unsafe_trait: bool,
    pub(super) default_trait: bool,
    pub(super) sizedness_trait: bool,
    pub(super) associated_types: &'cx [DefinitionId],
    pub(super) supertraits: &'cx [DefinitionId],
    /// Complete relevant trait bucket; no caller-selected scope subset.
    pub(super) implementations: &'cx [DefinitionId],
}
pub(super) struct ImplFacts<'cx> {
    pub(super) trait_ref: Option<Early<'cx, TraitRef<Cx<'cx>>>>,
    pub(super) polarity: ImplPolarity,
    pub(super) constant: bool,
    pub(super) default_impl: bool,
    pub(super) reflection_generic: bool,
    pub(super) guaranteed_unsized: bool,
    pub(super) super_outlives: ClauseList<'cx>,
    /// Complete specialization ancestry prepared by the actual client.
    pub(super) specializes: BTreeSet<DefinitionId>,
}
pub(super) struct AdtFacts<'cx> {
    pub(super) info: AdtInfo<'cx, PreparedProvider<'cx>>,
    pub(super) unsizing: DenseBitSet<u32>,
}
pub(super) struct LangItems {
    pub(super) traits: ty::data_structures::HashMap<SolverTraitLangItem, DefinitionId>,
    pub(super) projections: ty::data_structures::HashMap<SolverProjectionLangItem, DefinitionId>,
    pub(super) adts: ty::data_structures::HashMap<SolverAdtLangItem, DefinitionId>,
    projection_by_definition: BTreeMap<DefinitionId, SolverProjectionLangItem>,
    trait_by_definition: BTreeMap<DefinitionId, SolverTraitLangItem>,
    adt_by_definition: BTreeMap<DefinitionId, SolverAdtLangItem>,
}
impl LangItems {
    pub(super) fn new(
        traits: ty::data_structures::HashMap<SolverTraitLangItem, DefinitionId>,
        projections: ty::data_structures::HashMap<SolverProjectionLangItem, DefinitionId>,
        adts: ty::data_structures::HashMap<SolverAdtLangItem, DefinitionId>,
    ) -> Result<Self, Diagnostic> {
        fn reverse<K: Copy>(
            items: &ty::data_structures::HashMap<K, DefinitionId>,
        ) -> Result<BTreeMap<DefinitionId, K>, Diagnostic> {
            let mut result = BTreeMap::new();
            for (kind, id) in items {
                if result.insert(*id, *kind).is_some() {
                    return Err(fault("ambiguous prepared language-item identity"));
                }
            }
            Ok(result)
        }
        Ok(Self {
            projection_by_definition: reverse(&projections)?,
            trait_by_definition: reverse(&traits)?,
            adt_by_definition: reverse(&adts)?,
            traits,
            projections,
            adts,
        })
    }
}
pub(super) struct PreparedFacts<'cx> {
    context: Cx<'cx>,
    source_membership: Arc<QueryValue>,
    pub(super) definitions: BTreeMap<DefinitionId, DefinitionFacts<'cx>>,
    pub(super) traits: BTreeMap<DefinitionId, TraitFacts<'cx>>,
    pub(super) implementations: BTreeMap<DefinitionId, ImplFacts<'cx>>,
    /// Demand-prepared impls, each joined to its actual raw/canonical header.
    pub(super) impl_declarations: BTreeMap<Digest, DefinitionId>,
    pub(super) adts: BTreeMap<DefinitionId, AdtFacts<'cx>>,
    pub(super) parameters: BTreeMap<(ParamKey, BoundKind), DefinitionId>,
    pub(super) lang: LangItems,
    pub(super) clients: Clients<'cx>,
    /// Optional diagnostic-print attachments, not semantic region inference.
    pub(super) anonymous_bounds: BTreeMap<(usize, usize), Region<Cx<'cx>>>,
    pub(super) anonymous_canonical: BTreeMap<usize, Region<Cx<'cx>>>,
    indexes: OnceCell<CandidateIndex>,
}
struct CandidateIndex {
    traits: BTreeMap<DefinitionId, ImplBucket>,
}
#[derive(Default)]
pub(super) struct ImplBucket {
    constructors: ty::data_structures::HashMap<SimplifiedType<DefinitionId>, Vec<DefinitionId>>,
    blankets: Vec<DefinitionId>,
}
impl ImplBucket {
    pub(super) fn non_blanket_impls(
        &self,
    ) -> &ty::data_structures::HashMap<SimplifiedType<DefinitionId>, Vec<DefinitionId>> {
        &self.constructors
    }
    pub(super) fn blanket_impls(&self) -> &[DefinitionId] {
        &self.blankets
    }
}
impl<'cx> PreparedFacts<'cx> {
    /// All source facts/client functions are explicit; the candidate index is
    /// derived later with the actual pinned classifier, never caller-supplied.
    pub(super) fn new(
        context: Cx<'cx>,
        source_membership: Arc<QueryValue>,
        definitions: BTreeMap<DefinitionId, DefinitionFacts<'cx>>,
        traits: BTreeMap<DefinitionId, TraitFacts<'cx>>,
        implementations: BTreeMap<DefinitionId, ImplFacts<'cx>>,
        impl_declarations: BTreeMap<Digest, DefinitionId>,
        adts: BTreeMap<DefinitionId, AdtFacts<'cx>>,
        parameters: BTreeMap<(ParamKey, BoundKind), DefinitionId>,
        lang: LangItems,
        clients: Clients<'cx>,
        anonymous_bounds: BTreeMap<(usize, usize), Region<Cx<'cx>>>,
        anonymous_canonical: BTreeMap<usize, Region<Cx<'cx>>>,
    ) -> Self {
        Self {
            context,
            source_membership,
            definitions,
            traits,
            implementations,
            impl_declarations,
            adts,
            parameters,
            lang,
            clients,
            anonymous_bounds,
            anonymous_canonical,
            indexes: OnceCell::new(),
        }
    }
}

/// Frozen metadata/index validation only, not current VM/source authority.
/// Only ready() can construct this token, after complete-source publication
/// and the pinned constructor/blanket classification and catalog comparison.
#[derive(Clone, Copy)]
pub(crate) struct ReadyProvider<'cx> {
    provider: PreparedProvider<'cx>,
    context: Cx<'cx>,
}

/// The concrete shared root-goal entry is gated by actual source preparation.
/// Required source-dependent solver clients are not yet implemented/installed.
pub(crate) fn evaluate_prepared<'cx>(
    ready: ReadyProvider<'cx>,
    delegate: &super::infer::delegate::SolverContext<'cx>,
    goal: ty::solve::Goal<Cx<'cx>, Predicate<'cx, PreparedProvider<'cx>>>,
    span: Span,
) -> Result<
    Result<rustc_next_trait_solver::solve::GoalEvaluation<Cx<'cx>>, ty::solve::NoSolution>,
    Diagnostic,
> {
    let cx = rustc_next_trait_solver::delegate::SolverDelegate::cx(delegate);
    if cx.services() != ready.provider || cx != ready.context {
        return Err(fault("solver delegate belongs to another prepared context"));
    }
    Ok(super::evaluate(delegate, goal, span))
}

#[derive(Clone, Copy)]
pub(crate) struct PreparedProvider<'cx> {
    world: Digest,
    settings: &'cx Settings<'cx>,
    facts: &'cx OnceCell<PreparedFacts<'cx>>,
        generic_facts: &'cx ScopedCache<BTreeMap<DefinitionId, Generics<'cx, PreparedProvider<'cx>>>>,
    diagnostics: &'cx RefCell<Vec<(Span, String)>>,
}
impl fmt::Debug for PreparedProvider<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparedLrrSolverFacts")
    }
}
impl PartialEq for PreparedProvider<'_> {
    fn eq(&self, b: &Self) -> bool {
        std::ptr::eq(self.settings, b.settings)
            && std::ptr::eq(self.facts, b.facts)
            && std::ptr::eq(self.generic_facts, b.generic_facts)
            && std::ptr::eq(self.diagnostics, b.diagnostics)
            && self.world == b.world
    }
}
impl Eq for PreparedProvider<'_> {}
impl Hash for PreparedProvider<'_> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        std::ptr::from_ref(self.settings).hash(h);
        std::ptr::from_ref(self.facts).hash(h);
        std::ptr::from_ref(self.generic_facts).hash(h);
        std::ptr::from_ref(self.diagnostics).hash(h);
        self.world.hash(h);
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}
pub(super) fn catalog_world(catalog: &DiscoveryInput) -> Result<Digest, Diagnostic> {
    if !matches!(catalog.seal.domain(), SealDomain::ImplCatalog { .. }) {
        return Err(fault(
            "upstream solving requires actual ImplCatalog completion, not a scope/test seal",
        ));
    }
    let completion = catalog.impl_catalog.as_ref().ok_or_else(|| fault("upstream solving requires actual configured source-catalog completion"))?;
    completion.require_publication(catalog)?;
    Ok(catalog.seal.fingerprint())
}

/// Own the nonescaping context lifetime. This type-checks the concrete reader
/// and scoped arenas without inventing a source provider or inference client.
/// The callback still has to translate/freeze facts and obtain ready() before
/// it can evaluate a goal. It cannot return borrowed upstream IR handles.
pub(super) fn with_provider<R>(
    settings: &Settings<'_>,
    configuration: &super::configuration::PreparedSolverConfiguration,
    run: impl for<'cx> FnOnce(Cx<'cx>, PreparedProvider<'cx>) -> R,
) -> Result<R, Diagnostic> {
    let facts = OnceCell::new();
    let diagnostics = RefCell::new(Vec::new());
    let generic_facts = ScopedCache::empty();
    let provider = PreparedProvider::new(settings, &facts, &generic_facts, &diagnostics)?;
    let state = super::context::State::new(provider);
    let view = state.view();
    let _cache = state.enter_cache();
    let prepared = settings.construction_generics.map(|source| source.materialize(view.context(), configuration)).transpose()?;
    // This owner drops the typed table before the context/interner. The
    // separate initially empty slot never retains arena handles at destruction.
    let _generic_guard = prepared.map(|prepared| generic_facts.enter(prepared));
    Ok(run(view.context(), provider))
}
impl<'cx> PreparedProvider<'cx> {
    pub(super) fn solver_clients(self) -> &'cx super::infer::delegate::SolverClients<'cx> {
        &self.data().clients.solver
    }
    pub(super) fn record_solver_overflow(self, span: Span, message: String) {
        // Diagnostic evidence only; this is not a lint/static rejection.
        self.diagnostics.borrow_mut().push((span, message));
    }
    pub(super) fn enable_next_solver_overflow_fcw(self) -> bool { self.settings.enable_next_solver_overflow_fcw }
    pub(super) fn disable_trait_solver_fast_paths(self) -> bool { self.settings.disable_trait_solver_fast_paths }
    pub(super) fn source_world(self) -> Digest { self.world }
    pub(super) fn source_parameter(self, key: ParamKey, kind: BoundKind) -> Option<SourceParameter> {
        self.settings.construction_generics?.parameter(key, kind)
    }
    pub(super) fn source_early_region(self, key: ParamKey) -> Option<EarlyRegion<'cx>> {
        self.settings.construction_generics?.early_region(key)
    }
    pub(super) fn require_generic_value(self, id: DefinitionId, value: Generics<'cx, Self>) -> Result<(), Diagnostic> {
        if id.world() != self.world || self.generic_facts.with(|generics| generics.get(&id).copied()) != Some(value) {
            return Err(fault("typed generics differ from actual source identity/order/parent count/const Type"));
        }
        Ok(())
    }
    /// A typed reader handle is not a ready solver context. Its actual complete
    /// source producer must publish every demanded record before entry.
    pub(super) fn new(
        settings: &'cx Settings<'cx>,
        facts: &'cx OnceCell<PreparedFacts<'cx>>,
    generic_facts: &'cx ScopedCache<BTreeMap<DefinitionId, Generics<'cx, PreparedProvider<'cx>>>>,
        diagnostics: &'cx RefCell<Vec<(Span, String)>>,
    ) -> Result<Self, Diagnostic> {
        let world = catalog_world(&settings.catalog)?;
        Ok(Self {
            world,
            settings,
            facts,
            generic_facts,
            diagnostics,
        })
    }
    pub(super) fn publish(self, facts: PreparedFacts<'cx>) -> Result<(), Diagnostic> {
        if facts.context.services() != self {
            return Err(fault("typed facts belong to another provider"));
        }
        if facts.indexes.get().is_some() {
            return Err(fault(
                "caller-supplied candidate indexes are not preparation evidence",
            ));
        }
        let world = self.world;
        for (id, row) in &facts.definitions {
            let source = self.settings.construction_generics.and_then(|source| source.get(*id))
                .ok_or_else(|| fault("typed definition lacks actual generic source preparation"))?;
            if !row.receipt.matches_generics(source) || self.require_generic_value(*id, row.generics).is_err() {
                return Err(fault("typed generics differ from actual source parameter identity, order or const Type"));
            }
            if id.world() != world || id.source() != row.receipt.definition {
                return Err(fault("prepared source declaration identity differs"));
            }
        }
        let QueryValue::ImplTraitCatalog(source_membership) = facts.source_membership.as_ref() else {
            return Err(fault("typed facts lack the actual source membership catalog"));
        };
        if !source_membership.matches_catalog(&self.settings.catalog) {
            return Err(fault("source membership catalog is incomplete or belongs to another world"));
        }
        let inventory = &self.settings.catalog.declaration_inventory;
        if facts.implementations.len() != facts.impl_declarations.len() {
            return Err(fault("demanded impl source/typed metadata sets differ"));
        }
        let mut declarations = BTreeSet::new();
        for (raw, id) in &facts.impl_declarations {
            let Ok(ordinal) = inventory.headers().binary_search_by_key(raw, |header| header.identity) else {
                return Err(fault("translated impl is absent from the actual complete catalog"));
            };
            let header = &inventory.headers()[ordinal];
            if !declarations.insert(*id) {
                return Err(fault("distinct impl declarations share a translated identity"));
            }
            let Some(definition) = facts.definitions.get(id) else {
                return Err(fault("demanded impl lacks its prepared definition"));
            };
            let Some(implementation) = facts.implementations.get(id) else {
                return Err(fault("demanded impl lacks its prepared metadata"));
            };
            let (InputValue::DefinitionHeader(recipe), InputValue::DeclarationContext(context)) = (
                definition.receipt.header.as_ref(), definition.receipt.context.as_ref(),
            ) else { return Err(fault("actual impl source receipt differs")); };
            let InputValue::DefinitionSource(parse) = definition.receipt.source.as_ref() else {
                return Err(fault("demanded impl source variant differs"));
            };
            let Some(classified) = source_membership.classification(*raw) else {
                return Err(fault("demanded impl actual classification absent"));
            };
            if classified.syntax().canonical_owner().map(|owner| owner.identity()) != Some(id.source())
                || !classified.matches_metadata(parse, recipe, context)
                || recipe.tokens != header.header || context.range != header.range || context.scope != header.scope
                || definition.receipt.implementation != Some(implementation.trait_ref.is_some()) {
                return Err(fault("translated impl does not match its actual retained source"));
            }
        }
        for (id, row) in &facts.adts {
            let definition = facts.definitions.get(id).ok_or_else(|| fault("ADT definition is absent"))?;
            match (&row.info.discriminant, definition.receipt.is_enum()?) {
                (Some(plan), true) if row.info.kind == AdtKind::Enum && plan.matches(&definition.receipt) => {},
                (None, false) if row.info.kind != AdtKind::Enum => {},
                _ => return Err(fault("enum discriminant metadata lacks its actual source schema")),
            }
        }
        for id in facts
            .traits
            .keys()
            .chain(facts.implementations.keys())
            .chain(facts.adts.keys())
            .chain(facts.parameters.values())
            .chain(facts.lang.traits.values())
            .chain(facts.lang.projections.values())
            .chain(facts.lang.adts.values())
        {
            if id.world() != world || !facts.definitions.contains_key(id) {
                return Err(fault(
                    "prepared fact refers to a missing/foreign declaration",
                ));
            }
        }
        let translated_sources = facts.impl_declarations.iter().map(|(raw, id)| (*raw, id.source())).collect();
        for (id, row) in &facts.traits {
            let source = source_membership;
            if !facts.definitions[id].receipt.trait_declaration {
                return Err(fault("trait membership is incomplete, foreign, or not a trait"));
            }
            let candidates = source.require_translation(&self.settings.catalog, id.source(), &translated_sources)?;
            let mut actual = Vec::new();
            for (raw, canonical) in candidates {
                let candidate = *facts.impl_declarations.get(raw)
                    .ok_or_else(|| fault("demanded typed trait bucket omits an actual source impl"))?;
                if candidate.source() != *canonical { return Err(fault("trait candidate canonical identity differs")); }
                let receipt = &facts.definitions[&candidate].receipt;
                let (InputValue::DefinitionSource(parse), InputValue::DefinitionHeader(header),
                    InputValue::DeclarationContext(context)) =
                    (receipt.source.as_ref(), receipt.header.as_ref(), receipt.context.as_ref())
                    else { return Err(fault("trait candidate receipt variants differ")); };
                if !source.classification(*raw).is_some_and(|identity| identity.matches_metadata(parse, header, context)) {
                    return Err(fault("typed candidate differs from its actual classified source"));
                }
                actual.push(candidate);
            }
            actual.sort_unstable();
            if actual != row.implementations {
                return Err(fault("typed trait bucket differs from its complete source candidates"));
            }
            if row.implementations.windows(2).any(|p| p[0] >= p[1]) {
                return Err(fault(
                    "prepared candidate identities are not unique and ordered",
                ));
            }
            for implementation in row.implementations {
                let Some(implementation) = facts.implementations.get(implementation) else {
                    return Err(fault("candidate bucket refers to an unprepared impl"));
                };
                if implementation.trait_ref.map(|r| r.def_id()) != Some(*id) {
                    return Err(fault("candidate belongs to a different actual trait"));
                }
            }
        }
        for (id, implementation) in &facts.implementations {
            if let Some(reference) = implementation.trait_ref {
                let Some(trait_data) = facts.traits.get(&reference.def_id()) else {
                    return Err(fault("actual impl trait metadata is missing"));
                };
                if trait_data.implementations.binary_search(id).is_err() {
                    return Err(fault("prepared trait bucket omits an actual trait impl"));
                }
            }
        }
        self.facts
            .set(facts)
            .map_err(|_| fault("prepared source facts were already frozen"))
    }
    pub(super) fn ready(self, cx: Cx<'cx>) -> Result<ReadyProvider<'cx>, Diagnostic> {
        if self.facts.get().is_none() { return Err(fault("complete actual source facts are not prepared")); }
        if cx.services() != self || cx != self.data().context {
            return Err(fault("source-fact reader belongs to another context"));
        }
        if self.data().indexes.get().is_none() {
            let mut index = CandidateIndex {
                traits: self
                    .data()
                    .traits
                    .keys()
                    .map(|id| (*id, ImplBucket::default()))
                    .collect(),
            };
            for (id, implementation) in &self.data().implementations {
                let Some(reference) = implementation.trait_ref else {
                    continue;
                };
                let reference = reference.skip_binder();
                let bucket = index
                    .traits
                    .get_mut(&reference.def_id)
                    .expect("validated actual trait");
                match simplify_type(cx, reference.self_ty(), TreatParams::InstantiateWithInfer) {
                    Some(key) => bucket.constructors.entry(key).or_default().push(*id),
                    None => bucket.blankets.push(*id),
                }
            }
            // Compare every derived entry against the complete source trait
            // buckets once, before publication/solver entry, not on dispatch.
            let mut observed: BTreeMap<DefinitionId, BTreeSet<DefinitionId>> = self
                .data()
                .traits
                .keys()
                .map(|id| (*id, BTreeSet::new()))
                .collect();
            for (trait_id, bucket) in &index.traits {
                for id in bucket
                    .constructors
                    .values()
                    .flatten()
                    .chain(&bucket.blankets)
                {
                    if !observed
                        .get_mut(trait_id)
                        .expect("validated trait")
                        .insert(*id)
                    {
                        return Err(fault("duplicate classified candidate"));
                    }
                }
            }
            for (trait_id, ids) in observed {
                if !ids.iter().copied().eq(self
                    .trait_data(trait_id)
                    .implementations
                    .iter()
                    .copied())
                {
                    return Err(fault(
                        "pinned candidate index differs from the complete catalog",
                    ));
                }
            }
            self.data()
                .indexes
                .set(index)
                .map_err(|_| fault("candidate index was concurrently replaced"))?;
        }
        Ok(ReadyProvider {
            provider: self,
            context: cx,
        })
    }
    pub(super) fn prepared_impls(self, id: DefinitionId) -> &'cx ImplBucket {
        self.data()
            .indexes
            .get()
            .expect("pinned candidate indexes must be ready before selection")
            .traits
            .get(&id)
            .expect("actual source trait index is missing")
    }
    fn data(self) -> &'cx PreparedFacts<'cx> {
        self.facts
            .get()
            .expect("source preparation must freeze facts before upstream callbacks")
    }
    fn definition(self, id: DefinitionId) -> &'cx DefinitionFacts<'cx> {
        assert_eq!(id.world(), self.world, "foreign prepared definition world");
        self.data()
            .definitions
            .get(&id)
            .expect("demanded source declaration was not prepared")
    }
    fn trait_data(self, id: DefinitionId) -> &'cx TraitFacts<'cx> {
        self.data()
            .traits
            .get(&id)
            .expect("demanded source trait was not prepared")
    }
    fn implementation(self, id: DefinitionId) -> &'cx ImplFacts<'cx> {
        self.data()
            .implementations
            .get(&id)
            .expect("demanded source impl was not prepared")
    }
    fn const_data(self, id: DefinitionId) -> &'cx ConstFacts<'cx> {
        self.definition(id)
            .constant
            .as_ref()
            .expect("demanded source constant was not prepared")
    }
    fn parent(self, id: DefinitionId) -> DefinitionId {
        self.definition(id)
            .parent
            .expect("associated source parent was not prepared")
    }
    fn alias_layout(self, kind: AliasTermKind<Cx<'cx>>) -> &'cx [BoundKind] {
        self.definition(alias_definition(kind))
            .alias_layouts
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, v)| v.as_slice())
            .expect("actual alias argument layout was not prepared")
    }
    fn compatible(self, layout: impl IntoIterator<Item = BoundKind>, args: A<'cx>) -> bool {
        let mut layout = layout.into_iter();
        for arg in args {
            let kind = match arg {
                Arg::Type(_) => BoundKind::Type,
                Arg::Region(_) => BoundKind::Region,
                Arg::Const(_) => BoundKind::Const,
            };
            if layout.next() != Some(kind) {
                return false;
            }
        }
        layout.next().is_none()
    }
    fn ordinary_layout(self, id: DefinitionId) -> impl Iterator<Item = BoundKind> + 'cx {
        assert_eq!(id.world(), self.world, "foreign constructor definition world");
        let source = self.settings.construction_generics.and_then(|layouts| layouts.argument_kinds(id));
        let typed = source.is_none().then(|| self.definition(id).generics.parameters);
        source.into_iter().flat_map(|kinds| kinds.iter().copied())
            .chain(typed.into_iter().flat_map(|parameters| parameters.iter().map(parameter_kind)))
    }
    fn report(self, span: Span, message: String) -> ErrorToken {
        let mut e = StableEncoder::new("lrr/v1/upstream-client-error");
        e.digest(span.origin);
        e.u32(span.start);
        e.u32(span.end);
        e.string(&message);
        let token = ErrorToken(e.finish());
        self.diagnostics.borrow_mut().push((span, message));
        token
    }
}
fn parameter_kind<'cx, F: SourceFacts<'cx>>(parameter: GenericParameter<'cx, F>) -> BoundKind {
    match parameter { GenericParameter::Type(_) => BoundKind::Type,
        GenericParameter::Region(_) => BoundKind::Region, GenericParameter::Const(_, _) => BoundKind::Const }
}
fn alias_definition(kind: AliasTermKind<Cx<'_>>) -> DefinitionId {
    match kind {
        AliasTermKind::ProjectionTy { def_id }
        | AliasTermKind::ProjectionConst { def_id }
        | AliasTermKind::InherentTy { def_id }
        | AliasTermKind::OpaqueTy { def_id }
        | AliasTermKind::FreeTy { def_id }
        | AliasTermKind::AnonConst { def_id }
        | AliasTermKind::FreeConst { def_id }
        | AliasTermKind::InherentConstSelf { def_id }
        | AliasTermKind::InherentConstImpl { def_id } => def_id,
    }
}

impl<'cx> SourceFacts<'cx> for PreparedProvider<'cx> {
    fn abstract_const(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Result<Option<Early<'cx, Const<Cx<'cx>>>>, ErrorToken> {
        self.const_data(id).abstract_value
    }
    fn generics_of(self, _: Cx<'cx>, id: DefinitionId) -> Generics<'cx, Self> {
        assert_eq!(id.world(), self.world, "foreign generic fact world");
        self.generic_facts.with(|generics| generics.get(&id).copied())
            .unwrap_or_else(|| self.definition(id).generics)
    }
    fn variances_of(self, _: Cx<'cx>, id: DefinitionId) -> &'cx [Variance] {
        self.definition(id).variances
    }
    fn opt_alias_variances(
        self,
        _: Cx<'cx>,
        kind: impl Into<AliasTermKind<Cx<'cx>>>,
    ) -> Option<&'cx [Variance]> {
        self.definition(alias_definition(kind.into()))
            .alias_variances
    }
    fn type_of(self, _: Cx<'cx>, id: DefinitionId) -> Early<'cx, T<'cx>> {
        self.definition(id)
            .ty
            .expect("source item has no prepared type")
    }
    fn type_of_opaque_hir_typeck(self, _: Cx<'cx>, id: DefinitionId) -> Early<'cx, T<'cx>> {
        self.definition(id)
            .opaque_hidden
            .expect("opaque hidden type is not prepared")
    }
    fn is_direct_const(self, _: Cx<'cx>, alias: AliasConstKind<Cx<'cx>>) -> bool {
        self.const_data(alias_definition(alias.into())).direct
    }
    fn const_of_item(
        self,
        _: Cx<'cx>,
        alias: AliasConstKind<Cx<'cx>>,
    ) -> Option<Early<'cx, Const<Cx<'cx>>>> {
        self.const_data(alias_definition(alias.into())).value
    }
    fn anon_const_kind(self, _: Cx<'cx>, id: DefinitionId) -> AnonConstKind {
        self.const_data(id).anon_kind
    }
    fn def_span(self, _: Cx<'cx>, id: DefinitionId) -> Span {
        self.definition(id).span
    }
    fn trait_ref_and_own_args_for_alias(
        self,
        cx: Cx<'cx>,
        id: DefinitionId,
        args: A<'cx>,
    ) -> (TraitRef<Cx<'cx>>, &'cx [Arg<'cx, Self>]) {
        let reference = TraitRef::from_assoc(cx, self.parent(id), args);
        (reference, &args.items()[reference.args.len()..])
    }
    fn check_alias_term_args_compatible(
        self,
        _: Cx<'cx>,
        kind: AliasTermKind<Cx<'cx>>,
        args: A<'cx>,
    ) -> bool {
        self.compatible(self.alias_layout(kind).iter().copied(), args)
    }
    fn debug_assert_args_compatible(self, _: Cx<'cx>, id: DefinitionId, args: A<'cx>) {
        debug_assert!(self.compatible(self.ordinary_layout(id), args));
    }
    fn debug_assert_alias_term_args_compatible(
        self,
        cx: Cx<'cx>,
        kind: AliasTermKind<Cx<'cx>>,
        args: A<'cx>,
    ) {
        debug_assert!(self.check_alias_term_args_compatible(cx, kind, args));
    }
    fn debug_assert_existential_args_compatible(self, _: Cx<'cx>, id: DefinitionId, args: A<'cx>) {
        let mut layout = self.ordinary_layout(id);
        debug_assert_eq!(layout.next(), Some(BoundKind::Type));
        debug_assert!(self.compatible(layout, args));
    }
    fn projection_parent(self, _: Cx<'cx>, id: DefinitionId) -> DefinitionId {
        self.parent(id)
    }
    fn impl_or_trait_assoc_term_parent(self, _: Cx<'cx>, id: DefinitionId) -> DefinitionId {
        self.parent(id)
    }
    fn inherent_alias_term_parent(self, _: Cx<'cx>, id: DefinitionId) -> DefinitionId {
        self.parent(id)
    }
    fn recursion_limit(self) -> usize {
        self.settings.recursion_limit
    }
    fn features(self) -> FeatureSet<'cx> {
        self.settings.features
    }
    fn assumptions_on_binders(self) -> bool {
        self.settings.assumptions_on_binders
    }
    fn renormalize_rigid_aliases(self) -> bool {
        self.settings.renormalize_rigid_aliases
    }
    fn coroutine_hidden_types(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, Binder<Cx<'cx>, CoroutineWitnessTypes<Cx<'cx>>>> {
        self.definition(id)
            .coroutine
            .as_ref()
            .expect("coroutine metadata was not prepared")
            .hidden
    }
    fn fn_sig(self, _: Cx<'cx>, id: DefinitionId) -> Early<'cx, Binder<Cx<'cx>, FnSig<Cx<'cx>>>> {
        self.definition(id)
            .signature
            .expect("function signature was not prepared")
    }
    fn coroutine_movability(self, _: Cx<'cx>, id: DefinitionId) -> Movability {
        self.definition(id)
            .coroutine
            .as_ref()
            .expect("coroutine metadata was not prepared")
            .movability
    }
    fn coroutine_for_closure(self, _: Cx<'cx>, id: DefinitionId) -> DefinitionId {
        self.definition(id)
            .closure_coroutine
            .expect("closure coroutine owner was not prepared")
    }
    fn generics_require_sized_self(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id).requires_sized_self
    }
    fn item_bounds(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.definition(id).predicates.item
    }
    fn explicit_item_bounds(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = (C<'cx>, Span)>> {
        self.definition(id).predicates.explicit_item.map_bound(|values| values.iter().copied())
    }
    fn is_conditionally_const(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id).conditionally_const
    }
    fn item_self_bounds(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.definition(id).predicates.item_self
    }
    fn item_non_self_bounds(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.definition(id).predicates.item_non_self
    }
    fn clauses_of(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.definition(id).predicates.all
    }
    fn own_clauses_of(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.definition(id).predicates.own
    }
    fn explicit_super_clauses_of(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = (C<'cx>, Span)>> {
        self.definition(id)
            .predicates
            .explicit_super
            .map_bound(|v| v.iter().copied())
    }
    fn explicit_implied_clauses_of(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = (C<'cx>, Span)>> {
        self.definition(id)
            .predicates
            .explicit_implied
            .map_bound(|v| v.iter().copied())
    }
    fn impl_super_outlives(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = C<'cx>>> {
        self.implementation(id).super_outlives
    }
    fn supertrait_def_ids(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> impl Iterator<Item = DefinitionId> {
        self.trait_data(id).supertraits.iter().copied()
    }
    fn impl_is_const(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.implementation(id).constant
    }
    fn fn_is_const(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id).constant_function
    }
    fn closure_is_const(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id).constant_closure
    }
    fn alias_has_const_conditions(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id).has_const_conditions
    }
    fn const_conditions(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = Binder<Cx<'cx>, TraitRef<Cx<'cx>>>>> {
        self.definition(id)
            .predicates
            .const_conditions
            .map_bound(|v| v.iter().copied())
    }
    fn explicit_implied_const_bounds(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Early<'cx, impl IntoIterator<Item = Binder<Cx<'cx>, TraitRef<Cx<'cx>>>>> {
        self.definition(id)
            .predicates
            .implied_const_bounds
            .map_bound(|v| v.iter().copied())
    }
    fn impl_self_is_guaranteed_unsized(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.implementation(id).guaranteed_unsized
    }
    fn has_target_features(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id).target_features
    }
    fn require_projection_lang_item(
        self,
        _: Cx<'cx>,
        item: SolverProjectionLangItem,
    ) -> DefinitionId {
        *self
            .data()
            .lang
            .projections
            .get(&item)
            .expect("required source projection lang item is absent")
    }
    fn require_trait_lang_item(self, _: Cx<'cx>, item: SolverTraitLangItem) -> DefinitionId {
        *self
            .data()
            .lang
            .traits
            .get(&item)
            .expect("required source trait lang item is absent")
    }
    fn require_adt_lang_item(self, _: Cx<'cx>, item: SolverAdtLangItem) -> DefinitionId {
        *self
            .data()
            .lang
            .adts
            .get(&item)
            .expect("required source ADT lang item is absent")
    }
    fn is_projection_lang_item(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
        item: SolverProjectionLangItem,
    ) -> bool {
        self.data().lang.projections.get(&item) == Some(&id)
    }
    fn is_trait_lang_item(self, _: Cx<'cx>, id: DefinitionId, item: SolverTraitLangItem) -> bool {
        self.data().lang.traits.get(&item) == Some(&id)
    }
    fn is_adt_lang_item(self, _: Cx<'cx>, id: DefinitionId, item: SolverAdtLangItem) -> bool {
        self.data().lang.adts.get(&item) == Some(&id)
    }
    fn is_default_trait(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).default_trait
    }
    fn is_sizedness_trait(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).sizedness_trait
    }
    fn as_projection_lang_item(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> Option<SolverProjectionLangItem> {
        self.data().lang.projection_by_definition.get(&id).copied()
    }
    fn as_trait_lang_item(self, _: Cx<'cx>, id: DefinitionId) -> Option<SolverTraitLangItem> {
        self.data().lang.trait_by_definition.get(&id).copied()
    }
    fn as_adt_lang_item(self, _: Cx<'cx>, id: DefinitionId) -> Option<SolverAdtLangItem> {
        self.data().lang.adt_by_definition.get(&id).copied()
    }
    fn associated_type_def_ids(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> impl IntoIterator<Item = DefinitionId> {
        self.trait_data(id).associated_types.iter().copied()
    }
    fn for_each_relevant_impl<R: VisitorResult>(
        self,
        cx: Cx<'cx>,
        reference: TraitRef<Cx<'cx>>,
        f: impl FnMut(DefinitionId) -> R,
    ) -> R {
        self.for_each_relevant_source_impl(cx, reference.def_id, reference.self_ty(), f)
    }
    fn for_each_blanket_impl<R: VisitorResult>(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
        mut f: impl FnMut(DefinitionId) -> R,
    ) -> R {
        for id in self.prepared_impls(id).blanket_impls() {
            ty::try_visit!(f(*id));
        }
        R::output()
    }
    fn has_item_definition(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id).has_definition
    }
    fn impl_specializes(self, _: Cx<'cx>, id: DefinitionId, victim: DefinitionId) -> bool {
        self.implementation(id).specializes.contains(&victim)
    }
    fn impl_is_default(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.implementation(id).default_impl
    }
    fn impl_trait_ref(self, _: Cx<'cx>, id: DefinitionId) -> Early<'cx, TraitRef<Cx<'cx>>> {
        self.implementation(id)
            .trait_ref
            .expect("trait reference requested for an inherent impl")
    }
    fn impl_polarity(self, _: Cx<'cx>, id: DefinitionId) -> ImplPolarity {
        self.implementation(id).polarity
    }
    fn is_fully_generic_for_reflection(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.implementation(id).reflection_generic
    }
    fn trait_is_auto(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).automatic
    }
    fn trait_is_marker(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).marker
    }
    fn trait_is_coinductive(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).coinductive
    }
    fn trait_is_alias(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).alias
    }
    fn trait_is_dyn_compatible(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).dyn_compatible
    }
    fn trait_is_fundamental(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).fundamental
    }
    fn trait_is_unsafe(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.trait_data(id).unsafe_trait
    }
    fn is_impl_trait_in_trait(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id).impl_trait_in_trait
    }
    fn delay_bug(self, _: Cx<'cx>, message: impl ToString) -> ErrorToken {
        self.report(
            <Span as ty::inherent::Span<Cx<'cx>>>::dummy(),
            message.to_string(),
        )
    }
    fn span_delayed_bug(self, _: Cx<'cx>, span: Span, message: impl ToString) -> ErrorToken {
        self.report(span, message.to_string())
    }
    fn is_general_coroutine(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .coroutine
            .as_ref()
            .expect("coroutine metadata was not prepared")
            .general
    }
    fn coroutine_is_async(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .coroutine
            .as_ref()
            .expect("coroutine metadata was not prepared")
            .asynchronous
    }
    fn coroutine_is_gen(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .coroutine
            .as_ref()
            .expect("coroutine metadata was not prepared")
            .generator
    }
    fn coroutine_is_async_gen(self, _: Cx<'cx>, id: DefinitionId) -> bool {
        self.definition(id)
            .coroutine
            .as_ref()
            .expect("coroutine metadata was not prepared")
            .async_generator
    }
    fn unsizing_params_for_adt(self, _: Cx<'cx>, id: DefinitionId) -> &'cx DenseBitSet<u32> {
        &self
            .data()
            .adts
            .get(&id)
            .expect("ADT metadata was not prepared")
            .unsizing
    }
    fn opaque_types_defined_by(self, _: Cx<'cx>, id: DefinitionId) -> List<'cx, DefinitionId> {
        self.definition(id).opaques
    }
    fn opaque_types_and_coroutines_defined_by(
        self,
        _: Cx<'cx>,
        id: DefinitionId,
    ) -> List<'cx, DefinitionId> {
        self.definition(id).opaques_and_coroutines
    }
    fn evaluate_root_goal_for_proof_tree_raw(
        self,
        cx: Cx<'cx>,
        goal: Interned<'cx, ty::solve::CanonicalInputData<Cx<'cx>>>,
        depth: usize,
    ) -> ProofResult<'cx> {
        assert_eq!(self.data().context, cx, "proof-tree context differs from prepared source");
        assert!(self.data().indexes.get().is_some(), "proof-tree candidate index is not prepared");
        rustc_next_trait_solver::solve::evaluate_root_goal_for_proof_tree_raw_provider::<
            super::infer::delegate::SolverContext<'cx>, _,
        >(cx, goal, depth)
    }
    fn item_name(self, _: Cx<'cx>, id: DefinitionId) -> Symbol<'cx> {
        self.definition(id).name
    }
    fn get_anon_re_bounds_lifetime(
        self,
        _: Cx<'cx>,
        index: usize,
        var: usize,
    ) -> Option<Region<Cx<'cx>>> {
        self.data().anonymous_bounds.get(&(index, var)).copied()
    }
    fn get_anon_re_canonical_bounds_lifetime(
        self,
        _: Cx<'cx>,
        index: usize,
    ) -> Option<Region<Cx<'cx>>> {
        self.data().anonymous_canonical.get(&index).copied()
    }
    fn parameter_definition(self, _: Cx<'cx>, parameter: SourceParameter) -> DefinitionId {
        *self
            .data()
            .parameters
            .get(&(parameter.source(), parameter.kind()))
            .expect("source parameter declaration was not prepared")
    }
    fn adt_info(self, _: Cx<'cx>, id: DefinitionId) -> &'cx AdtInfo<'cx, Self> {
        &self
            .data()
            .adts
            .get(&id)
            .expect("ADT metadata was not prepared")
            .info
    }
    fn field_representing_type_info(
        self,
        cx: Cx<'cx>,
        id: DefinitionId,
        args: A<'cx>,
    ) -> Option<FieldInfo<Cx<'cx>>> {
        (self.data().clients.reflective_field)(cx, id, args)
    }
    fn discriminant_type(self, cx: Cx<'cx>, ty: T<'cx>) -> T<'cx> {
        ty.pinned_discriminant_ty(cx)
    }
}

/// Host-only source-attachment control, called with the actual discovery
/// prepared by the ordinary CLI harness. It never supplies invented solver
/// facts or promotes that scope certificate into catalog completeness.
#[cfg(test)]
pub(crate) fn check_scope_receipts(db: &mut Database, seal: crate::db::seals::SealKey) -> usize {
    fn ready<T>(value: QueryPoll<T>) -> T {
        match value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("{error}"),
            QueryPoll::NeedInput(input) => panic!("unprepared source receipt: {input:?}"),
        }
    }
    let input = db.input(&InputKey::DiscoverySeal(seal.clone())).unwrap();
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
        panic!("actual discovery input absent")
    };
    assert!(matches!(catalog.seal.domain(), SealDomain::Scope { .. }));
    assert_eq!(
        catalog_world(catalog).unwrap_err().code,
        "LRR-E-UPSTREAM-SOLVER-INPUT"
    );
    let prepared = ready(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(seal)));
    let QueryValue::PreparedScopeImplHeaders(inventory) = prepared.value.as_ref() else {
        panic!("actual source inventory absent")
    };
    db.trace.detailed = true;
    let start = db.trace.events.len();
    let mut observed = 0;
    for header in inventory.inventory_root().headers() {
        let owner = inventory
            .canonical_owner(header.identity)
            .expect("actual impl owner")
            .identity();
        let receipt =
            ready(DefinitionReceipt::observe(db, owner)).expect("actual declaration receipt");
        let repeated = ready(DefinitionReceipt::observe(db, owner)).unwrap();
        assert_eq!(receipt.definition, owner);
        assert!(Arc::ptr_eq(&receipt.source, &repeated.source));
        assert!(Arc::ptr_eq(&receipt.header, &repeated.header));
        assert!(Arc::ptr_eq(&receipt.context, &repeated.context));
        assert!(Arc::ptr_eq(&receipt.parsed, &repeated.parsed));
        let InputValue::DefinitionSource(parse) = receipt.source.as_ref() else {
            panic!()
        };
        let InputValue::DefinitionHeader(recipe) = receipt.header.as_ref() else {
            panic!()
        };
        let InputValue::DeclarationContext(context) = receipt.context.as_ref() else {
            panic!()
        };
        assert_eq!(recipe.tokens, header.header);
        assert_eq!(context.range, header.range);
        assert_eq!(context.scope, header.scope);
        assert_eq!(parse.profile, recipe.profile);
        assert!(receipt.implementation.is_some());
        observed += 1;
    }
    let absent = Digest::of_bytes(b"unpublished declaration receipt control");
    assert!(db.input(&InputKey::DefinitionSource(absent)).is_none());
    assert!(ready(DefinitionReceipt::observe(db, absent)).is_none());
    assert!(db.trace.events[start..].iter().all(|event| !matches!(
        event.key,
        QueryKey::BodyStructure(_)
            | QueryKey::Thunk(_)
            | QueryKey::PatternPlan(_)
            | QueryKey::FunctionSignature(_)
    )));
    observed
}

#[cfg(test)]
pub(crate) fn check_completed_source_catalog(input: &DiscoveryInput) -> Result<Digest, Diagnostic> {
    catalog_world(input)
}

/// Supplemental source receipt control; never constructs an opaque Type ID,
/// full facts, client table or ReadyProvider from syntax-only information.
#[cfg(test)]
pub(crate) fn check_opaque_return_receipt(
    db: &mut Database, definition: Digest, expected: &Arc<QueryValue>, previous: Option<&Arc<QueryValue>>,
) {
    let receipt = match DefinitionReceipt::observe(db, definition) {
        QueryPoll::Ready(Ok(Some(receipt))) => receipt,
        other => panic!("actual opaque function receipt: {other:?}"),
    };
    let QueryValue::DeclarationGenerics(current) = expected.as_ref() else { panic!("actual generic declaration result") };
    assert!(receipt.matches_generics(current));
    let plan = current.opaque_returns().expect("actual declared return Type");
    assert!(std::ptr::eq(receipt.opaque_returns().unwrap(), plan), "receipt must share the actual retained declaration plan");
    if let Some(previous) = previous {
        let QueryValue::DeclarationGenerics(old) = previous.as_ref() else { panic!() };
        assert!(!receipt.matches_generics(old), "replaced source metadata must not acquire current authority");
    }
}
