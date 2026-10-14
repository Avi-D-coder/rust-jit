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
    catalog: Arc<PreparedProcCatalog>,
    receipts: Vec<Digest>,
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
        if !registry.spans().iter().any(|s| s.key() == site)
            || receipts.is_empty()
            || receipts.len() > 4096
        {
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
            catalog,
            receipts,
        };
        selected.metadata_bytes()?;
        Ok(selected)
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
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| fault("source derive selection metadata limit"))?;
        Ok(bytes)
    }
    pub(crate) fn matches_request(
        &self,
        request: &PreparedFunctionRequest,
    ) -> Result<bool, Diagnostic> {
        let selected = self
            .catalog
            .select(&self.definition, ProceduralKind::Derive)?
            .ok_or_else(|| fault("source derive export absent"))?;
        Ok(request.source_derive_proof() == Some(self.fingerprint())
            && request.source() == self.source()
            && request.key().invocation == self.input.identity()
            && &request.key().macro_identity == selected.identity()
            && request.key().consumer_compile_env == self.catalog.environment()
            && request.key().procedural_epoch == Some(self.catalog.epoch()))
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
        if span.source.path.len() > 64 * 1024 {
            return Err(Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "source derive origin path limit".into(),
            ));
        }
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
        self.catalog
            .source_failure_origin(&self.definition, request.key(), invocation)
    }
    pub(crate) fn prepare(
        &self,
        session: &MacroSession,
        context: &DemandContext,
    ) -> Result<PreparedFunctionRequest, Diagnostic> {
        if context.unit != self.scope.crate_key
            || session.export_catalog(context)?.fingerprint() != self.catalog.fingerprint()
        {
            return Err(fault("source derive session/catalog/consumer differs"));
        }
        self.metadata_bytes()?;
        let selected = self
            .catalog
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
        e.digest(self.catalog.fingerprint());
        e.digest(self.definition.identity);
        e.digest(self.definition.defining_unit);
        e.u32(u32::from(self.definition.definition_edition));
        e.bytes(&self.site.0);
        e.sequence(&self.receipts, |e, id| e.digest(*id));
    }
}
