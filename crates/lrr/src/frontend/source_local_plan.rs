//! Owned local-prefix preparation. Metadata is not completed name authority.
use super::{OrdinaryNameSite, SourceLocalIndex, SourceLocalTables, error};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{dependency::InputValue, memo::Database, poll::QueryPoll},
    expand::{
        identity::MacroIdentity,
        scopes::{MacroDefinition, ScopeKey, TextualScopes},
    },
    frontend::{
        body_namespace::Namespace,
        hygiene::HygieneFacts,
        macro_boundary::RetainedMacroScope,
        macro_origins::ExpansionAncestry,
        macro_scopes::MacroDeclarations,
        names::{NameLookup, NameNamespaces},
        pattern_binding_occurrences::PreparedPatternBindingOccurrences,
        source_body_context::PreparedBodyContext,
        source_macros::{
            EnteredMacroIndex, OwnedRuntimeOuter, PublishedMacroIndex, SourceMacroIndex,
        },
    },
    source::changes::PreparedSourceView,
    syntax::{outline::ProducerKind, token_wire::Key},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[path = "qualified_type_names.rs"]
pub mod qualified_type_names;
#[path = "source_value_goals.rs"]
pub mod source_value_goals;
#[path = "local_type_modules.rs"]
mod type_modules;
#[path = "local_type_names.rs"]
mod type_names;
pub(crate) use type_modules::LocalTypeModuleSelector;
use type_modules::{LexicalTypeModule, ModuleRoute};
#[path = "declaration_field_types.rs"]
pub(super) mod field_types;
#[path = "body_type_declarations.rs"]
pub(crate) mod type_declarations;
pub(super) use type_names::{ItemNames, LocalTypeNames};
/// These are required proof joins, not negative language answers.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum LocalPlanAuthorityDemand {
    CurrentEnvironmentPublication,
    ExactEnteredOccurrence,
    DeclarationNameContext,
    ModuleNamespacePublication,
    ExpansionDefinition(Key),
}
/// Debug/preparation data only. No caller can turn it into a source certificate.
pub(crate) struct LocalPrefixMetadata {
    lookup: Option<NameLookup>,
    demands: Vec<LocalPlanAuthorityDemand>,
}
impl LocalPrefixMetadata {
    pub(crate) fn lookup(&self) -> Option<&NameLookup> {
        self.lookup.as_ref()
    }
    pub(crate) fn demands(&self) -> &[LocalPlanAuthorityDemand] {
        &self.demands
    }
}

/// Source-local proof fragment. A coordinator must still join its actual
/// current environment-publication input before downstream source Type authority.
pub(crate) struct LocalTypePrefix {
    plan: Arc<PreparedSourceLocalPlan>,
    node: Option<crate::ir::pc::NodeId>,
    header: Option<DeclarationTypeSite>,
    selection: LocalTypePrefixSelection,
    use_name: Option<LocalTypeUse>,
    module_selector: Option<LocalTypeModuleSelector>,
    fingerprint: Digest,
}
#[derive(Clone)]
enum DeclarationTypeSite {
    Return {
        definition: Digest,
        range: crate::syntax::spans::SourceRange,
    },
    Field {
        definition: Digest,
        range: crate::syntax::spans::SourceRange,
    },
    Parameter {
        definition: Digest,
        ordinal: u32,
        range: crate::syntax::spans::SourceRange,
    },
}
#[derive(Clone)]
pub(crate) struct LocalTypeUse {
    name: String,
    range: crate::syntax::spans::SourceRange,
    context: Key,
    edition: u16,
}
impl LocalTypeUse {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn range(&self) -> crate::syntax::spans::SourceRange {
        self.range
    }
    pub(crate) fn context(&self) -> Key {
        self.context
    }
    pub(crate) fn edition(&self) -> u16 {
        self.edition
    }
}
pub(crate) enum LocalTypePrefixSelection {
    Lookup(NameLookup),
    Parameter(crate::traits::term::ParamKey),
    ModuleTail,
    NeedImport(Digest),
    NeedDeclarationOccurrence(Digest),
    NeedOwnerGenericOccurrence,
    NeedSource(crate::frontend::source_body_context::type_occurrences::BodyTypeAuthorityDemand),
    QualifiedTail,
    NeedExpansionDefinition(Key),
    NeedDefinitionModule(Key),
    NeedModulePublication,
}
impl LocalTypePrefix {
    pub(crate) fn module_selector(&self) -> Option<&LocalTypeModuleSelector> {
        self.module_selector.as_ref()
    }
    /// Cold query tail only. The owning current environment publication remains
    /// a prerequisite for final Type authority; this returns the ordinary current
    /// module observation, including incomplete/unsealed states unchanged.
    pub(crate) fn module_name(
        &self,
        db: &mut Database,
    ) -> QueryPoll<Option<Arc<crate::frontend::current_module_names::CurrentModuleNameObservation>>>
    {
        if !matches!(self.selection, LocalTypePrefixSelection::ModuleTail) {
            return QueryPoll::Ready(Ok(None));
        }
        match self.plan.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let Some(name) = &self.use_name else {
            return QueryPoll::Ready(Err(error("module tail has no actual Type use")));
        };
        if self.module_selector.is_none() {
            return QueryPoll::Ready(Err(error("module tail has no use-context selector")));
        }
        let key = match self
            .plan
            .lexical_module
            .request(name.name(), name.edition())
        {
            Ok(Some(key)) => key,
            Ok(None) => {
                return QueryPoll::Ready(Err(error("module tail source publication absent")));
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let result = match db.fetch(
            &crate::db::provider::SyntaxProvider,
            crate::db::key::QueryKey::CurrentModuleName(Box::new(key.clone())),
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        match result.value.as_ref() {
            crate::db::value::QueryValue::CurrentModuleName(observation)
                if observation.matches_request(&key) =>
            {
                QueryPoll::Ready(Ok(Some(Arc::clone(observation))))
            }
            _ => QueryPoll::Ready(Err(error("module tail current observation differs"))),
        }
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn node(&self) -> Option<crate::ir::pc::NodeId> {
        self.node
    }
    pub(crate) fn use_name(&self) -> Option<&LocalTypeUse> {
        self.use_name.as_ref()
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn selection(&self) -> &LocalTypePrefixSelection {
        &self.selection
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let extra = match &self.selection {
            LocalTypePrefixSelection::Lookup(NameLookup::Ambiguous(definitions)) => definitions
                .capacity()
                .checked_mul(std::mem::size_of::<crate::frontend::names::NameDefinition>())
                .ok_or_else(crate::runtime::arena::resource)?,
            _ => 0,
        };
        std::mem::size_of::<Self>()
            .checked_add(extra)
            .and_then(|n| {
                n.checked_add(
                    self.use_name
                        .as_ref()
                        .map_or(0, |name| name.name.capacity()),
                )
            })
            .ok_or_else(crate::runtime::arena::resource)
    }
}
impl StableEncode for LocalTypePrefix {
    const DOMAIN: &'static str = "lrr/v5/local-type-prefix-source-proof";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.plan.source_identity());
        e.option(&self.node, |e, node| e.u32(node.0));
        e.option(&self.header, |e, site| {
            let (definition, span) = match site {
                DeclarationTypeSite::Return { definition, range } => {
                    e.u8(2);
                    (definition, range)
                }
                DeclarationTypeSite::Field { definition, range } => {
                    e.u8(0);
                    (definition, range)
                }
                DeclarationTypeSite::Parameter {
                    definition,
                    ordinal,
                    range,
                } => {
                    e.u8(1);
                    e.u32(*ordinal);
                    (definition, range)
                }
            };
            e.digest(*definition);
            e.u32(span.start);
            e.u32(span.end);
        });
        e.option(&self.use_name, |e, name| {
            e.string(name.name());
            e.u32(name.range().start);
            e.u32(name.range().end);
            e.digest(Digest(name.context().0));
            e.u16(name.edition());
        });
        e.option(&self.module_selector, |e, selector| selector.encode(e));
        match &self.selection {
            LocalTypePrefixSelection::Lookup(lookup) => {
                e.u8(0);
                e.record(lookup);
            }
            LocalTypePrefixSelection::Parameter(parameter) => {
                e.u8(1);
                e.digest(parameter.definition);
                e.u32(parameter.ordinal);
            }
            LocalTypePrefixSelection::ModuleTail => e.u8(2),
            LocalTypePrefixSelection::NeedImport(identity) => {
                e.u8(3);
                e.digest(*identity);
            }
            LocalTypePrefixSelection::NeedDeclarationOccurrence(identity) => {
                e.u8(4);
                e.digest(*identity);
            }
            LocalTypePrefixSelection::NeedOwnerGenericOccurrence => e.u8(5),
            LocalTypePrefixSelection::NeedSource(demand) => {
                e.u8(6);
                use crate::frontend::source_body_context::type_occurrences::BodyTypeAuthorityDemand as D;
                e.u8(match demand {
                    D::CachedLocalPrefix => 0,
                    D::GeneratedBoundary => 1,
                    D::RetainedExpansionOccurrence => 2,
                    D::TypeRecipe => 3,
                });
            }
            LocalTypePrefixSelection::NeedExpansionDefinition(key) => {
                e.u8(8);
                e.digest(Digest(key.0));
            }
            LocalTypePrefixSelection::NeedDefinitionModule(expansion) => {
                e.u8(10);
                e.digest(Digest(expansion.0));
            }
            LocalTypePrefixSelection::NeedModulePublication => e.u8(11),
            LocalTypePrefixSelection::QualifiedTail => e.u8(12),
        }
    }
}
pub struct PreparedSourceLocalPlan {
    entered: EnteredMacroIndex<'static>,
    tables: Arc<SourceLocalTables>,
    context: Arc<PreparedBodyContext>,
    missing_expansions: Vec<Key>,
    type_names: LocalTypeNames,
    field_types: field_types::DeclarationFieldTypes,
    pattern_occurrences: PreparedPatternBindingOccurrences,
    lexical_module: LexicalTypeModule,
    source_identity: Digest,
    exclusive_bytes: usize,
    environment: Option<Arc<crate::db::BodyEnvironmentInput>>,
}
/// Actual immutable roots. Reading them never permits rebinding a plan to a
/// different environment; the lookup adapter below always borrows its own owner.
pub(crate) struct SourceLocalPlanRoots<'a> {
    entered: &'a EnteredMacroIndex<'static>,
}
impl SourceLocalPlanRoots<'_> {
    pub(crate) fn source(&self) -> &PreparedSourceView {
        &self.entered.parent.view
    }
    pub(crate) fn source_index(&self) -> &Arc<SourceMacroIndex> {
        self.entered.source_macro_root()
    }
    pub(crate) fn runtime_outers(&self) -> &Arc<Vec<OwnedRuntimeOuter>> {
        self.entered.runtime_outer_root()
    }
    pub(crate) fn scopes(&self) -> &Arc<TextualScopes> {
        self.entered.scope_root()
    }
    pub(crate) fn declarations(&self) -> &Arc<MacroDeclarations> {
        self.entered.declaration_root()
    }
    pub(crate) fn scope_keys(&self) -> &Arc<BTreeMap<u32, ScopeKey>> {
        self.entered.scope_key_root()
    }
    pub(crate) fn retired_producers(&self) -> &Arc<BTreeSet<Digest>> {
        self.entered.retired_root()
    }
    pub(crate) fn namespace(&self) -> Option<&Arc<Namespace>> {
        self.entered.namespace_root()
    }
    pub(crate) fn producers(&self) -> impl Iterator<Item = &Arc<BTreeMap<Digest, ProducerKind>>> {
        self.entered.producer_roots()
    }
    pub(crate) fn retained_scopes(&self) -> impl Iterator<Item = &RetainedMacroScope> {
        self.entered.retained_scope_roots()
    }
    pub(crate) fn hygiene(&self) -> &Arc<HygieneFacts> {
        self.entered.hygiene_root_handle()
    }
    pub(crate) fn ancestry(&self) -> &Arc<ExpansionAncestry> {
        self.entered.ancestry_root_handle()
    }
}
impl PreparedSourceLocalPlan {
    pub(in crate::frontend) fn enter_prepared_local_body(
        &self,
        context: Arc<PreparedBodyContext>,
    ) -> Result<EnteredMacroIndex<'static>, Diagnostic> {
        let selected = context
            .source()
            .local_parent()
            .ok_or_else(|| error("local source plan requires its actual retained parent"))?;
        if selected.source_identity != self.source_identity
            || selected.environment.as_ref().map(|env| env.identity())
                != self.environment.as_ref().map(|env| env.identity())
        {
            return Err(error(
                "local source plan belongs to another parent environment",
            ));
        }
        self.entered
            .enter_prepared_nested(context)
            .map(|entered| entered.into_owned())
    }
    pub(crate) fn environment(&self) -> Option<&Arc<crate::db::BodyEnvironmentInput>> {
        self.environment.as_ref()
    }
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        let Some(environment) = &self.environment else {
            return self.context.require_current(db);
        };
        let key = crate::db::dependency::InputKey::BodyEnvironment(environment.body().clone());
        if !matches!(db.input(&key).as_deref(),Some(InputValue::BodyEnvironment(current)) if current.identity()==environment.identity())
        {
            return QueryPoll::Ready(Err(error("published local environment was replaced")));
        }
        self.context.require_current_in_environment(db, environment)
    }
    /// Pure-query join to an opaque coordinator input. No caller-provided
    /// digest or equivalent-looking source can install this association.
    pub(crate) fn attach_environment(
        self: Arc<Self>,
        environment: Arc<crate::db::BodyEnvironmentInput>,
    ) -> Result<Arc<Self>, Diagnostic> {
        if self.context.source().identity() != environment.source().identity()
            || self.context.source().index().body() != environment.body()
        {
            return Err(error("local plan owning environment differs"));
        }
        let mut plan = self;
        let inner =
            Arc::get_mut(&mut plan).ok_or_else(|| error("shared local plan cannot be rebound"))?;
        let mut identity = StableEncoder::new("lrr/v1/published-source-local-plan");
        identity.digest(inner.source_identity);
        identity.digest(environment.semantic_identity());
        inner.source_identity = identity.finish();
        inner.environment = Some(environment);
        Ok(plan)
    }
    /// Cold source Type preparation. Production callers must consume the
    /// retained result through the owning current environment query.
    pub(crate) fn prepare_body_type(
        self: &Arc<Self>,
        db: &mut Database,
        node: crate::ir::pc::NodeId,
    ) -> QueryPoll<type_declarations::BodyTypeOutcome> {
        let prefix = match self.type_prefix(db, node) {
            QueryPoll::Ready(Ok(prefix)) => prefix,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        type_declarations::prepare(db, prefix)
    }
    pub(crate) fn prepare(
        db: &mut Database,
        parent: &PublishedMacroIndex,
        context: Arc<PreparedBodyContext>,
    ) -> QueryPoll<Arc<Self>> {
        match context.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let lexical_module = match LexicalTypeModule::prepare(db, &context) {
            QueryPoll::Ready(Ok(module)) => module,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready(Self::build(parent, context, lexical_module).map(Arc::new))
    }
    fn build(
        parent: &PublishedMacroIndex,
        context: Arc<PreparedBodyContext>,
        lexical_module: LexicalTypeModule,
    ) -> Result<Self, Diagnostic> {
        let entered = parent.enter_prepared_body(Arc::clone(&context))?;
        let (expansions, missing_expansions) = expansion_definitions(&entered, &context)?;
        let (root, hygiene) = entered.local_hygiene();
        let type_names = LocalTypeNames::prepare(&context, root, &hygiene)?;
        let field_types = field_types::DeclarationFieldTypes::prepare(&context, root, &hygiene)?;
        let pattern_occurrences =
            PreparedPatternBindingOccurrences::capture(Arc::clone(&context), usize::MAX)?;
        let module_bytes = lexical_module.extra_bytes();
        let mut identity = StableEncoder::new("lrr/v5/owned-local-source-environment");
        identity.digest(context.source().identity());
        identity.digest(type_names.fingerprint());
        identity.digest(field_types.fingerprint());
        identity.digest(pattern_occurrences.identity());
        identity.digest(lexical_module.fingerprint());
        identity.u64(expansions.len() as u64);
        for (key, definition) in &expansions {
            identity.digest(Digest(key.0));
            identity.digest(definition.identity);
            identity.digest(definition.defining_unit);
            identity.u32(u32::from(definition.definition_edition));
        }
        identity.u64(missing_expansions.len() as u64);
        for key in &missing_expansions {
            identity.digest(Digest(key.0));
        }
        // No raw caller-provided pattern decisions or emitted declaration grants.
        let locals = entered.local_index_with_pattern_names(
            parent.source(),
            context.source().body(),
            &BTreeMap::new(),
            &expansions,
            context.source().index(),
            Some(&type_names),
        )?;
        let tables = Arc::clone(&locals.tables);
        identity.digest(tables.binding_source_identity());
        drop(locals);
        let entered = entered.into_owned();
        let exclusive_bytes = std::mem::size_of::<Self>()
            .checked_sub(std::mem::size_of::<EnteredMacroIndex>())
            .and_then(|n| n.checked_add(entered.exclusive_metadata_bytes().ok()?))
            .and_then(|n| {
                n.checked_add(
                    missing_expansions
                        .capacity()
                        .checked_mul(std::mem::size_of::<Key>())?,
                )
            })
            .and_then(|n| n.checked_add(type_names.extra_bytes()))
            .and_then(|n| n.checked_add(field_types.extra_bytes()))
            .and_then(|n| {
                n.checked_add(
                    pattern_occurrences
                        .metadata_bytes()
                        .checked_sub(std::mem::size_of::<PreparedPatternBindingOccurrences>())?,
                )
            })
            .and_then(|n| n.checked_add(module_bytes))
            .ok_or_else(crate::runtime::arena::resource)?;
        // Bound the newly retained allocation independently of shared roots.
        tables
            .metadata_bytes()?
            .checked_add(exclusive_bytes)
            .ok_or_else(crate::runtime::arena::resource)?;
        Ok(Self {
            entered,
            tables,
            context,
            missing_expansions,
            type_names,
            field_types,
            pattern_occurrences,
            lexical_module,
            source_identity: identity.finish(),
            exclusive_bytes,
            environment: None,
        })
    }
    fn select_type_prefix(
        &self,
        found: super::PrefixResolution,
        context: Key,
    ) -> Result<(LocalTypePrefixSelection, Option<LocalTypeModuleSelector>), Diagnostic> {
        let mut selector = None;
        let selected = match found {
            super::PrefixResolution::Name(found) => LocalTypePrefixSelection::Lookup(found),
            super::PrefixResolution::Parameter(parameter) => {
                LocalTypePrefixSelection::Parameter(parameter)
            }
            super::PrefixResolution::ModuleTail => {
                let (_, hygiene) = self.entered.local_hygiene();
                match self.lexical_module.route(&hygiene, context)? {
                    ModuleRoute::Definition(expansion) => {
                        LocalTypePrefixSelection::NeedDefinitionModule(expansion)
                    }
                    ModuleRoute::Lexical(actual) => {
                        selector = Some(actual);
                        if self.lexical_module.has_module_source() {
                            LocalTypePrefixSelection::ModuleTail
                        } else {
                            LocalTypePrefixSelection::NeedModulePublication
                        }
                    }
                }
            }
            super::PrefixResolution::NeedImport(id) => LocalTypePrefixSelection::NeedImport(id),
            super::PrefixResolution::NeedDeclaration(id) => {
                LocalTypePrefixSelection::NeedDeclarationOccurrence(id)
            }
            super::PrefixResolution::NeedOwnerGeneric => {
                LocalTypePrefixSelection::NeedOwnerGenericOccurrence
            }
        };
        Ok((selected, selector))
    }
    pub(crate) fn type_prefix(
        self: &Arc<Self>,
        db: &mut Database,
        node: crate::ir::pc::NodeId,
    ) -> QueryPoll<LocalTypePrefix> {
        self.type_prefix_with(db, node, false)
    }
    /// The declaration a generic-applied written Type (`Opt<i32>`) names,
    /// selected by the same prefix/declaration kernel as any Type occurrence;
    /// its arguments are separate indexed occurrences of their own.
    pub(crate) fn generic_base_body_type(
        self: &Arc<Self>,
        db: &mut Database,
        node: crate::ir::pc::NodeId,
    ) -> QueryPoll<type_declarations::BodyTypeOutcome> {
        let prefix = match self.type_prefix_with(db, node, true) {
            QueryPoll::Ready(Ok(prefix)) => prefix,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        type_declarations::prepare(db, prefix)
    }
    fn type_prefix_with(
        self: &Arc<Self>,
        db: &mut Database,
        node: crate::ir::pc::NodeId,
        generic_base: bool,
    ) -> QueryPoll<LocalTypePrefix> {
        match self.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        use crate::frontend::source_body_context::type_occurrences::{
            BodyTypeOrigin, BodyTypeSegment, BodyTypeShape,
        };
        let occurrence = match self.context.body_type_occurrence(db, node) {
            QueryPoll::Ready(Ok(Some(occurrence))) => occurrence,
            QueryPoll::Ready(Ok(None)) => {
                return QueryPoll::Ready(Err(error("entered Type occurrence absent")));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let mut module_selector = None;
        let answer = (|| -> Result<LocalTypePrefixSelection, Diagnostic> {
            if matches!(
                occurrence.origin(),
                BodyTypeOrigin::NeedGeneratedBoundary | BodyTypeOrigin::NeedRetainedOccurrence
            ) || (occurrence.shape() != BodyTypeShape::Named
                && !(generic_base && occurrence.shape() == BodyTypeShape::NeedGenericArguments))
            {
                return Ok(LocalTypePrefixSelection::NeedSource(
                    occurrence.next_authority_demand(),
                ));
            }
            let [BodyTypeSegment::Name(identifier)] = occurrence.segments() else {
                return Ok(LocalTypePrefixSelection::QualifiedTail);
            };
            if occurrence.absolute() {
                return Ok(LocalTypePrefixSelection::QualifiedTail);
            }
            if let Some(key) = self.missing_expansions.first() {
                return Ok(LocalTypePrefixSelection::NeedExpansionDefinition(*key));
            }
            let (root, _) = self.entered.local_hygiene();
            let context = self
                .context
                .source()
                .identifier_context_or_original(identifier.range(), root)?;
            // The cached source occurrence is reselected above under its real
            // source/currentness dependencies. No caller name/context is trusted.
            if identifier.raw_context().is_some_and(|raw| raw != context) {
                return Err(error("cached Type occurrence context differs"));
            }
            let site = OrdinaryNameSite {
                source: self.entered.parent.source(),
                body: occurrence.body(),
                at: u64::from(identifier.range().start),
                name: identifier.name(),
                context,
                namespace: crate::source::manifest::Namespace::Type,
            };
            let found = self.adapter().lookup_prepared_type_prefix(
                &site,
                &self.type_names,
                identifier.edition(),
            )?;
            let (selection, selector) = self.select_type_prefix(found, context)?;
            module_selector = selector;
            Ok(selection)
        })();
        QueryPoll::Ready(answer.and_then(|selection| {
            let use_name = if let Some(BodyTypeSegment::Name(name)) = occurrence.segments().last() {
                let (root, _) = self.entered.local_hygiene();
                Some(LocalTypeUse {
                    name: name.name().to_owned(),
                    range: name.range(),
                    context: self
                        .context
                        .source()
                        .identifier_context_or_original(name.range(), root)?,
                    edition: name.edition(),
                })
            } else {
                None
            };
            let mut proof = LocalTypePrefix {
                plan: Arc::clone(self),
                node: Some(node),
                header: None,
                selection,
                use_name,
                module_selector,
                fingerprint: Digest([0; 32]),
            };
            proof
                .exclusive_metadata_bytes()?
                .checked_add(self.exclusive_bytes)
                .ok_or_else(crate::runtime::arena::resource)?;
            proof.fingerprint = StableEncode::fingerprint(&proof);
            Ok(proof)
        }))
    }
    pub(crate) fn module_scope(&self) -> Option<&crate::syntax::outline::ScopeKey> {
        // Membership only. A use must consume type_prefix/module_selector,
        // not turn this retained module row into a hygienic lookup decision.
        Some(self.lexical_module.scope())
    }
    /// Shared query allocation, retained independently of its outer memo.
    pub(crate) fn lexical_outline_root(&self) -> &Arc<crate::syntax::outline::Outline> {
        self.lexical_module.outline_root()
    }
    pub(crate) fn lexical_outline_key(&self) -> &crate::db::key::OutlineKey {
        self.lexical_module.key()
    }
    pub(crate) fn context(&self) -> &Arc<PreparedBodyContext> {
        &self.context
    }
    /// Actual physical Arc owners, solely for allocation/retirement accounting.
    /// The entered lookup adapter can retain the same context as the plan.
    pub(crate) fn context_roots(&self) -> impl Iterator<Item = &Arc<PreparedBodyContext>> {
        std::iter::once(&self.context)
            .chain(self.entered.body_context.iter())
            .chain(std::iter::once(self.pattern_occurrences.source()))
    }
    pub(crate) fn source_identity(&self) -> Digest {
        self.source_identity
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        self.exclusive_bytes
    }
    pub(crate) fn tables_root(&self) -> &Arc<SourceLocalTables> {
        &self.tables
    }
    pub(crate) fn allocation_roots(&self) -> SourceLocalPlanRoots<'_> {
        SourceLocalPlanRoots {
            entered: &self.entered,
        }
    }
    /// This is an exact allocation/environment comparison for deduplication,
    /// not an admission, rebind API or semantic cache key.
    pub(crate) fn same_environment(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.tables, &other.tables) && self.entered.same_environment(&other.entered)
    }
    fn adapter(&self) -> SourceLocalIndex<'_> {
        SourceLocalIndex {
            entered: &self.entered,
            tables: Arc::clone(&self.tables),
        }
    }
    /// Cold diagnostic observation only. This API cannot return a completed
    /// occurrence proof: publication and declaration occurrence joins are held.
    pub(crate) fn inspect_prefix_metadata(
        &self,
        db: &mut Database,
        site: OrdinaryNameSite<'_>,
        names: &NameNamespaces,
    ) -> QueryPoll<LocalPrefixMetadata> {
        match self.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        if site.body != &self.entered.body
            || site.source.canonical().as_ref().ok() != Some(self.entered.parent.source())
        {
            return QueryPoll::Ready(Err(error("owned local observation source/body differs")));
        }
        let mut demands = vec![
            LocalPlanAuthorityDemand::CurrentEnvironmentPublication,
            LocalPlanAuthorityDemand::ExactEnteredOccurrence,
            LocalPlanAuthorityDemand::DeclarationNameContext,
            LocalPlanAuthorityDemand::ModuleNamespacePublication,
        ];
        if !self.missing_expansions.is_empty() {
            demands.extend(
                self.missing_expansions
                    .iter()
                    .copied()
                    .map(LocalPlanAuthorityDemand::ExpansionDefinition),
            );
            return QueryPoll::Ready(Ok(LocalPrefixMetadata {
                lookup: None,
                demands,
            }));
        }
        QueryPoll::Ready(
            self.adapter()
                .lookup_ordinary_prefix(&site, names)
                .map(|lookup| LocalPrefixMetadata { lookup, demands }),
        )
    }
}

/// Join actual selected expansion records to retained definition metadata. Raw
/// context marks provide neither the definition identity nor its owning unit.
fn expansion_definitions(
    entered: &EnteredMacroIndex<'_>,
    context: &PreparedBodyContext,
) -> Result<(Vec<(Key, MacroDefinition)>, Vec<Key>), Diagnostic> {
    let mut known = BTreeMap::new();
    let mut missing = BTreeSet::new();
    for input in context.source().retained_inputs() {
        match input.as_ref() {
            InputValue::RetainedSyntax(unit) => {
                for (key, definition) in unit.prepared().expansion_definitions() {
                    if known.insert(Key(key.0), definition.clone()).is_some() {
                        return Err(error("duplicate retained expansion definition"));
                    }
                }
            }
            InputValue::GeneratedSyntax(unit) => {
                let mut join =
                    |record: &crate::expand::record::ExpansionRecord| -> Result<(), Diagnostic> {
                        let key = Key(record.key().fingerprint().0);
                        let selected = match record.key().macro_identity {
                            MacroIdentity::Declaration { definition }
                            | MacroIdentity::Builtin { definition } => {
                                entered.retained_definition(definition)?
                            }
                            MacroIdentity::Procedural { .. }
                            | MacroIdentity::BuiltinDerive { .. } => None,
                        };
                        if let Some(definition) = selected {
                            if definition.definition_edition != record.key().definition_edition {
                                return Err(error(
                                    "expansion record and retained definition edition differ",
                                ));
                            }
                            if known
                                .insert(key, definition.clone())
                                .is_some_and(|old| old != definition)
                            {
                                return Err(error(
                                    "expansion record definition metadata conflicts",
                                ));
                            }
                        } else {
                            missing.insert(key);
                        }
                        Ok(())
                    };
                join(unit.items.record())?;
                if let Some(boundary) = context.generated_boundary() {
                    for parent in boundary.generated_roots() {
                        join(parent.record())?;
                    }
                }
            }
            _ => {}
        }
    }
    known
        .len()
        .checked_add(missing.len())
        .ok_or_else(crate::runtime::arena::resource)?;
    Ok((known.into_iter().collect(), missing.into_iter().collect()))
}

#[cfg(test)]
#[path = "local_type_prefix_tests.rs"]
mod tests;

impl std::fmt::Debug for PreparedSourceLocalPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedSourceLocalPlan")
            .field("body", self.context.source().index().body())
            .field("source_identity", &self.source_identity)
            .finish()
    }
}

// One finalizer for actual header and body Type occurrences. The prefix owns
// the source spelling/context; callers cannot provide a primitive name.
impl LocalTypePrefix {
    pub(crate) fn prepare_named(
        self,
        db: &mut Database,
    ) -> QueryPoll<crate::frontend::source_types::NamedTypePreparation> {
        self.prepare_named_with(db, false)
    }
    /// The base of a written generic application (`Option<u8>`): a selected
    /// generic declaration names its nominal, the written arguments applying.
    pub(crate) fn prepare_named_base(
        self,
        db: &mut Database,
    ) -> QueryPoll<crate::frontend::source_types::NamedTypePreparation> {
        self.prepare_named_with(db, true)
    }
    fn prepare_named_with(
        self,
        db: &mut Database,
        generic_base: bool,
    ) -> QueryPoll<crate::frontend::source_types::NamedTypePreparation> {
        use crate::frontend::{
            current_module_names::ModuleNameCompletion,
            signature_names::SignatureName,
            source_types::{NamedTypePreparation, SourceType},
        };
        macro_rules! ready {
            ($value:expr) => {
                match $value {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        ready!(self.retained_plan().require_current(db));
        if let LocalTypePrefixSelection::Parameter(parameter) = self.selection() {
            return QueryPoll::Ready(Ok(NamedTypePreparation::Parameter(*parameter)));
        }
        if matches!(self.selection(), LocalTypePrefixSelection::ModuleTail) {
            if let Some(observation) = ready!(self.module_name(db)) {
                if observation.completion() == ModuleNameCompletion::Complete
                    && matches!(observation.lookup(), Some(NameLookup::Absent))
                {
                    let Some(name) = self.use_name() else {
                        return QueryPoll::Ready(Err(error("named Type prefix occurrence absent")));
                    };
                    let classified = match crate::frontend::signature_names::classify_completed_type_lookup(
                        name.name(), name.edition(), NameLookup::Absent,
                    ) {
                        Ok(value) => value,
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    };
                    return match classified {
                        SignatureName::Primitive(primitive) => QueryPoll::Ready(
                            SourceType::selected_primitive(primitive).map(NamedTypePreparation::Ready),
                        ),
                        // The builtin unsized `str`, classified by the same
                        // completed lookup as every builtin Type name.
                        SignatureName::Str => QueryPoll::Ready(
                            SourceType::selected_str().map(NamedTypePreparation::Ready),
                        ),
                        other => QueryPoll::Ready(Ok(NamedTypePreparation::Name(other))),
                    };
                }
            }
        }
        match ready!(type_declarations::prepare(db, self)) {
            type_declarations::BodyTypeOutcome::Ready(proof) => {
                QueryPoll::Ready(Ok(NamedTypePreparation::Ready(proof.source_type().clone())))
            }
            type_declarations::BodyTypeOutcome::Pending(
                type_declarations::BodyTypeDemand::GenericArguments(identity),
            ) if generic_base => QueryPoll::Ready(Ok(NamedTypePreparation::Alias(
                crate::frontend::type_aliases::AliasTarget::Nominal(identity),
            ))),
            type_declarations::BodyTypeOutcome::Pending(demand) => QueryPoll::Ready(Err(
                Diagnostic::new("LRR-E-TYPE-CONTEXT", format!("Type requires actual source authority: {demand:?}")),
            )),
        }
    }
}
impl PreparedSourceLocalPlan {
    pub(crate) fn prepare_named_body_type(
        self: &Arc<Self>, db: &mut Database, node: crate::ir::pc::NodeId,
    ) -> QueryPoll<crate::frontend::source_types::NamedTypePreparation> {
        match self.type_prefix(db, node) {
            QueryPoll::Ready(Ok(prefix)) => prefix.prepare_named(db),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}
