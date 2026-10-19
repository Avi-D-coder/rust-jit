//! Immutable captured-source selection; metadata and request construction execute no macro.
use super::{
    expansion::DemandContext,
    function_macro_requests::PreparedFunctionRequest,
    item_macro_inputs::DeriveOccurrenceInput,
    item_macro_requests::{PreparedItemRequest, ResolvedItemTarget},
    macro_session::MacroSession,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        derive_origin::DeriveDeclarationOrigin,
        identity::{MacroIdentity, ProceduralKind},
        scopes::MacroDefinition,
    },
    frontend::procedural_exports::PreparedProcCatalog,
    source::changes::PreparedSourceView,
    syntax::outline::ScopeKey,
};
use lrr_expand_adapter::{
    proc_client::{CapturedSpanSource, SourceDigestEncoding},
    token_wire::{Key, TokenForest},
};
use std::sync::Arc;

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
pub struct SourceDeriveSelection {
    source: PreparedSourceView,
    origin: DeriveDeclarationOrigin,
    scope: ScopeKey,
    input: DeriveOccurrenceInput,
    registry: TokenForest,
    site: Key,
    definition: MacroDefinition,
    executor: DeriveExecutor,
    receipts: Vec<Digest>,
}
/// Who expands the selected derive: the run's procedural catalog export, or
/// the pinned adapter's builtin expander for a library builtin derive.
enum DeriveExecutor {
    Procedural(Arc<PreparedProcCatalog>),
    Builtin {
        kind: lrr_expand_adapter::builtin_macros::BuiltinDeriveKind,
        facts: lrr_expand_adapter::builtin_macros::DeriveContainerFacts,
    },
}
impl SourceDeriveSelection {
    pub(crate) fn new(
        source: &PreparedSourceView,
        scope: ScopeKey,
        input: DeriveOccurrenceInput,
        registry: TokenForest,
        request: &crate::frontend::generated_derive_exports::DeriveDeclarationRequest,
        catalog: Arc<PreparedProcCatalog>,
        receipts: Vec<Digest>,
    ) -> Result<Self, Diagnostic> {
        let site = Key(input.selected().first_span.0);
        if !registry.spans().iter().any(|s| s.key() == site) || receipts.is_empty() {
            return Err(fault(
                "source derive site or helper receipt inventory differs",
            ));
        }
        Self::selection_bytes(
            &input,
            &registry,
            &scope,
            &request.origin,
            receipts.capacity(),
        )?;
        let selected = Self {
            source: source.share(),
            origin: request.origin.clone(),
            scope,
            input,
            registry,
            site,
            definition: request.definition.clone(),
            executor: DeriveExecutor::Procedural(catalog),
            receipts,
        };
        selected.metadata_bytes()?;
        Ok(selected)
    }
    /// A library builtin derive occurrence (`#[derive(Clone)]` resolving to
    /// `core::clone::Clone`), selected by its exact definition identity.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn builtin(
        source: &PreparedSourceView,
        scope: ScopeKey,
        input: DeriveOccurrenceInput,
        registry: TokenForest,
        origin: DeriveDeclarationOrigin,
        definition: MacroDefinition,
        kind: lrr_expand_adapter::builtin_macros::BuiltinDeriveKind,
        facts: lrr_expand_adapter::builtin_macros::DeriveContainerFacts,
        receipts: Vec<Digest>,
    ) -> Result<Self, Diagnostic> {
        let site = Key(input.selected().first_span.0);
        if !registry.spans().iter().any(|s| s.key() == site) || receipts.is_empty() {
            return Err(fault(
                "builtin derive site or helper receipt inventory differs",
            ));
        }
        let selected = Self {
            source: source.share(),
            origin,
            scope,
            input,
            registry,
            site,
            definition,
            executor: DeriveExecutor::Builtin { kind, facts },
            receipts,
        };
        selected.metadata_bytes()?;
        Ok(selected)
    }
    /// Definition from the actual producer selection, bound to this output.
    /// Context marks cannot supply this immutable source proof.
    pub(crate) fn definition_for_expansion(
        &self,
        record: &crate::expand::record::ExpansionRecord,
    ) -> Result<MacroDefinition, Diagnostic> {
        let key = record.key();
        key.validate()?;
        if key.invocation != self.input.identity()
            || key.definition_edition != self.definition.definition_edition
            || key.grammar != crate::expand::identity::GrammarSlot::Items
        {
            return Err(fault("derive output selection identity differs"));
        }
        let matches = match &self.executor {
            DeriveExecutor::Builtin { .. } => matches!(key.macro_identity,
                MacroIdentity::BuiltinDerive { definition } if definition == self.definition.identity),
            DeriveExecutor::Procedural(catalog) => {
                let selected = catalog
                    .select(&self.definition, ProceduralKind::Derive)?
                    .ok_or_else(|| fault("derive output selected export absent"))?;
                &key.macro_identity == selected.identity()
                    && key.consumer_compile_env == catalog.environment()
                    && key.procedural_epoch == Some(catalog.epoch())
            }
        };
        if !matches {
            return Err(fault("derive output producer differs from selection"));
        }
        Ok(self.definition.clone())
    }
    pub(crate) fn is_builtin(&self) -> bool {
        matches!(self.executor, DeriveExecutor::Builtin { .. })
    }
    fn catalog(&self) -> Result<&Arc<PreparedProcCatalog>, Diagnostic> {
        match &self.executor {
            DeriveExecutor::Procedural(catalog) => Ok(catalog),
            DeriveExecutor::Builtin { .. } => {
                Err(fault("builtin derive selection has no procedural export"))
            }
        }
    }
    /// The in-process expansion request of a builtin derive occurrence: the
    /// exact item input joined with the derive path's span record.
    pub(crate) fn builtin_request(
        &self,
        compile_env: Digest,
        engine: Digest,
    ) -> Result<crate::expand::source_declarative::SourceDeclarativeRequest, Diagnostic> {
        let DeriveExecutor::Builtin { kind, facts } = &self.executor else {
            return Err(fault("procedural derive selection has no builtin expander"));
        };
        let site = self
            .registry
            .spans()
            .iter()
            .find(|span| span.key() == self.site)
            .ok_or_else(|| fault("builtin derive site absent"))?;
        let item = TokenForest::decode(
            &self
                .input
                .item()
                .encode()
                .map_err(|e| fault(&e.to_string()))?,
        )
        .map_err(|e| fault(&e.message))?;
        let [lrr_expand_adapter::token_wire::TokenTree::Group { .. }] = item.roots() else {
            return Err(fault("builtin derive item input root differs"));
        };
        // The site's context chain joins the item's own records.
        let mut contexts = std::collections::BTreeMap::new();
        for row in item.contexts() {
            contexts.insert(row.key(), row.clone());
        }
        let registry_contexts: std::collections::BTreeMap<_, _> = self
            .registry
            .contexts()
            .iter()
            .map(|row| (row.key(), row))
            .collect();
        let mut pending = vec![site.context];
        while let Some(key) = pending.pop() {
            if contexts.contains_key(&key) {
                continue;
            }
            let row = registry_contexts
                .get(&key)
                .ok_or_else(|| fault("builtin derive site context absent"))?;
            if let Some(parent) = row.parent {
                pending.push(parent);
            }
            contexts.insert(key, (*row).clone());
        }
        let mut spans = std::collections::BTreeMap::new();
        for row in item.spans() {
            spans.insert(row.key(), row.clone());
        }
        spans.insert(site.key(), site.clone());
        let input = TokenForest::new(
            item.roots().to_vec(),
            contexts.into_values().collect(),
            spans.into_values().collect(),
        )
        .map_err(|e| fault(&e.message))?;
        let input_identity = crate::expand::builtin_derives::input_identity(
            &input.encode().map_err(|e| fault(&e.message))?,
            *facts,
        )?;
        let key = crate::expand::identity::ExpansionKey {
            macro_identity: MacroIdentity::BuiltinDerive {
                definition: self.definition.identity,
            },
            invocation: self.input.identity(),
            input_tokens: input_identity,
            consumer_compile_env: compile_env,
            parent_context: Digest(site.context.0),
            definition_edition: self.definition.definition_edition,
            call_edition: self.source.outline().profile.edition,
            grammar: crate::expand::identity::GrammarSlot::Items,
            engine_version: engine,
            procedural_epoch: None,
        };
        crate::expand::source_declarative::SourceDeclarativeRequest::builtin_derive(
            key,
            input,
            self.site,
            self.definition.defining_unit,
            *kind,
            *facts,
        )
    }
    pub(crate) fn declaration_site_span(
        &self,
    ) -> Result<crate::syntax::token_wire::SpanRecord, Diagnostic> {
        let forest = crate::syntax::token_wire::TokenForest::decode(
            &self.registry.encode().map_err(|e| fault(&e.message))?,
        )
        .map_err(|e| fault(&e.message))?;
        forest
            .spans()
            .iter()
            .find(|span| span.key().0 == self.site.0)
            .cloned()
            .ok_or_else(|| fault("source derive declaration span absent"))
    }
    pub fn input(&self) -> &DeriveOccurrenceInput {
        &self.input
    }
    pub fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub(crate) fn source(&self) -> &crate::source::changes::SourceSelection {
        self.source.selection()
    }
    pub(crate) fn validate_database(
        &self,
        db: &mut crate::db::memo::Database,
    ) -> Result<(), Diagnostic> {
        self.origin.validate_database(db)
    }
    pub(crate) fn original_item_target<'a>(
        &self,
        source: &'a PreparedSourceView,
    ) -> Result<crate::source::original_items::OriginalItemTarget<'a>, Diagnostic> {
        if self.source.selection() != source.selection() || !self.source.same_publication(source) {
            return Err(fault("source derive retained publication differs"));
        }
        let target = self.origin.source_target(source)?;
        if &target.item().scope != self.scope() {
            return Err(fault("source derive canonical insertion scope differs"));
        }
        Ok(target)
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        Self::selection_bytes(
            &self.input,
            &self.registry,
            &self.scope,
            &self.origin,
            self.receipts.capacity(),
        )
    }
    fn selection_bytes(
        input: &DeriveOccurrenceInput,
        registry: &TokenForest,
        scope: &ScopeKey,
        origin: &DeriveDeclarationOrigin,
        receipt_capacity: usize,
    ) -> Result<usize, Diagnostic> {
        let bytes = registry
            .allocated_bytes()
            .map_err(|e| fault(&e.message))?
            .checked_add(input.retained_bytes())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .and_then(|n| n.checked_add(scope.heap_bytes()))
            .and_then(|n| {
                n.checked_add(receipt_capacity.checked_mul(std::mem::size_of::<Digest>())?)
            })
            .and_then(|n| n.checked_add(origin.metadata_bytes().ok()?))
            .ok_or_else(|| fault("source derive selection metadata limit"))?;
        Ok(bytes)
    }
    pub(crate) fn matches_request(
        &self,
        request: &PreparedFunctionRequest,
    ) -> Result<bool, Diagnostic> {
        let DeriveExecutor::Procedural(catalog) = &self.executor else {
            return Ok(false);
        };
        let selected = catalog
            .select(&self.definition, ProceduralKind::Derive)?
            .ok_or_else(|| fault("source derive export absent"))?;
        Ok(request.source_derive_proof() == Some(self.fingerprint())
            && request.source() == self.source()
            && request.key().invocation == self.input.identity()
            && &request.key().macro_identity == selected.identity()
            && request.key().consumer_compile_env == catalog.environment()
            && request.key().procedural_epoch == Some(catalog.epoch()))
    }
    /// Retain the actual occurrence token anchor after exact request binding.
    pub(crate) fn diagnostic_origin(
        &self,
        request: &PreparedFunctionRequest,
    ) -> Result<crate::syntax::origin::MacroOrigin, Diagnostic> {
        if !self.matches_request(request)? {
            return Err(fault("source derive failure origin request differs"));
        }
        let span = self
            .registry
            .spans()
            .iter()
            .find(|span| span.key() == self.site)
            .ok_or_else(|| fault("source derive failure origin site absent"))?;
        let mut path = String::new();
        path.try_reserve_exact(span.source.path.len())
            .map_err(|_| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "source derive origin path allocation".into(),
                )
            })?;
        path.push_str(&span.source.path);
        let invocation = crate::syntax::origin::SourceOrigin {
            source: crate::syntax::token_wire::SourceIdentity {
                unit: crate::syntax::token_wire::Key(span.source.unit.0),
                content: crate::syntax::token_wire::Key(span.source.content.0),
                path,
            },
            range: crate::syntax::spans::SourceRange::checked(span.start, span.end)?,
        };
        self.catalog()?
            .source_failure_origin(&self.definition, request.key(), invocation)
    }
    pub(crate) fn prepare(
        &self,
        session: &MacroSession,
        context: &DemandContext,
    ) -> Result<PreparedFunctionRequest, Diagnostic> {
        let catalog = self.catalog()?;
        if context.unit != self.scope.crate_key
            || session.export_catalog(context)?.fingerprint() != catalog.fingerprint()
        {
            return Err(fault("source derive session/catalog/consumer differs"));
        }
        self.metadata_bytes()?;
        let selected = catalog
            .select(&self.definition, ProceduralKind::Derive)?
            .ok_or_else(|| fault("source derive selected export absent"))?;
        let MacroIdentity::Procedural {
            artifact, export, ..
        } = selected.identity()
        else {
            return Err(fault("source derive export kind differs"));
        };
        let site = self
            .registry
            .spans()
            .iter()
            .find(|s| s.key() == self.site)
            .ok_or_else(|| fault("source derive site absent"))?;
        let captured = CapturedSpanSource::new(
            site.source.clone(),
            self.source.captured_macro_blob()?.text()?.into(),
            SourceDigestEncoding::LrrSourceBytes,
            None,
        )
        .map_err(|e| fault(&e.message))?;
        let definition = session
            .source_definition_site(*artifact)
            .ok_or_else(|| fault("source derive definition site is not captured"))?;
        let registry = crate::expand::procedural_sites::merge(definition, &self.registry)?;
        let definition_source = crate::expand::procedural_sites::source(definition)?;
        let target = ResolvedItemTarget {
            artifact: Key(artifact.0),
            export: export.clone(),
            scope: Key(self.scope.fingerprint().0),
            parent_origin: Key(crate::db::key::QueryKey::Outline(
                self.source.selection().outline.clone(),
            )
            .fingerprint()
            .0),
            registry: &registry,
            definition_site: Key(definition.site.0),
            call_site: self.site,
            mixed_site: self.site,
            sources: vec![captured, definition_source],
        };
        let prepared =
            PreparedItemRequest::derive_occurrence(session, context, &self.input, target)?;
        if &prepared.key().macro_identity != selected.identity() {
            return Err(fault("source derive prepared export differs"));
        }
        Ok(PreparedFunctionRequest::from_source_derive(
            self.source.selection().clone(),
            prepared,
            self.fingerprint(),
        ))
    }
}
impl StableEncode for SourceDeriveSelection {
    const DOMAIN: &'static str = "lrr/v2/source-derive-selection";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.origin);
        e.record(&self.scope);
        e.digest(self.input.identity());
        match &self.executor {
            DeriveExecutor::Procedural(catalog) => e.digest(catalog.fingerprint()),
            DeriveExecutor::Builtin { kind, facts } => {
                e.string(kind.name());
                e.u8(u8::from(facts.derives_copy));
                e.u8(u8::from(facts.derives_ord));
            }
        }
        e.digest(self.definition.identity);
        e.digest(self.definition.defining_unit);
        e.u32(u32::from(self.definition.definition_edition));
        e.bytes(&self.site.0);
        e.sequence(&self.receipts, |e, id| e.digest(*id));
    }
}
