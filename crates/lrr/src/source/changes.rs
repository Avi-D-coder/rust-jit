//! In-memory source overlays: a validated batch publishes as one revision.
use super::snapshot::{Snapshot, SourceBlob, SourcePath};
use crate::core::diagnostic::Diagnostic;
use std::collections::BTreeSet;

/// A complete source/module context selected by the immutable source resolver.
/// Includes used as bytes are not parsing contexts. Generated contexts join this
/// same publication after expansion has established their declaration universe.
#[derive(Clone)]
pub struct SourceUnitInput {
    pub path: SourcePath,
    pub crate_key: crate::core::digest::Digest,
    pub module_path: Vec<String>,
    pub profile: crate::core::profile::ProfileIdentity,
    pub cfg: super::snapshot::CfgSet,
    pub blob: SourceBlob,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct UnitLocation {
    path: SourcePath,
    crate_key: crate::core::digest::Digest,
    module_path: Vec<String>,
}
pub(crate) struct PublishedFile {
    pub(crate) key: crate::db::key::OutlineKey,
    pub(crate) parsed: std::sync::Arc<crate::syntax::parse::ParsedFile>,
    pub(crate) outline: std::sync::Arc<crate::syntax::outline::Outline>,
}

/// All outlines are refreshed before the database revision can be published.
/// The current source universe owns only its own parse/outline handles; old
/// contexts and deleted declarations disappear from the next publication.
pub(crate) struct PreparedSourceState {
    _module_origin: Option<std::sync::Arc<super::module_view::CapturedModuleSourceView>>,
    _ordinary_module_origin:
        Option<std::sync::Arc<super::ordinary_module::OrdinaryModuleSourceView>>,
    pub(crate) inputs: std::collections::BTreeMap<
        crate::db::dependency::InputKey,
        crate::db::dependency::InputValue,
    >,
    pub(crate) files: std::collections::BTreeMap<UnitLocation, PublishedFile>,
    pub(crate) parses: std::collections::BTreeMap<
        crate::syntax::parse::ParseKey,
        std::sync::Arc<crate::syntax::parse::ParsedFile>,
    >,
    pub(crate) outlines: std::collections::BTreeMap<
        crate::db::key::OutlineKey,
        std::sync::Arc<crate::syntax::outline::Outline>,
    >,
}
impl PreparedSourceState {
    pub(crate) fn file_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>();
        for (location, file) in &self.files {
            for n in [
                64 + std::mem::size_of::<UnitLocation>() + std::mem::size_of::<PublishedFile>(),
                location.path.as_str().len(),
                location.module_path.capacity() * std::mem::size_of::<String>(),
                file.key.module_path.capacity() * std::mem::size_of::<String>(),
                file.key.parse.profile.semantics.capacity(),
            ] {
                bytes = bytes
                    .checked_add(n)
                    .ok_or_else(|| publication_fault("prepared metadata accounting overflow"))?;
            }
            for module in location.module_path.iter().chain(&file.key.module_path) {
                bytes = bytes
                    .checked_add(module.capacity())
                    .ok_or_else(|| publication_fault("prepared metadata accounting overflow"))?;
            }
        }
        for key in self.parses.keys() {
            bytes = bytes
                .checked_add(
                    64 + std::mem::size_of_val(key)
                        + std::mem::size_of::<std::sync::Arc<crate::syntax::parse::ParsedFile>>()
                        + key.profile.semantics.capacity(),
                )
                .ok_or_else(|| publication_fault("prepared parse index accounting overflow"))?;
        }
        for key in self.outlines.keys() {
            bytes = bytes
                .checked_add(
                    64 + std::mem::size_of_val(key)
                        + std::mem::size_of::<std::sync::Arc<crate::syntax::outline::Outline>>()
                        + key.parse.profile.semantics.capacity()
                        + key.module_path.capacity() * std::mem::size_of::<String>(),
                )
                .ok_or_else(|| publication_fault("prepared outline index accounting overflow"))?;
            for module in &key.module_path {
                bytes = bytes.checked_add(module.capacity()).ok_or_else(|| {
                    publication_fault("prepared outline index accounting overflow")
                })?;
            }
        }
        Ok(bytes)
    }
}
/// Exact immutable source context, including profile and cfg, not content alone.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceSelection {
    pub path: SourcePath,
    pub outline: crate::db::key::OutlineKey,
}
impl SourceSelection {
    /// Stable captured file location; content/profile/cfg changes update its input value.
    pub fn location_identity(&self) -> crate::core::digest::Digest {
        let mut e = crate::core::digest::StableEncoder::new("lrr/v2/captured-source-location");
        e.string(self.path.as_str());
        e.digest(self.outline.crate_key);
        e.sequence(&self.outline.module_path, |e, s| e.string(s));
        e.finish()
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.path.as_str().len())
            .and_then(|n| n.checked_add(self.outline.parse.profile.semantics.capacity()))
            .and_then(|n| {
                self.outline
                    .module_path
                    .capacity()
                    .checked_mul(std::mem::size_of::<String>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(|| publication_fault("captured source metadata overflow"))?;
        for part in &self.outline.module_path {
            bytes = bytes
                .checked_add(part.capacity())
                .ok_or_else(|| publication_fault("captured source metadata overflow"))?;
        }
        Ok(bytes)
    }
    pub(crate) fn canonical(&self) -> Result<Self, Diagnostic> {
        self.outline.parse.profile.validate_source()?;
        let module_path =
            crate::syntax::ast_adapter::semantic_module_path(&self.outline.module_path)
                .ok_or_else(|| publication_fault("invalid selected module context"))?;
        Ok(Self {
            path: self.path.clone(),
            outline: crate::db::key::OutlineKey {
                parse: self.outline.parse.clone(),
                crate_key: self.outline.crate_key,
                module_path,
                cfg: self.outline.cfg,
            },
        })
    }
}
pub enum SourceSelectionOutcome {
    Selected(PreparedSourceView),
    Absent,
    Changed { actual: SourceSelection },
}
/// Retains the immutable publication containing the selected parse/outline.
/// Holding this lease participates in existing prepared-source cache accounting.
pub struct PreparedSourceView {
    state: std::sync::Arc<PreparedSourceState>,
    location: UnitLocation,
    selection: SourceSelection,
}
impl PreparedSourceView {
    /// Share only the identity of this already prepared immutable publication.
    /// Registration never publishes its inputs or grants current source authority.
    pub(crate) fn retained_state_weak(&self) -> std::sync::Weak<PreparedSourceState> {
        std::sync::Arc::downgrade(&self.state)
    }
    pub(crate) fn from_retained_state(
        state: std::sync::Arc<PreparedSourceState>,
        expected: &SourceSelection,
    ) -> Result<Option<Self>, Diagnostic> {
        if expected.canonical()? != *expected {
            return Err(publication_fault("selected source is noncanonical"));
        }
        let location = UnitLocation {
            path: expected.path.clone(),
            crate_key: expected.outline.crate_key,
            module_path: expected.outline.module_path.clone(),
        };
        let Some(file) = state.files.get(&location) else {
            return Ok(None);
        };
        if file.key != expected.outline {
            return Ok(None);
        }
        Ok(Some(Self {
            state,
            location,
            selection: expected.clone(),
        }))
    }
    /// Replay only exact current original declaration facts from a genuine
    /// published scope certificate. Expansion/provider execution is forbidden.
    pub(crate) fn replay_published_macro_index(
        &self,
        input: &crate::db::seals::DiscoveryInput,
    ) -> Result<crate::frontend::source_macros::PublishedMacroIndex, Diagnostic> {
        crate::expand::source_declarative::SourceDeclarative::replay_scalar_pattern_source(
            self, input,
        )
    }
    pub(crate) fn from_module(
        origin: &std::sync::Arc<super::module_view::CapturedModuleSourceView>,
    ) -> Result<Self, Diagnostic> {
        Self::from_captured_module_parts(
            origin.capture(),
            origin.outline_handle(),
            origin.parsed_handle(),
            origin.caller(),
            Some(origin.clone()),
            None,
        )
    }
    pub(crate) fn from_ordinary_module(
        origin: &std::sync::Arc<super::ordinary_module::OrdinaryModuleSourceView>,
    ) -> Result<Self, Diagnostic> {
        Self::from_captured_module_parts(
            origin.capture(),
            origin.outline_handle(),
            origin.parsed_handle(),
            origin.declaration().source(),
            None,
            Some(origin.clone()),
        )
    }
    fn from_captured_module_parts(
        capture: &super::module_capture::AdoptedModuleCapture,
        outline: std::sync::Arc<crate::syntax::outline::Outline>,
        parsed: std::sync::Arc<crate::syntax::parse::ParsedFile>,
        caller: &Self,
        generated_origin: Option<std::sync::Arc<super::module_view::CapturedModuleSourceView>>,
        ordinary_origin: Option<std::sync::Arc<super::ordinary_module::OrdinaryModuleSourceView>>,
    ) -> Result<Self, Diagnostic> {
        use crate::{
            core::digest::StableEncode,
            db::{
                dependency::{InputKey, InputValue},
                key::OutlineKey,
            },
            syntax::outline::DeclarationContext,
        };
        use std::{collections::BTreeMap, sync::Arc};
        let selected = capture.record().selected()?;
        let cfg = caller.captured_macro_cfg()?.clone();
        let key = OutlineKey {
            parse: parsed.key(),
            crate_key: outline.root_scope.crate_key,
            module_path: outline.root_scope.module_path.clone(),
            cfg: cfg.fingerprint(),
        };
        let location = UnitLocation {
            path: selected.location().path.clone(),
            crate_key: key.crate_key,
            module_path: key.module_path.clone(),
        };
        let selection = SourceSelection {
            path: location.path.clone(),
            outline: key.clone(),
        };
        if selection.canonical()? != selection
            || selected.location().unit != key.crate_key
            || outline.items.len() > 1_000_000
        {
            return Err(publication_fault(
                "module source declaration context mismatch or limit",
            ));
        }
        let mut inputs = BTreeMap::new();
        insert_input(
            &mut inputs,
            InputKey::SourceBlob(parsed.content),
            InputValue::SourceBlob(selected.blob().clone()),
        )?;
        insert_input(
            &mut inputs,
            InputKey::CrateCfg(key.crate_key),
            InputValue::CrateCfg(cfg),
        )?;
        insert_input(
            &mut inputs,
            InputKey::CapturedSource(selection.location_identity()),
            InputValue::CapturedSource(selection.clone()),
        )?;
        insert_input(
            &mut inputs,
            InputKey::SourceDependencies,
            InputValue::SourceDependencies(caller.source_dependencies()?),
        )?;
        for item in &outline.items {
            let id = item.def_key.fingerprint();
            insert_input(
                &mut inputs,
                InputKey::DefinitionHeader(id),
                InputValue::DefinitionHeader(item.header.clone()),
            )?;
            insert_input(
                &mut inputs,
                InputKey::DefinitionSource(id),
                InputValue::DefinitionSource(key.parse.clone()),
            )?;
            insert_input(
                &mut inputs,
                InputKey::DeclarationContext(id),
                InputValue::DeclarationContext(DeclarationContext {
                    scope: item.scope.clone(),
                    range: item.declaration_span,
                }),
            )?;
            if let Some(body) = &item.body {
                insert_input(
                    &mut inputs,
                    InputKey::DefinitionBody(id),
                    InputValue::DefinitionBody(body.clone()),
                )?;
            }
        }
        for (scope, members) in &outline.scopes {
            // An external declaration contributes a discovery frontier, not the
            // captured child's membership row. Its own projection supplies that row.
            if outline.textual_events.iter().any(|event| {
                event.scope == *scope
                    && matches!(
                        event.kind,
                        crate::syntax::outline::TextualEventKind::Producer {
                            kind: crate::syntax::outline::ProducerKind::ModuleSource,
                            ..
                        }
                    )
            }) {
                continue;
            }
            insert_input(
                &mut inputs,
                InputKey::ScopeMembers(scope.clone()),
                InputValue::ScopeMembers(members.clone()),
            )?;
        }
        let state = Arc::new(PreparedSourceState {
            _module_origin: generated_origin,
            _ordinary_module_origin: ordinary_origin,
            inputs,
            files: BTreeMap::from([(
                location.clone(),
                PublishedFile {
                    key: key.clone(),
                    parsed: parsed.clone(),
                    outline: outline.clone(),
                },
            )]),
            parses: BTreeMap::from([(key.parse.clone(), parsed)]),
            outlines: BTreeMap::from([(key, outline)]),
        });
        let view = Self {
            state,
            location,
            selection,
        };
        view.require_declaration_syntax()?;
        Ok(view)
    }
    pub(crate) fn prepared_inputs(
        &self,
    ) -> &std::collections::BTreeMap<
        crate::db::dependency::InputKey,
        crate::db::dependency::InputValue,
    > {
        &self.state.inputs
    }
    pub(crate) fn prepared_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.state.file_metadata_bytes()
    }
    /// Share only the immutable source lease; guest state and authority are not copied.
    pub fn share(&self) -> Self {
        Self {
            state: std::sync::Arc::clone(&self.state),
            location: self.location.clone(),
            selection: self.selection.clone(),
        }
    }
    pub(crate) fn same_publication(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.state, &other.state)
    }
    /// Root hygiene may be prepared only for physically original source. A
    /// generated publication must retain its completed expansion contexts.
    pub(crate) fn require_original_hygiene(&self) -> Result<(), Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("original source lease absent"))?;
        if file.parsed.generated_origin.is_some() {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "generated constructor requires its completed hygiene publication".into(),
            ));
        }
        Ok(())
    }
    pub fn selection(&self) -> &SourceSelection {
        &self.selection
    }
    /// A damaged declaration boundary blocks namespace discovery; diagnostics
    /// come from the pinned parser without forcing otherwise cold body errors.
    /// Pure nested declaration scan of the already captured parser/cfg input.
    pub(crate) fn declaration_inventory(
        &self,
        limits: crate::syntax::declaration_scan::DeclarationScanLimits,
    ) -> Result<
        std::sync::Arc<crate::syntax::declaration_scan::DeclarationScan>,
        crate::core::diagnostic::Diagnostic,
    > {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("declaration parser source missing"))?;
        let cfg = self.captured_macro_cfg()?;
        let scan = file.parsed.scan_declarations(
            self.selection.path.clone(),
            self.outline().root_scope.clone(),
            cfg,
            limits,
        )?;
        if scan.parse() != &self.selection.outline.parse || scan.cfg() != self.selection.outline.cfg
        {
            return Err(publication_fault(
                "captured nested declaration identity differs",
            ));
        }
        Ok(std::sync::Arc::new(scan))
    }

    /// Validate only after resolution selects the compiler Test facet.
    pub(crate) fn validate_compiler_test_target(
        &self,
        scan: &crate::syntax::declaration_scan::DeclarationScan,
        producer: crate::core::digest::Digest,
    ) -> Result<(), Diagnostic> {
        if !self.attribute_targets_free_function(scan, producer)? {
            return Err(Diagnostic::new(
                "LRR-E-TEST-DISCOVERY",
                "compiler Test attribute requires a free function".into(),
            ));
        }
        Ok(())
    }

    /// Exact syntax ownership fact; never selects or executes an attribute.
    pub(crate) fn attribute_targets_free_function(
        &self,
        scan: &crate::syntax::declaration_scan::DeclarationScan,
        producer: crate::core::digest::Digest,
    ) -> Result<bool, Diagnostic> {
        if scan.parse() != &self.selection.outline.parse
            || scan.cfg() != self.selection.outline.cfg
            || scan.path() != &self.selection.path
            || scan.scope() != &self.outline().root_scope
            || scan.generated_origin().is_some()
        {
            return Err(publication_fault("compiler Test source attachment differs"));
        }
        let row = scan
            .producers()
            .iter()
            .find(|row| {
                row.identity == producer
                    && row.kind
                        == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            })
            .ok_or_else(|| publication_fault("compiler Test attribute producer absent"))?;
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("compiler Test parser source missing"))?;
        file.parsed.attribute_targets_free_function(row.range)
    }

    pub(crate) fn require_declaration_syntax(&self) -> Result<(), Diagnostic> {
        let outline = self.outline();
        let unreliable = outline.scopes.values().any(|scope| {
            scope
                .blockers
                .iter()
                .any(|b| b.kind == crate::syntax::outline::BlockerKind::ParseBoundary)
        });
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("declaration parser source missing"))?;
        let diagnostic = file.parsed.errors.iter().find(|error| {
            !outline
                .items
                .iter()
                .filter_map(|item| item.body.as_ref())
                .any(|body| {
                    error.range.start >= body.range.start && error.range.end <= body.range.end
                })
        });
        if diagnostic.is_none() && !unreliable {
            return Ok(());
        }
        let message = if let Some(error) = diagnostic {
            let mut end = error.message.len().min(4096);
            while !error.message.is_char_boundary(end) {
                end -= 1;
            }
            format!(
                "Rust {} declaration parse error at bytes {}..{}: {}",
                outline.profile.edition,
                error.range.start,
                error.range.end,
                &error.message[..end]
            )
        } else {
            format!(
                "Rust {} declaration parse boundary is unreliable",
                outline.profile.edition
            )
        };
        Err(Diagnostic::new("LRR-E-PARSE-BLOCKS-ENTRY", message))
    }
    pub(crate) fn macro_call_for_opening_at(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<Option<crate::syntax::macro_calls::SourceMacroCall>, Diagnostic> {
        self.state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("selected invocation source missing"))?
            .parsed
            .macro_call_for_opening_at(range)
    }
    pub(crate) fn macro_call_at(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<crate::syntax::macro_calls::SourceMacroCall, Diagnostic> {
        self.state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("selected invocation source missing"))?
            .parsed
            .macro_call_at(range)
    }
    pub(crate) fn macro_prelude_policy(
        &self,
    ) -> Result<crate::syntax::macro_prelude::MacroPreludePolicy, Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("macro prelude source missing"))?;
        let Some(crate::db::dependency::InputValue::CrateCfg(cfg)) =
            self.state
                .inputs
                .get(&crate::db::dependency::InputKey::CrateCfg(
                    self.selection.outline.crate_key,
                ))
        else {
            return Err(publication_fault("macro prelude cfg missing"));
        };
        file.parsed.macro_prelude_policy(cfg)
    }
    pub(crate) fn require_module_predicate_call(
        &self,
        body: &crate::syntax::outline::BodyHandle,
        index: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
        name: &str,
    ) -> Result<(), Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("selected predicate source missing"))?;
        file.parsed
            .require_module_predicate_call(body, index, key, name)
    }
    pub(crate) fn runtime_macro_binding_read(
        &self,
        body: &crate::syntax::outline::BodyHandle,
        index: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
        name: &str,
        destination: crate::ir::pc::SlotId,
    ) -> Result<crate::ir::op::Op, Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("selected binding predicate source missing"))?;
        file.parsed
            .runtime_macro_binding_read(body, index, key, name, destination)
    }
    pub(crate) fn runtime_macro_scalar_plan(
        &self,
        body: &crate::syntax::outline::BodyHandle,
        index: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
        arguments: &crate::syntax::runtime_macro_arguments::RuntimeMacroArguments,
    ) -> Result<crate::syntax::runtime_macro_scalars::RuntimeScalarPlan, Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("selected scalar assertion source missing"))?;
        file.parsed
            .runtime_macro_scalar_plan(body, index, key, arguments)
    }
    pub(crate) fn require_runtime_macro_call(
        &self,
        body: &crate::syntax::outline::BodyHandle,
        index: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
        call: &crate::syntax::runtime_macro_scalars::RuntimeMacroCall,
    ) -> Result<(), Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("selected assertion call source missing"))?;
        file.parsed
            .require_runtime_macro_call(body, index, key, call)
    }
    pub(crate) fn runtime_assertion_predicate_text(
        &self,
        arguments: &crate::syntax::runtime_macro_arguments::RuntimeMacroArguments,
    ) -> Result<String, Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("selected scalar assertion source missing"))?;
        file.parsed.runtime_assertion_predicate_text(arguments)
    }
    pub(crate) fn runtime_macro_successor(
        &self,
        body: &crate::syntax::outline::BodyHandle,
        index: &crate::syntax::body_index::BodyIndex,
        key: &crate::ir::pc::PcKey,
    ) -> Result<crate::ir::pc::PcKey, Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("selected macro continuation source missing"))?;
        file.parsed.runtime_macro_successor(body, index, key)
    }
    pub(crate) fn prepare_runtime_macro(
        &self,
        range: crate::syntax::spans::SourceRange,
        lookup: crate::frontend::macro_names::PathLookup,
        catalog: &crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog,
    ) -> Result<crate::frontend::runtime_macro_plan::RuntimeMacroPreparation, Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("selected runtime macro source missing"))?;
        let invocation = file.parsed.macro_call_at(range)?;
        crate::frontend::runtime_macro_plan::prepare_runtime_macro(
            &file.parsed,
            invocation,
            lookup,
            catalog,
        )
    }
    pub(crate) fn entered_macro_facts(
        &self,
        key: &super::manifest::BodyKey,
    ) -> Result<crate::syntax::entered_macros::EnteredMacroFacts, Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .expect("selected immutable source context");
        let item = file
            .outline
            .items
            .iter()
            .find(|item| item.body.as_ref().is_some_and(|body| body.key() == *key))
            .ok_or_else(|| publication_fault("selected body is absent or changed"))?;
        let body = item.body.as_ref().expect("matching entered body");
        let Some(crate::db::dependency::InputValue::CrateCfg(cfg)) =
            self.state
                .inputs
                .get(&crate::db::dependency::InputKey::CrateCfg(
                    self.selection.outline.crate_key,
                ))
        else {
            return Err(publication_fault("selected body cfg is missing"));
        };
        file.parsed.entered_macro_facts(body, cfg, &item.scope)
    }
    pub(crate) fn retained_body_facts(
        &self,
        body: &crate::syntax::outline::BodyHandle,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Result<crate::syntax::entered_macros::EnteredMacroFacts, Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .expect("selected immutable source context");
        let Some(crate::db::dependency::InputValue::CrateCfg(cfg)) =
            self.state
                .inputs
                .get(&crate::db::dependency::InputKey::CrateCfg(
                    self.selection.outline.crate_key,
                ))
        else {
            return Err(publication_fault("retained body cfg missing"));
        };
        file.parsed.entered_macro_facts(body, cfg, scope)
    }
    pub(crate) fn retained_container_facts(
        &self,
        handle: &crate::syntax::body_items::ContainerHandle,
        scope: &crate::syntax::outline::ScopeKey,
        context: &[crate::syntax::ast_adapter::TokenRecipe],
    ) -> Result<crate::syntax::entered_macros::EnteredMacroFacts, Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .expect("selected immutable source context");
        let Some(crate::db::dependency::InputValue::CrateCfg(cfg)) =
            self.state
                .inputs
                .get(&crate::db::dependency::InputKey::CrateCfg(
                    self.selection.outline.crate_key,
                ))
        else {
            return Err(publication_fault("retained container cfg missing"));
        };
        file.parsed
            .entered_container_facts(handle, cfg, scope, context)
    }
    /// Index only an explicitly selected original body in this immutable publication.
    pub fn index_body(
        &self,
        expected: &SourceSelection,
        key: &crate::source::manifest::BodyKey,
        limits: crate::syntax::body_index::BodyIndexLimits,
    ) -> Result<crate::syntax::body_index::BodyIndex, Diagnostic> {
        if expected.canonical()? != self.selection {
            return Err(publication_fault("body index source revision mismatch"));
        }
        let file = self
            .state
            .files
            .get(&self.location)
            .expect("selected immutable source context");
        let body = file
            .outline
            .items
            .iter()
            .filter_map(|item| item.body.as_ref())
            .find(|body| body.key() == *key)
            .ok_or_else(|| publication_fault("body index selection absent or changed"))?;
        file.parsed.index_body(body, limits)
    }
    pub(crate) fn initializer_parameters(
        &self,
        key: &crate::source::manifest::BodyKey,
    ) -> Result<
        Vec<crate::syntax::signatures::DefinitionParameter>,
        crate::core::diagnostic::Diagnostic,
    > {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("initializer source lease absent"))?;
        let mut selected = file
            .outline
            .items
            .iter()
            .filter(|item| item.body.as_ref().is_some_and(|body| body.key() == *key));
        let item = selected
            .next()
            .ok_or_else(|| publication_fault("initializer source declaration absent"))?;
        if selected.next().is_some()
            || !matches!(
                item.def_key.item_kind,
                crate::source::manifest::ItemKind::Constant
                    | crate::source::manifest::ItemKind::Static
            )
        {
            return Err(publication_fault(
                "initializer source declaration kind or identity differs",
            ));
        }
        let body = item
            .body
            .as_ref()
            .ok_or_else(|| publication_fault("initializer body absent"))?;
        file.parsed.initializer_parameters(
            &item.header,
            &crate::syntax::outline::DeclarationContext {
                scope: item.scope.clone(),
                range: item.declaration_span,
            },
            body,
        )
    }
    /// Recheck a captured plan against this immutable source lease. This proves
    /// only syntax/body/cfg identity, not a type, namespace seal or authority.
    pub(crate) fn require_constructor_plan(
        &self,
        index: &crate::syntax::body_index::BodyIndex,
        plan: &crate::syntax::constructors::ConstructorPlan,
    ) -> Result<(), Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("constructor source lease absent"))?;
        let mut bodies = file
            .outline
            .items
            .iter()
            .filter_map(|item| item.body.as_ref())
            .filter(|body| &body.key() == index.body());
        let body = bodies.next().ok_or_else(|| {
            publication_fault("constructor body absent from selected publication")
        })?;
        if bodies.next().is_some()
            || file
                .parsed
                .constructor_plan(body, index, plan.node(), self.captured_macro_cfg()?)?
                .as_ref()
                != Some(plan)
        {
            return Err(publication_fault("constructor source plan differs"));
        }
        Ok(())
    }
    pub(crate) fn record_constructor_path(
        &self,
        index: &crate::syntax::body_index::BodyIndex,
        plan: &crate::syntax::constructors::ConstructorPlan,
    ) -> Result<crate::syntax::constructors::RecordPathSelection, crate::core::diagnostic::Diagnostic>
    {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("constructor source lease absent"))?;
        let body = file
            .outline
            .items
            .iter()
            .filter_map(|item| item.body.as_ref())
            .find(|body| &body.key() == index.body())
            .ok_or_else(|| {
                publication_fault("constructor body absent from selected publication")
            })?;
        file.parsed
            .record_constructor_path(body, index, plan, self.captured_macro_cfg()?)
    }
    pub(crate) fn named_field_member(
        &self,
        index: &crate::syntax::body_index::BodyIndex,
        node: crate::ir::pc::NodeId,
    ) -> Result<String, Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("field source lease absent"))?;
        let mut bodies = file
            .outline
            .items
            .iter()
            .filter_map(|item| item.body.as_ref())
            .filter(|body| &body.key() == index.body());
        let body = bodies
            .next()
            .ok_or_else(|| publication_fault("field caller body absent"))?;
        if bodies.next().is_some() {
            return Err(publication_fault("field caller body is ambiguous"));
        }
        file.parsed.named_field_member(body, index, node)
    }
    pub fn fixture_markers(
        &self,
    ) -> Result<crate::syntax::fixture_markers::FixtureMarkers, Diagnostic> {
        self.state
            .files
            .get(&self.location)
            .expect("selected immutable source context")
            .parsed
            .fixture_markers()
    }
    pub(crate) fn has_runtime_macro_syntax(&self) -> Result<bool, Diagnostic> {
        Ok(self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("runtime macro source missing"))?
            .parsed
            .has_runtime_macro_syntax())
    }
    pub(crate) fn captured_macro_cfg(&self) -> Result<&super::snapshot::CfgSet, Diagnostic> {
        match self
            .state
            .inputs
            .get(&crate::db::dependency::InputKey::CrateCfg(
                self.selection.outline.crate_key,
            )) {
            Some(crate::db::dependency::InputValue::CrateCfg(cfg))
                if crate::core::digest::StableEncode::fingerprint(cfg)
                    == self.selection.outline.cfg =>
            {
                Ok(cfg)
            }
            _ => Err(publication_fault("captured macro cfg absent or changed")),
        }
    }
    pub fn captured_external_module_declarations(
        &self,
    ) -> Result<Vec<crate::syntax::modules::ExternalModuleDeclaration>, Diagnostic> {
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("captured module source missing"))?;
        crate::syntax::modules::external_module_declarations_from_tree(
            &file.parsed.parsed.tree(),
            self.captured_macro_cfg()?,
        )
    }
    pub(crate) fn captured_inline_module_layout(
        &self,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Result<crate::syntax::modules::InlineModuleLayout, Diagnostic> {
        let suffix = scope
            .module_path
            .strip_prefix(self.outline().root_scope.module_path.as_slice())
            .filter(|_| {
                scope.crate_key == self.outline().root_scope.crate_key && scope.owner.is_none()
            })
            .ok_or_else(|| publication_fault("inline module caller scope mismatch"))?;
        let file = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("inline module caller source missing"))?;
        crate::syntax::modules::inline_module_layout(
            &file.parsed.parsed.tree(),
            self.captured_macro_cfg()?,
            suffix,
        )
    }
    pub(crate) fn captured_macro_blob(&self) -> Result<&SourceBlob, Diagnostic> {
        self.state
            .inputs
            .get(&crate::db::dependency::InputKey::SourceBlob(
                self.selection.outline.parse.content,
            ))
            .and_then(crate::db::dependency::InputValue::source_blob)
            .ok_or_else(|| publication_fault("captured macro source blob absent"))
    }
    pub(crate) fn macro_token_group(
        &self,
        selected: crate::syntax::spans::SourceRange,
        definition: bool,
    ) -> Result<crate::syntax::spans::SourceRange, Diagnostic> {
        self.state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("captured macro parse absent"))?
            .parsed
            .macro_token_group(selected, definition)
    }
    pub(crate) fn macro_definition_shape_at(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<crate::syntax::macro_calls::MacroDefinitionShape, Diagnostic> {
        self.state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("captured macro definition parse absent"))?
            .parsed
            .macro_definition_shape_at(range)
    }
    pub(crate) fn macro_definition_header_at(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<(crate::syntax::spans::SourceRange, String), Diagnostic> {
        self.state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("captured macro definition parse absent"))?
            .parsed
            .macro_definition_header_at(range)
    }
    pub(crate) fn macro_definition_header(
        &self,
        definition: crate::core::digest::Digest,
    ) -> Result<(crate::syntax::spans::SourceRange, String), Diagnostic> {
        use crate::core::digest::StableEncode;
        let item = self
            .outline()
            .items
            .iter()
            .find(|item| {
                item.def_key.item_kind == super::manifest::ItemKind::Macro
                    && item.def_key.fingerprint() == definition
            })
            .ok_or_else(|| publication_fault("captured macro definition owner absent"))?;
        let result = self
            .state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("captured macro definition parse absent"))?
            .parsed
            .macro_definition_header_at(item.declaration_span)?;
        if result.1 != item.def_key.declared_name {
            return Err(publication_fault(
                "macro definition label differs from source member",
            ));
        }
        Ok(result)
    }
    pub(crate) fn source_dependencies(
        &self,
    ) -> Result<std::sync::Arc<super::dependencies::SourceDependencies>, Diagnostic> {
        match self
            .state
            .inputs
            .get(&crate::db::dependency::InputKey::SourceDependencies)
        {
            Some(crate::db::dependency::InputValue::SourceDependencies(graph)) => Ok(graph.clone()),
            _ => Err(publication_fault("captured source dependency graph absent")),
        }
    }
    pub fn outline(&self) -> &crate::syntax::outline::Outline {
        &self
            .state
            .files
            .get(&self.location)
            .expect("selected immutable source context")
            .outline
    }
}
pub struct PreparedSourceInputs {
    state: std::sync::Arc<PreparedSourceState>,
    pub parsed_files: usize,
    pub outlined_files: usize,
    pub reused_files: usize,
}
impl PreparedSourceInputs {
    pub fn refresh(
        previous: Option<&Self>,
        units: Vec<SourceUnitInput>,
        compile_env: std::collections::BTreeMap<String, String>,
    ) -> Result<Self, Diagnostic> {
        let dependencies = match previous.and_then(|previous| {
            previous
                .state
                .inputs
                .get(&crate::db::dependency::InputKey::SourceDependencies)
        }) {
            Some(crate::db::dependency::InputValue::SourceDependencies(graph)) => {
                graph.copy_rows()?
            }
            None => Vec::new(),
            _ => {
                return Err(publication_fault(
                    "previous source dependency input variant differs",
                ));
            }
        };
        Self::refresh_with_dependencies(previous, units, compile_env, dependencies)
    }
    pub fn refresh_with_dependencies(
        previous: Option<&Self>,
        units: Vec<SourceUnitInput>,
        compile_env: std::collections::BTreeMap<String, String>,
        dependencies: Vec<super::dependencies::SourceDependency>,
    ) -> Result<Self, Diagnostic> {
        use crate::{
            core::digest::StableEncode,
            db::{
                dependency::{InputKey, InputValue},
                key::OutlineKey,
            },
            syntax::{
                outline::ScopeMembers,
                parse::{ParseKey, ParsedFile},
            },
        };
        use std::{collections::BTreeMap, sync::Arc};
        if units.len() > 100_000 {
            return Err(Diagnostic::new(
                "LRR-E-SOURCE-LIMIT",
                "source publication exceeds 100,000 contexts".into(),
            ));
        }
        for unit in &units {
            unit.profile.validate_live()?;
        }
        let mut contents = BTreeSet::new();
        let mut source_bytes = 0usize;
        for unit in &units {
            if contents.insert(unit.blob.content()) {
                source_bytes = source_bytes
                    .checked_add(unit.blob.bytes().len())
                    .ok_or_else(|| {
                        publication_fault("source publication byte accounting overflow")
                    })?;
            }
        }
        if source_bytes > super::snapshot::MAX_SNAPSHOT_SOURCE {
            return Err(Diagnostic::new(
                "LRR-E-SOURCE-LIMIT",
                "source publication exceeds snapshot byte limit".into(),
            ));
        }
        let mut state = PreparedSourceState {
            _module_origin: None,
            _ordinary_module_origin: None,
            inputs: BTreeMap::new(),
            files: BTreeMap::new(),
            parses: BTreeMap::new(),
            outlines: BTreeMap::new(),
        };
        let mut parsed_files = 0;
        let mut outlined_files = 0;
        let mut reused_files = 0;
        let mut scopes = BTreeMap::<_, ScopeMembers>::new();
        let mut definitions = BTreeSet::new();
        let mut parses = BTreeMap::<ParseKey, Arc<ParsedFile>>::new();
        for mut unit in units {
            unit.module_path = crate::syntax::ast_adapter::semantic_module_path(&unit.module_path)
                .ok_or_else(|| publication_fault("invalid or oversized semantic module path"))?;
            let location = UnitLocation {
                path: unit.path,
                crate_key: unit.crate_key,
                module_path: unit.module_path.clone(),
            };
            if state.files.contains_key(&location) {
                return Err(publication_fault("duplicate source context"));
            }
            let key = OutlineKey {
                parse: ParseKey {
                    content: unit.blob.content(),
                    profile: unit.profile,
                },
                crate_key: unit.crate_key,
                module_path: unit.module_path,
                cfg: unit.cfg.fingerprint(),
            };
            let old = previous.and_then(|p| p.state.files.get(&location));
            let parsed = if let Some(old) = old.filter(|old| old.key.parse == key.parse) {
                old.parsed.clone()
            } else if let Some(parsed) = parses.get(&key.parse) {
                parsed.clone()
            } else {
                parsed_files += 1;
                Arc::new(
                    if key.parse.profile.edition == crate::core::profile::LIVE_EDITION {
                        ParsedFile::parse(&unit.blob, key.parse.profile.clone())?
                    } else {
                        ParsedFile::parse_dependency(&unit.blob, key.parse.profile.clone())?
                    },
                )
            };
            parses.insert(key.parse.clone(), parsed.clone());
            let outline = if let Some(old) = old.filter(|old| old.key == key) {
                reused_files += 1;
                old.outline.clone()
            } else {
                outlined_files += 1;
                Arc::new(parsed.outline(key.crate_key, key.module_path.clone(), &unit.cfg)?)
            };
            insert_input(
                &mut state.inputs,
                InputKey::SourceBlob(unit.blob.content()),
                InputValue::SourceBlob(unit.blob),
            )?;
            insert_input(
                &mut state.inputs,
                InputKey::CrateCfg(key.crate_key),
                InputValue::CrateCfg(unit.cfg),
            )?;
            let selection = SourceSelection {
                path: location.path.clone(),
                outline: key.clone(),
            };
            insert_input(
                &mut state.inputs,
                InputKey::CapturedSource(selection.location_identity()),
                InputValue::CapturedSource(selection),
            )?;
            for item in &outline.items {
                let definition = item.def_key.fingerprint();
                if !definitions.insert(definition) {
                    return Err(publication_fault(
                        "definition identity appears in multiple source contexts",
                    ));
                }
                insert_input(
                    &mut state.inputs,
                    InputKey::DefinitionHeader(definition),
                    InputValue::DefinitionHeader(item.header.clone()),
                )?;
                insert_input(
                    &mut state.inputs,
                    InputKey::DefinitionSource(definition),
                    InputValue::DefinitionSource(key.parse.clone()),
                )?;
                insert_input(
                    &mut state.inputs,
                    InputKey::DeclarationContext(definition),
                    InputValue::DeclarationContext(crate::syntax::outline::DeclarationContext {
                        scope: item.scope.clone(),
                        range: item.declaration_span,
                    }),
                )?;
                if let Some(body) = &item.body {
                    insert_input(
                        &mut state.inputs,
                        InputKey::DefinitionBody(definition),
                        InputValue::DefinitionBody(body.clone()),
                    )?;
                }
            }
            for (scope, members) in &outline.scopes {
                scopes.entry(scope.clone()).or_default().merge(members);
            }
            state.files.insert(
                location,
                PublishedFile {
                    key,
                    parsed,
                    outline,
                },
            );
        }
        for (scope, members) in scopes {
            insert_input(
                &mut state.inputs,
                InputKey::ScopeMembers(scope),
                InputValue::ScopeMembers(members),
            )?;
        }
        for (name, value) in compile_env {
            insert_input(
                &mut state.inputs,
                InputKey::CompileEnv(name),
                InputValue::CompileEnv(value),
            )?;
        }
        let roots = state
            .files
            .iter()
            .filter(|(location, _)| location.module_path.is_empty())
            .map(|(location, file)| SourceSelection {
                path: location.path.clone(),
                outline: file.key.clone(),
            })
            .collect();
        let dependency_graph =
            super::dependencies::SourceDependencies::prepare(roots, dependencies)?;
        insert_input(
            &mut state.inputs,
            InputKey::SourceDependencies,
            InputValue::SourceDependencies(Arc::new(dependency_graph)),
        )?;
        state.parses = parses;
        state.outlines = state
            .files
            .values()
            .map(|file| (file.key.clone(), file.outline.clone()))
            .collect();
        Ok(Self {
            state: Arc::new(state),
            parsed_files,
            outlined_files,
            reused_files,
        })
    }
    /// Explicit preparation adds a captured unit environment to this immutable publication.
    pub fn with_compile_environment(
        self,
        environment: std::sync::Arc<crate::expand::builtins::FrozenCompileEnvironment>,
    ) -> Result<Self, Diagnostic> {
        use crate::db::dependency::{InputKey, InputValue};
        if !self.state.files.keys().any(|location| {
            location.crate_key == environment.unit() && location.module_path.is_empty()
        }) {
            return Err(publication_fault(
                "compile environment owner root is not captured",
            ));
        }
        let mut bytes = self
            .state
            .file_metadata_bytes()?
            .checked_add(environment.allocated_bytes()?)
            .ok_or_else(|| publication_fault("compile environment metadata overflow"))?;
        for input in self.state.inputs.values() {
            bytes = bytes
                .checked_add(input.estimated_bytes()?)
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or_else(|| {
                    publication_fault("compile environment publication metadata limit")
                })?;
        }
        let mut state = PreparedSourceState {
            _module_origin: self.state._module_origin.clone(),
            _ordinary_module_origin: self.state._ordinary_module_origin.clone(),
            inputs: self.state.inputs.clone(),
            files: self
                .state
                .files
                .iter()
                .map(|(location, file)| {
                    (
                        location.clone(),
                        PublishedFile {
                            key: file.key.clone(),
                            parsed: file.parsed.clone(),
                            outline: file.outline.clone(),
                        },
                    )
                })
                .collect(),
            parses: self.state.parses.clone(),
            outlines: self.state.outlines.clone(),
        };
        insert_input(
            &mut state.inputs,
            InputKey::UnitCompileEnv(environment.unit()),
            InputValue::UnitCompileEnv(environment),
        )?;
        Ok(Self {
            state: std::sync::Arc::new(state),
            parsed_files: self.parsed_files,
            outlined_files: self.outlined_files,
            reused_files: self.reused_files,
        })
    }
    /// Select only an exact currently prepared context. Stale and missing input
    /// are typed outcomes and cannot silently choose a previous publication.
    pub fn select(&self, expected: &SourceSelection) -> Result<SourceSelectionOutcome, Diagnostic> {
        let expected = expected.canonical()?;
        expected.outline.parse.profile.validate_live()?;

        let location = UnitLocation {
            path: expected.path.clone(),
            crate_key: expected.outline.crate_key,
            module_path: expected.outline.module_path.clone(),
        };
        let Some(file) = self.state.files.get(&location) else {
            return Ok(SourceSelectionOutcome::Absent);
        };
        if file.key != expected.outline {
            return Ok(SourceSelectionOutcome::Changed {
                actual: SourceSelection {
                    path: expected.path,
                    outline: file.key.clone(),
                },
            });
        }
        Ok(SourceSelectionOutcome::Selected(PreparedSourceView {
            state: self.state.clone(),
            location,
            selection: expected,
        }))
    }
    pub fn publish(&self, db: &mut crate::db::memo::Database) -> Result<(), Diagnostic> {
        db.publish_prepared(self.state.clone())
    }
}
fn publication_fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-SNAPSHOT", message.into())
}
fn insert_input(
    inputs: &mut std::collections::BTreeMap<
        crate::db::dependency::InputKey,
        crate::db::dependency::InputValue,
    >,
    key: crate::db::dependency::InputKey,
    value: crate::db::dependency::InputValue,
) -> Result<(), Diagnostic> {
    if let Some(old) = inputs.get(&key) {
        if old.fingerprint() != value.fingerprint() {
            return Err(publication_fault("conflicting input contexts"));
        }
    } else {
        inputs.insert(key, value);
    }
    Ok(())
}

pub fn apply_batch(
    base: &Snapshot,
    changes: Vec<(SourcePath, Option<Vec<u8>>)>,
) -> Result<Snapshot, Diagnostic> {
    let mut files = base.files().clone();
    let mut seen = BTreeSet::new();
    for (path, bytes) in changes {
        if !seen.insert(path.clone()) {
            return Err(Diagnostic::new(
                "LRR-E-SOURCE-BATCH",
                format!("duplicate source change {}", path.as_str()),
            ));
        }
        match bytes {
            Some(bytes) => {
                files.insert(path, SourceBlob::new(bytes)?);
            }
            None if path == base.project().root_source => {
                return Err(Diagnostic::new(
                    "LRR-E-SOURCE-BATCH",
                    "cannot delete selected root".into(),
                ));
            }
            None => {
                files.remove(&path);
            }
        }
    }
    let cfg = base
        .cfg_by_crate()
        .get(&base.project().selected_crate)
        .ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-SOURCE-MISSING",
                "selected crate cfg is missing".into(),
            )
        })?;
    super::graph::validate(&base.project().root_source, &files, cfg)?;
    Snapshot::new(
        base.project().clone(),
        files,
        base.cfg_by_crate().clone(),
        base.compile_env().clone(),
        Some(base.id()),
    )
}

impl PreparedSourceView {
    pub(crate) fn has_lexical_callable_syntax(&self) -> Result<bool, Diagnostic> {
        self.state
            .files
            .get(&self.location)
            .ok_or_else(|| publication_fault("lexical source lease absent"))?
            .parsed
            .has_lexical_callable_syntax()
    }
}
