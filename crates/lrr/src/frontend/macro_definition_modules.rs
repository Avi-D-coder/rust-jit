//! Selected opaque expansion -> actual source declaration module. This receipt
//! does not adjust a syntax context or grant a final Type/member lookup.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
    },
    expand::{
        identity::MacroIdentity, scopes::MacroDefinition, source_discovery::SealedMacroExportView,
    },
    frontend::{
        source_locals::owned_plan::{
            LocalTypePrefix, LocalTypePrefixSelection, PreparedSourceLocalPlan,
        },
        source_macros::SourceMacroIndex,
    },
    source::changes::{PreparedSourceView, SourceSelection},
    syntax::{
        outline::ScopeKey,
        spans::SourceRange,
        token_wire::{Key, NameContextKey},
    },
};
use std::sync::Arc;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-DEFINITION-MODULE", message.into())
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum DefinitionModuleDemand {
    OpaqueTypeOccurrence,
    CurrentExpansion(Key),
    SourceDeclaration(Digest),
    OwningSource(Digest),
    ConfiguredOwnerDependency(Digest),
    ConfiguredOwnerModule(Digest),
}
pub(crate) enum DefinitionModuleOutcome {
    Ready(SelectedMacroDefinitionModule),
    Pending(DefinitionModuleDemand),
}
/// Compact source proof only. Provider/parser/catalogue roots of a macro-only
/// dependency are not retained or promoted into live runtime source storage.
pub(crate) struct SelectedMacroDefinitionModule {
    plan: Arc<PreparedSourceLocalPlan>,
    prefix: Digest,
    expansion: Key,
    record: Digest,
    definition: MacroDefinition,
    source: SourceSelection,
    scope: ScopeKey,
    declaration: SourceRange,
    header: Digest,
    context: Key,
    comparison: NameContextKey,
    dependencies: Vec<DependencyStamp>,
    identity: Digest,
    bytes: usize,
}
struct SelectedExpansion {
    expansion: Key,
    definition: Digest,
    edition: u16,
    mark_unit: Digest,
    mark_edition: u16,
    record: Digest,
    seal: Arc<InputValue>,
    dependency: DependencyStamp,
}
impl SelectedMacroDefinitionModule {
    /// Current ordinary source owner must be in the actual completed discovery
    /// closure. A separately captured module with the same spelling is not it.
    pub(crate) fn capture_source(
        db: &mut Database,
        prefix: &LocalTypePrefix,
        source: &PreparedSourceView,
        index: &SourceMacroIndex,
    ) -> QueryPoll<DefinitionModuleOutcome> {
        let selected = match select(db, prefix) {
            QueryPoll::Ready(Ok(Ok(selected))) => selected,
            QueryPoll::Ready(Ok(Err(demand))) => return pending(demand),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        Self::capture_selected(
            db,
            prefix.retained_plan(),
            prefix.fingerprint(),
            source,
            index,
            selected,
        )
    }
    fn capture_selected(
        db: &mut Database,
        plan: &Arc<PreparedSourceLocalPlan>,
        occurrence: Digest,
        source: &PreparedSourceView,
        index: &SourceMacroIndex,
        selected: SelectedExpansion,
    ) -> QueryPoll<DefinitionModuleOutcome> {
        let InputValue::DiscoverySeal(seal) = selected.seal.as_ref() else {
            unreachable!("private selected seal")
        };
        if !seal.sources.contains(source.selection()) {
            return pending(DefinitionModuleDemand::OwningSource(selected.definition));
        }
        let current = match db.prepared_source(source.selection()) {
            Ok(Some(current)) if source.same_publication(&current) => current,
            Ok(_) => {
                return QueryPoll::Ready(Err(fault(
                    "selected declaration source publication changed",
                )));
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let result = (|| {
            let mut dependencies = Vec::new();
            dependencies
                .try_reserve_exact(3)
                .map_err(|_| crate::runtime::arena::resource())?;
            for key in [
                InputKey::CapturedSource(current.selection().location_identity()),
                InputKey::CrateCfg(current.selection().outline.crate_key),
            ] {
                let value = db
                    .input(&key)
                    .ok_or_else(|| fault("selected declaration source dependency absent"))?;
                let exact = match (&key, value.as_ref()) {
                    (InputKey::CapturedSource(_), InputValue::CapturedSource(actual)) => {
                        actual == current.selection()
                    }
                    (InputKey::CrateCfg(_), InputValue::CrateCfg(cfg)) => {
                        cfg.fingerprint() == current.selection().outline.cfg
                    }
                    _ => false,
                };
                if !exact {
                    return Err(fault("selected declaration source/cfg dependency changed"));
                }
                let fingerprint = db
                    .published_input(&key)
                    .ok_or_else(|| fault("selected declaration source dependency disappeared"))?
                    .1;
                dependencies.push(DependencyStamp::Input { key, fingerprint });
            }
            Self::finish(plan, occurrence, source, index, selected, dependencies)
        })();
        QueryPoll::Ready(result)
    }
    /// A raw facet is deliberately not accepted. The completed view establishes
    /// its actual source/provider selection, but does not yet supply a genuine
    /// configured crate-root/module-parent ancestry receipt for Type lookup.
    pub(crate) fn capture_macro_export(
        db: &mut Database,
        prefix: &LocalTypePrefix,
        owner: &SealedMacroExportView<'_>,
    ) -> QueryPoll<DefinitionModuleOutcome> {
        match owner.revalidate(db) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let selected = match select(db, prefix) {
            QueryPoll::Ready(Ok(Ok(selected))) => selected,
            QueryPoll::Ready(Ok(Err(demand))) => return pending(demand),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let InputValue::DiscoverySeal(seal) = selected.seal.as_ref() else {
            unreachable!("private selected seal")
        };
        if seal.seal.key().unit_graph != owner.seal().key().unit_graph {
            return pending(DefinitionModuleDemand::ConfiguredOwnerDependency(
                selected.definition,
            ));
        }
        // Prove that the actual selected expansion used this configured owner;
        // an independently completed source facet is only a candidate otherwise.
        for dependency in &owner.dependencies()[..2] {
            let DependencyStamp::Input { key, fingerprint } = dependency else {
                return QueryPoll::Ready(Err(fault("configured macro owner dependency differs")));
            };
            if seal.compiler_inputs.get(key) != Some(fingerprint) {
                return pending(DefinitionModuleDemand::ConfiguredOwnerDependency(
                    selected.definition,
                ));
            }
        }
        // Even current candidate source + a complete local catalogue cannot
        // prove that a package file's supplied root scope is the actual module
        // in the configured crate. Do not promote that missing parent proof or
        // derive module ancestry from recorded_path/source.module_path.
        pending(DefinitionModuleDemand::ConfiguredOwnerModule(
            selected.definition,
        ))
    }
    fn finish(
        plan: &Arc<PreparedSourceLocalPlan>,
        occurrence: Digest,
        source: &PreparedSourceView,
        index: &SourceMacroIndex,
        selected: SelectedExpansion,
        mut dependencies: Vec<DependencyStamp>,
    ) -> Result<DefinitionModuleOutcome, Diagnostic> {
        let Some(definition) = index.definition(selected.definition) else {
            return Ok(DefinitionModuleOutcome::Pending(
                DefinitionModuleDemand::SourceDeclaration(selected.definition),
            ));
        };
        if definition.definition_edition != selected.edition
            || definition.defining_unit != selected.mark_unit
            || definition.definition_edition != selected.mark_edition
        {
            return Err(fault(
                "selected record/context and declaration unit/edition differ",
            ));
        }
        let Some(declaration) = index.source_definition_module(source, definition)? else {
            return Ok(DefinitionModuleOutcome::Pending(
                DefinitionModuleDemand::SourceDeclaration(selected.definition),
            ));
        };
        let comparison = index.hygiene_facts().name_item(declaration.context())?;
        dependencies.push(selected.dependency);
        let mut receipt = Self {
            plan: Arc::clone(plan),
            prefix: occurrence,
            expansion: selected.expansion,
            record: selected.record,
            definition: definition.clone(),
            source: source.selection().clone(),
            scope: declaration.item().scope.clone(),
            declaration: declaration.item().declaration_span,
            header: declaration.item().header.fingerprint(),
            context: declaration.context(),
            comparison,
            dependencies,
            identity: Digest([0; 32]),
            bytes: 0,
        };
        receipt.bytes = receipt.measure_bytes()?;
        receipt.identity = receipt.fingerprint();
        Ok(DefinitionModuleOutcome::Ready(receipt))
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn source(&self) -> &SourceSelection {
        &self.source
    }
    pub(crate) fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub(crate) fn definition(&self) -> &MacroDefinition {
        &self.definition
    }
    pub(crate) fn declaration_context(&self) -> Key {
        self.context
    }
    pub(crate) fn declaration_comparison(&self) -> NameContextKey {
        self.comparison
    }
    pub(crate) fn dependencies(&self) -> &[DependencyStamp] {
        &self.dependencies
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        self.bytes
    }
    fn measure_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.source
                    .metadata_bytes()?
                    .checked_sub(std::mem::size_of::<SourceSelection>())
                    .ok_or_else(crate::runtime::arena::resource)?,
            )
            .and_then(|n| {
                n.checked_add(
                    self.scope
                        .module_path
                        .capacity()
                        .checked_mul(std::mem::size_of::<String>())?,
                )
            })
            .and_then(|n| {
                self.scope
                    .module_path
                    .iter()
                    .try_fold(n, |n, part| n.checked_add(part.capacity()))
            })
            .and_then(|n| {
                n.checked_add(
                    self.dependencies
                        .capacity()
                        .checked_mul(std::mem::size_of::<DependencyStamp>())?,
                )
            })
            .ok_or_else(crate::runtime::arena::resource)?;
        for dependency in &self.dependencies {
            bytes = bytes
                .checked_add(dependency.heap_bytes())
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        Ok(bytes)
    }
    /// Cold source-proof validation only. Final pinned scope adjustment and
    /// Type/name/member authority require their own common query dependencies.
    pub(crate) fn revalidate(&self, db: &mut Database) -> QueryPoll<()> {
        match self.plan.context().require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            other => return other,
        }
        for dependency in &self.dependencies {
            match dependency {
                DependencyStamp::Input { key, fingerprint } => {
                    if db.input(key).is_none()
                        || !matches!(db.published_input(key), Some((_, actual)) if actual == *fingerprint)
                    {
                        return QueryPoll::Ready(Err(fault(
                            "selected declaration module input changed",
                        )));
                    }
                }
                DependencyStamp::Query { key, fingerprint } => {
                    match db.fetch(&SyntaxProvider, key.clone()) {
                        QueryPoll::Ready(Ok(outcome)) if outcome.fingerprint == *fingerprint => {}
                        QueryPoll::Ready(Ok(_)) => {
                            return QueryPoll::Ready(Err(fault(
                                "selected declaration module query changed",
                            )));
                        }
                        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    }
                }
            }
        }
        QueryPoll::Ready(Ok(()))
    }
}
impl StableEncode for SelectedMacroDefinitionModule {
    const DOMAIN: &'static str = "lrr/v1/selected-macro-declaration-module";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.prefix);
        e.digest(self.plan.source_identity());
        e.digest(Digest(self.expansion.0));
        e.digest(self.record);
        e.digest(self.definition.identity);
        e.digest(self.definition.defining_unit);
        e.u16(self.definition.definition_edition);
        e.string(self.source.path.as_str());
        e.record(&self.source.outline);
        e.record(&self.scope);
        e.u32(self.declaration.start);
        e.u32(self.declaration.end);
        e.digest(self.header);
        e.digest(Digest(self.context.0));
        e.digest(Digest(self.comparison.0.0));
        e.sequence(&self.dependencies, |e, dependency| e.record(dependency));
    }
}
fn pending(demand: DefinitionModuleDemand) -> QueryPoll<DefinitionModuleOutcome> {
    QueryPoll::Ready(Ok(DefinitionModuleOutcome::Pending(demand)))
}
fn select(
    db: &mut Database,
    prefix: &LocalTypePrefix,
) -> QueryPoll<Result<SelectedExpansion, DefinitionModuleDemand>> {
    let LocalTypePrefixSelection::NeedDefinitionModule(expansion) = prefix.selection() else {
        return QueryPoll::Ready(Ok(Err(DefinitionModuleDemand::OpaqueTypeOccurrence)));
    };
    let Some(actual) = prefix.use_name() else {
        return QueryPoll::Ready(Err(fault("opaque module use occurrence absent")));
    };
    let answer = select_context(db, prefix.retained_plan(), actual.context());
    match &answer {
        QueryPoll::Ready(Ok(Ok(selected))) if selected.expansion != *expansion => {
            QueryPoll::Ready(Err(fault("opaque module prefix context differs")))
        }
        _ => answer,
    }
}
fn select_context(
    db: &mut Database,
    plan: &Arc<PreparedSourceLocalPlan>,
    raw: Key,
) -> QueryPoll<Result<SelectedExpansion, DefinitionModuleDemand>> {
    let context = plan.context();
    match context.require_current(db) {
        QueryPoll::Ready(Ok(())) => {}
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    }
    let result = (|| {
        let roots = plan.allocation_roots();
        let normalized = roots.hygiene().item(raw)?;
        let mark = roots.hygiene().context(normalized)?;
        let Some(expansion) = mark.expansion.filter(|_| mark.parent.is_some()) else {
            return Ok(Err(DefinitionModuleDemand::OpaqueTypeOccurrence));
        };
        // Admission already validates these immutable units. Keep the actual
        // record and its own seal, including outputs of entered body macros.
        let mut candidates = Vec::new();
        if let Some(boundary) = context.generated_boundary() {
            for output in
                std::iter::once(boundary.output()).chain(boundary.generated_roots().iter())
            {
                candidates.push((output.record_input(), boundary.seal().clone()));
            }
        }
        for input in context.source().retained_inputs() {
            match input.as_ref() {
                InputValue::RetainedSyntax(unit) => {
                    for output in unit.prepared().outputs() {
                        candidates.push((output.record_handle(), unit.seal().clone()));
                    }
                }
                InputValue::GeneratedSyntax(unit) => {
                    candidates.push((unit.items.record_input(), unit.seal.clone()));
                }
                _ => {}
            }
        }
        let mut records = candidates
            .iter()
            .filter(|(record, _)| record.key().fingerprint().0 == expansion.0);
        let Some((record, seal_key)) = records.next() else {
            return Ok(Err(DefinitionModuleDemand::CurrentExpansion(expansion)));
        };
        if records.any(|(other, _)| !record.same_record(other)) {
            return Err(fault("opaque module expansion record conflicts"));
        }
        let definition = match record.key().macro_identity {
            MacroIdentity::Declaration { definition }
            | MacroIdentity::BuiltinDerive { definition } => definition,
            _ => return Ok(Err(DefinitionModuleDemand::CurrentExpansion(expansion))),
        };
        let seal = db
            .input(&InputKey::DiscoverySeal(seal_key.clone()))
            .ok_or_else(|| fault("opaque module discovery input absent"))?;
        let InputValue::DiscoverySeal(input) = seal.as_ref() else {
            return Err(fault("opaque module discovery input differs"));
        };
        let descriptor = record.descriptor().fingerprint();
        if input.seal.key() != *seal_key
            || input
                .seal
                .input_dependencies()
                .binary_search(&(record.key().fingerprint(), descriptor))
                .is_err()
        {
            return Err(fault(
                "opaque module record is outside actual discovery closure",
            ));
        }
        Ok(Ok((
            SelectedExpansion {
                expansion,
                definition,
                edition: record.key().definition_edition,
                mark_unit: Digest(mark.defining_unit.0),
                mark_edition: match mark.edition {
                    crate::syntax::token_wire::Edition::E2015 => 2015,
                    crate::syntax::token_wire::Edition::E2018 => 2018,
                    crate::syntax::token_wire::Edition::E2021 => 2021,
                    crate::syntax::token_wire::Edition::E2024 => 2024,
                },
                record: descriptor,
                seal,
                // Filled using the actual query result below, never its input stamp.
                dependency: DependencyStamp::Query {
                    key: QueryKey::DiscoverySeal(seal_key.clone()),
                    fingerprint: Digest([0; 32]),
                },
            },
            seal_key.clone(),
        )))
    })();
    let (mut selected, seal) = match result {
        Ok(Ok(selected)) => selected,
        Ok(Err(demand)) => return QueryPoll::Ready(Ok(Err(demand))),
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    match db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(seal.clone())) {
        QueryPoll::Ready(Ok(value)) => {
            selected.dependency = DependencyStamp::Query {
                key: QueryKey::DiscoverySeal(seal),
                fingerprint: value.fingerprint,
            };
            QueryPoll::Ready(Ok(Ok(selected)))
        }
        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
    }
}

/// One immutable module selection shared by Type and Value name occurrences.
/// This is only source/module authority; the namespace answer is a separate
/// ordinary CurrentModuleName query with its relevant positive/negative reads.
pub(crate) struct PreparedDefinitionModule {
    plan: Arc<PreparedSourceLocalPlan>,
    context: Key,
    outcome: DefinitionModuleOutcome,
}
impl std::fmt::Debug for PreparedDefinitionModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedDefinitionModule")
            .field("context", &self.context)
            .field(
                "ready",
                &matches!(self.outcome, DefinitionModuleOutcome::Ready(_)),
            )
            .finish_non_exhaustive()
    }
}
impl PreparedDefinitionModule {
    pub(crate) fn selected(&self) -> Option<&SelectedMacroDefinitionModule> {
        match &self.outcome {
            DefinitionModuleOutcome::Ready(module) => Some(module),
            DefinitionModuleOutcome::Pending(_) => None,
        }
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn matches(&self, body: &crate::source::manifest::BodyKey, context: Key) -> bool {
        self.plan.context().source().index().body() == body && self.context == context
    }
    pub(crate) fn request(
        &self,
        name: &str,
        edition: u16,
        namespace: crate::source::manifest::Namespace,
    ) -> Result<Option<crate::db::key::CurrentModuleNameKey>, Diagnostic> {
        let DefinitionModuleOutcome::Ready(module) = &self.outcome else {
            return Ok(None);
        };
        let name = crate::frontend::names::owned_name(name, edition)?;
        Ok(Some(crate::db::key::CurrentModuleNameKey {
            source: module.source().outline.clone(),
            path: module.source().path.as_str().to_owned(),
            bucket: crate::db::key::BucketKey {
                scope: module.scope().clone(),
                namespace,
                name: name.as_str().to_owned(),
            },
        }))
    }
    pub(crate) fn prepare(
        db: &mut Database,
        body: &crate::source::manifest::BodyKey,
        context: Key,
    ) -> QueryPoll<Self> {
        let plan = match db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(body.clone())) {
            QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
                crate::db::value::QueryValue::SourceLocalPlan(plan) => Arc::clone(plan),
                _ => return QueryPoll::Ready(Err(fault("definition module local plan differs"))),
            },
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let outcome = match Self::select_owner(db, &plan, context) {
            QueryPoll::Ready(Ok(outcome)) => outcome,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready(Ok(Self {
            plan,
            context,
            outcome,
        }))
    }
    fn select_owner(
        db: &mut Database,
        plan: &Arc<PreparedSourceLocalPlan>,
        context: Key,
    ) -> QueryPoll<DefinitionModuleOutcome> {
        let selected = match select_context(db, plan, context) {
            QueryPoll::Ready(Ok(Ok(value))) => value,
            QueryPoll::Ready(Ok(Err(demand))) => return pending(demand),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let id = selected.definition;
        let Some(environment) = plan.environment() else {
            return pending(DefinitionModuleDemand::OwningSource(id));
        };
        let completed = Arc::clone(environment.macros().source_environment());
        let (Some(parse), Some(declaration)) = (
            db.input(&InputKey::DefinitionSource(id)),
            db.input(&InputKey::DeclarationContext(id)),
        ) else {
            return pending(DefinitionModuleDemand::SourceDeclaration(id));
        };
        let (InputValue::DefinitionSource(parse), InputValue::DeclarationContext(declaration)) =
            (parse.as_ref(), declaration.as_ref())
        else {
            return QueryPoll::Ready(Err(fault(
                "definition module source/context variants differ",
            )));
        };
        let crate::expand::source_discovery::SourceScopeOwner::Selected(source) =
            completed.scope_owner(&declaration.scope)
        else {
            return pending(DefinitionModuleDemand::OwningSource(id));
        };
        if source.selection().outline.parse != *parse {
            return QueryPoll::Ready(Err(fault("definition module owning source differs")));
        }
        let owner = match completed.sealed_body_macro_environment(db, source.selection()) {
            QueryPoll::Ready(Ok(owner)) => owner,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let mut occurrence = StableEncoder::new("lrr/v1/definition-module-context");
        occurrence.record(plan.context().source().index().body());
        occurrence.digest(Digest(context.0));
        SelectedMacroDefinitionModule::capture_selected(
            db,
            plan,
            occurrence.finish(),
            owner.source(),
            owner.index().source_index_root(),
            selected,
        )
    }
}
impl StableEncode for PreparedDefinitionModule {
    const DOMAIN: &'static str = "lrr/v1/prepared-definition-module";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(self.plan.context().source().index().body());
        e.digest(self.plan.source_identity());
        e.digest(Digest(self.context.0));
        match &self.outcome {
            DefinitionModuleOutcome::Ready(module) => {
                e.u32(0);
                e.record(module);
            }
            DefinitionModuleOutcome::Pending(demand) => {
                e.u32(1);
                match demand {
                    DefinitionModuleDemand::OpaqueTypeOccurrence => e.u32(0),
                    DefinitionModuleDemand::CurrentExpansion(key) => {
                        e.u32(1);
                        e.digest(Digest(key.0));
                    }
                    DefinitionModuleDemand::SourceDeclaration(key) => {
                        e.u32(2);
                        e.digest(*key);
                    }
                    DefinitionModuleDemand::OwningSource(key) => {
                        e.u32(3);
                        e.digest(*key);
                    }
                    DefinitionModuleDemand::ConfiguredOwnerDependency(key) => {
                        e.u32(4);
                        e.digest(*key);
                    }
                    DefinitionModuleDemand::ConfiguredOwnerModule(key) => {
                        e.u32(5);
                        e.digest(*key);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "macro_definition_module_tests.rs"]
mod tests;
