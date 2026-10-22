//! Immutable pure demand for an owned generated item-position call.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    expand::{
        generated_items::GeneratedItems,
        identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        record::ExpansionRecord,
        scopes::MacroDefinition,
    },
    syntax::token_wire::Edition,
};
use std::sync::Arc;
pub struct GeneratedCallRequest {
    items: Arc<GeneratedItems>,
    body_items: Arc<GeneratedItems>,
    call: usize,
    attribute: Option<super::generated_attribute_input::GeneratedAttributeInput>,
    consumed_compiler_tests: usize,
    derive_location: Option<(usize, usize)>,
    derive: Option<(
        crate::syntax::token_wire::TokenForest,
        crate::syntax::token_wire::Key,
        Digest,
    )>,
    definition: MacroDefinition,
    key: ExpansionKey,
    compiler: bool,
    procedural: Option<lrr_expand_adapter::proc_client::ProcInvocation>,
    procedural_catalog: Option<Digest>,
    diagnostic_origin: Option<Arc<crate::syntax::origin::MacroOrigin>>,
    enclosing: Option<Box<super::source_declarative::SourceDeclarativeRequest>>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
impl GeneratedCallRequest {
    pub(crate) fn prepare(
        items: Arc<GeneratedItems>,
        call: usize,
        definition: MacroDefinition,
    ) -> Result<Self, Diagnostic> {
        let input = items
            .calls()
            .get(call)
            .ok_or_else(|| fault("generated call index absent"))?;
        let site = input
            .input
            .spans()
            .iter()
            .find(|s| s.key() == input.site)
            .ok_or_else(|| fault("generated call site absent"))?;
        let call_edition = match site.edition {
            Edition::E2015 => 2015,
            Edition::E2018 => 2018,
            Edition::E2021 => 2021,
            Edition::E2024 => 2024,
        };
        let parent = items.record().key();
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: definition.identity,
            },
            invocation: input.key.fingerprint(),
            input_tokens: Digest::of_bytes(&input.input.encode().map_err(|e| fault(&e.message))?),
            consumer_compile_env: parent.consumer_compile_env,
            parent_context: Digest(site.context.0),
            definition_edition: definition.definition_edition,
            call_edition,
            grammar: GrammarSlot::Items,
            engine_version: parent.engine_version,
            procedural_epoch: None,
        };
        key.validate()?;
        Ok(Self {
            body_items: items.clone(),
            items,
            call,
            attribute: None,
            consumed_compiler_tests: 0,
            derive: None,
            derive_location: None,
            definition,
            key,
            enclosing: None,
            compiler: false,
            procedural: None,
            procedural_catalog: None,
            diagnostic_origin: None,
        })
    }
    pub(crate) fn prepare_attribute(
        items: Arc<GeneratedItems>,
        index: usize,
        definition: MacroDefinition,
        input: super::generated_attribute_input::GeneratedAttributeInput,
    ) -> Result<Self, Diagnostic> {
        let header = items
            .items()
            .get(index)
            .ok_or_else(|| fault("generated attribute header absent"))?;
        let span = input
            .item
            .spans()
            .iter()
            .find(|span| span.key() == input.site)
            .ok_or_else(|| fault("generated attribute site absent"))?;
        let call_edition = match span.edition {
            Edition::E2015 => 2015,
            Edition::E2018 => 2018,
            Edition::E2021 => 2021,
            Edition::E2024 => 2024,
        };
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: definition.identity,
            },
            invocation: header.key.fingerprint(),
            input_tokens: Digest::of_bytes(&input.item.encode().map_err(|e| fault(&e.message))?),
            consumer_compile_env: items.record().key().consumer_compile_env,
            parent_context: Digest(span.context.0),
            definition_edition: definition.definition_edition,
            call_edition,
            grammar: GrammarSlot::Items,
            engine_version: items.record().key().engine_version,
            procedural_epoch: None,
        };
        key.validate()?;
        Ok(Self {
            body_items: items.clone(),
            items,
            call: 0,
            consumed_compiler_tests: input.consumed_tests.len(),
            attribute: Some(input),
            derive: None,
            derive_location: None,
            definition,
            key,
            compiler: false,
            procedural: None,
            procedural_catalog: None,
            diagnostic_origin: None,
            enclosing: None,
        })
    }
    pub(crate) fn prepare_derive(
        items: Arc<GeneratedItems>,
        index: usize,
        cfg: &crate::source::snapshot::CfgSet,
        row: usize,
        definition: MacroDefinition,
        helpers: Arc<crate::frontend::generated_derive_helpers::GeneratedDeriveHelpers>,
    ) -> Result<Self, Diagnostic> {
        let path = helpers.invocation_path(row, definition.identity)?;
        let body = helpers.materialize_occurrence_input(&items, index, cfg, row)?;
        let site = path.last_span;
        let span = path
            .tokens()
            .spans()
            .iter()
            .find(|s| s.key() == site)
            .ok_or_else(|| fault("derive call site absent"))?;
        // Materialization removes the derive annotation. Keep its exact site row
        // registered for the protocol without restoring it to the item tokens.
        let mut contexts: std::collections::BTreeMap<_, _> = body
            .contexts()
            .iter()
            .map(|r| (r.key(), r.clone()))
            .collect();
        for row in path.tokens().contexts() {
            if contexts.get(&row.key()).is_some_and(|old| old != row) {
                return Err(fault("derive site context conflicts with item"));
            }
            contexts.insert(row.key(), row.clone());
        }
        let mut spans: std::collections::BTreeMap<_, _> =
            body.spans().iter().map(|r| (r.key(), r.clone())).collect();
        if spans.get(&site).is_some_and(|old| old != span) {
            return Err(fault("derive site span conflicts with item"));
        }
        spans.insert(site, span.clone());
        let input = crate::syntax::token_wire::TokenForest::new(
            body.roots().to_vec(),
            contexts.into_values().collect(),
            spans.into_values().collect(),
        )
        .map_err(|e| fault(&e.message))?;
        let call_edition = match span.edition {
            Edition::E2015 => 2015,
            Edition::E2018 => 2018,
            Edition::E2021 => 2021,
            Edition::E2024 => 2024,
        };
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: definition.identity,
            },
            invocation: path.identity,
            input_tokens: Digest::of_bytes(&input.encode().map_err(|e| fault(&e.message))?),
            consumer_compile_env: items.record().key().consumer_compile_env,
            parent_context: Digest(span.context.0),
            definition_edition: definition.definition_edition,
            call_edition,
            grammar: GrammarSlot::Items,
            engine_version: items.record().key().engine_version,
            procedural_epoch: None,
        };
        key.validate()?;
        Ok(Self {
            body_items: items.clone(),
            items,
            call: 0,
            attribute: None,
            consumed_compiler_tests: 0,
            derive: Some((input, site, helpers.catalog_identity())),
            derive_location: Some((index, row)),
            definition,
            key,
            compiler: false,
            procedural: None,
            procedural_catalog: None,
            diagnostic_origin: None,
            enclosing: None,
        })
    }
    pub(crate) fn with_builtin_derive(
        mut self,
        kind: lrr_expand_adapter::builtin_macros::BuiltinDeriveKind,
        facts: lrr_expand_adapter::builtin_macros::DeriveContainerFacts,
    ) -> Result<Self, Diagnostic> {
        let (input, site, _) = self
            .derive
            .as_ref()
            .ok_or_else(|| fault("builtin derive input absent"))?;
        self.key.macro_identity = MacroIdentity::BuiltinDerive {
            definition: self.definition.identity,
        };
        let encoded = input.encode().map_err(|e| fault(&e.message))?;
        self.key.input_tokens = super::builtin_derives::input_identity(&encoded, facts)?;
        self.key.validate()?;
        let location = input
            .spans()
            .iter()
            .find(|span| span.key() == *site)
            .ok_or_else(|| fault("builtin derive origin site absent"))?;
        self.diagnostic_origin = Some(Arc::new(crate::syntax::origin::MacroOrigin {
            invocation_identity: self.key.invocation,
            invocation: Some(crate::syntax::origin::SourceOrigin {
                source: location.source.clone(),
                range: crate::syntax::spans::SourceRange::checked(location.start, location.end)?,
            }),
            definition: None,
            failure_kind: None,
            failure_span_key: None,
            failure_span: None,
            parents: Vec::new(),
        }));
        let input = lrr_expand_adapter::token_wire::TokenForest::decode(&encoded)
            .map_err(|e| fault(&e.message))?;
        self.enclosing = Some(Box::new(
            super::source_declarative::SourceDeclarativeRequest::builtin_derive(
                self.key.clone(),
                input,
                lrr_expand_adapter::token_wire::Key(site.0),
                self.definition.defining_unit,
                kind,
                facts,
            )?,
        ));
        Ok(self)
    }
    pub(crate) fn is_builtin_derive(&self) -> bool {
        self.derive.is_some()
            && self.enclosing.is_some()
            && matches!(self.key.macro_identity, MacroIdentity::BuiltinDerive { definition } if definition == self.definition.identity)
    }
    pub(crate) fn builtin_derive_request(
        &self,
    ) -> Option<&super::source_declarative::SourceDeclarativeRequest> {
        self.is_builtin_derive()
            .then(|| self.enclosing.as_deref())
            .flatten()
    }
    pub(crate) fn derive_location(&self) -> Result<(usize, usize), Diagnostic> {
        self.derive_location
            .ok_or_else(|| fault("derive location absent"))
    }
    pub fn is_derive(&self) -> bool {
        self.derive.is_some()
            || matches!(
                self.key.macro_identity,
                MacroIdentity::Procedural {
                    kind: super::identity::ProceduralKind::Derive,
                    ..
                }
            )
    }
    pub(crate) fn with_generated_body(
        mut self,
        items: Arc<GeneratedItems>,
    ) -> Result<Self, Diagnostic> {
        if items.macro_edition(self.definition.identity)? != self.definition.definition_edition {
            return Err(fault("generated definition body edition differs"));
        }
        self.body_items = items;
        Ok(self)
    }
    pub(crate) fn with_enclosing(
        mut self,
        provider: &super::source_declarative::SourceDeclarative,
        parents: Vec<crate::syntax::origin::ExpansionOrigin>,
    ) -> Result<Self, Diagnostic> {
        let input = &self.items.calls()[self.call];
        self.enclosing = Some(Box::new(
            provider
                .generated_request(&self.definition, self.key.clone(), &input.input, input.site)?
                .with_origin_parents(parents)?,
        ));
        Ok(self)
    }
    /// A generated call of a library `#[rustc_builtin_macro]` the pinned
    /// adapter expands. `provider` is the consumer source whose configuration
    /// and physical invocations supply the builtin's call-site facts.
    pub(crate) fn with_library_builtin(
        mut self,
        provider: &super::source_declarative::SourceDeclarative,
        kind: lrr_expand_adapter::builtin_macros::BuiltinKind,
        parents: Vec<crate::syntax::origin::ExpansionOrigin>,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Result<Self, Diagnostic> {
        let input = &self.items.calls()[self.call];
        self.enclosing = Some(Box::new(
            provider
                .generated_builtin_request(
                    &self.definition,
                    kind,
                    self.key.clone(),
                    &input.input,
                    input.site,
                    &parents,
                    scope,
                )?
                .with_origin_parents(parents)?,
        ));
        Ok(self)
    }
    pub(crate) fn with_compiler(
        mut self,
        catalog: &crate::frontend::compile_input_catalog::CompileInputCatalog,
    ) -> Result<Self, Diagnostic> {
        if catalog.select(&self.definition)?.is_none() {
            return Err(fault("generated compiler definition is absent"));
        }
        self.key.macro_identity = MacroIdentity::Builtin {
            definition: self.definition.identity,
        };
        self.key.validate()?;
        self.compiler = true;
        Ok(self)
    }
    /// Pure assembly retains real token origins and captured source callbacks.
    pub(crate) fn with_procedural(
        mut self,
        catalog: &crate::frontend::procedural_exports::PreparedProcCatalog,
        scope: &crate::syntax::outline::ScopeKey,
        providers: &[&super::source_declarative::SourceDeclarative],
    ) -> Result<Self, Diagnostic> {
        use lrr_expand_adapter::{
            proc_client::{
                CapturedSpanSource, ProcForm, ProcGrammar, ProcInvocation, SourceDigestEncoding,
            },
            token_wire::Key,
        };
        if scope.crate_key != catalog.consumer()
            || self.items.epoch() != catalog.epoch()
            || self.items.record().key().consumer_compile_env != catalog.environment()
        {
            return Err(fault(
                "generated procedural consumer/epoch/environment differs",
            ));
        }
        let kind = if self.derive.is_some() {
            super::identity::ProceduralKind::Derive
        } else if self.attribute.is_some() {
            super::identity::ProceduralKind::Attribute
        } else {
            super::identity::ProceduralKind::Function
        };
        let selected = catalog
            .select(&self.definition, kind)?
            .ok_or_else(|| fault("generated procedural export absent"))?;
        let MacroIdentity::Procedural {
            artifact, export, ..
        } = selected.identity()
        else {
            return Err(fault("generated procedural identity kind differs"));
        };
        let (arguments, attributes, site) = if let Some((input, site, proof_catalog)) = &self.derive
        {
            if *proof_catalog != catalog.fingerprint() {
                return Err(fault("derive helper catalog changed"));
            }
            (input.clone(), None, Key(site.0))
        } else if let Some(attribute) = &self.attribute {
            let item = crate::syntax::token_wire::TokenForest::decode(
                &attribute.item.encode().map_err(|e| fault(&e.message))?,
            )
            .map_err(|e| fault(&e.message))?;
            let attributes = attribute
                .arguments
                .as_ref()
                .map(|tokens| {
                    lrr_expand_adapter::token_wire::TokenForest::decode(
                        &tokens.encode().map_err(|e| fault(&e.message))?,
                    )
                    .map_err(|e| fault(&e.message))
                })
                .transpose()?;
            (item, attributes, Key(attribute.site.0))
        } else {
            let call = &self.items.calls()[self.call];
            (
                super::attributes::macro_argument_contents(&call.input)?,
                None,
                Key(call.site.0),
            )
        };
        let input = lrr_expand_adapter::token_wire::TokenForest::decode(
            &arguments.encode().map_err(|e| fault(&e.message))?,
        )
        .map_err(|e| fault(&e.message))?;
        let mut sources = std::collections::BTreeMap::new();
        let mut bytes = input
            .allocated_bytes()
            .map_err(|e| fault(&e.message))?
            .checked_add(std::mem::size_of::<ProcInvocation>())
            .and_then(|n| n.checked_add(export.capacity()))
            .ok_or_else(|| fault("generated procedural input byte limit"))?;
        for provider in providers {
            let source = provider.module_source();
            let forest = provider.owned_forest()?;
            let base = forest
                .spans()
                .first()
                .ok_or_else(|| fault("generated procedural source span table absent"))?;
            if !input.spans().iter().any(|span| {
                span.source.unit.0 == base.source.unit.0
                    && span.source.content.0 == base.source.content.0
                    && span.source.path == base.source.path
            }) {
                continue;
            }
            let source_key = (
                base.source.unit.0,
                base.source.content.0,
                base.source.path.clone(),
            );
            if sources.contains_key(&source_key) {
                continue;
            }
            let blob = source.captured_macro_blob()?;
            let text = blob.text()?;
            bytes
                .checked_add(text.len())
                .and_then(|n| n.checked_add(base.source.path.capacity()))
                .ok_or_else(|| fault("generated procedural capture byte limit"))?;
            let captured = CapturedSpanSource::new(
                base.source.clone(),
                text.into(),
                SourceDigestEncoding::LrrSourceBytes,
                None,
            )
            .map_err(|e| fault(&e.message))?;
            bytes = bytes
                .checked_add(captured.heap_bytes().map_err(|e| fault(&e.message))?)
                .ok_or_else(|| fault("generated procedural capture byte limit"))?;
            sources.insert(source_key, captured);
        }
        let mut request = ProcInvocation {
            identity: Key(self.key.invocation.0),
            unit: Key(scope.crate_key.0),
            epoch: self.items.epoch(),
            artifact: Key(artifact.0),
            export: export.clone(),
            form: if self.derive.is_some() {
                ProcForm::Derive
            } else if self.attribute.is_some() {
                ProcForm::Attribute
            } else {
                ProcForm::FunctionLike
            },
            grammar: ProcGrammar::Items,
            scope: Key(scope.fingerprint().0),
            parent_origin: Key(self.items.record().key().fingerprint().0),
            input,
            attributes,
            definition_site: site,
            call_site: site,
            mixed_site: site,
            sources: sources.into_values().collect(),
        };
        let definition = catalog
            .definition_site(*artifact)
            .ok_or_else(|| fault("generated procedural producer definition facts absent"))?;
        request.input = super::procedural_sites::merge(definition, &request.input)?;
        let definition_source = super::procedural_sites::source(definition)?;
        request.definition_site = Key(definition.site.0);
        if !request
            .sources
            .iter()
            .any(|source| source.source() == definition_source.source())
        {
            request.sources.push(definition_source);
        }
        if let Some(tokens) = &request.attributes {
            bytes
                .checked_add(tokens.allocated_bytes().map_err(|e| fault(&e.message))?)
                .ok_or_else(|| fault("generated procedural attribute input limit"))?;
        }
        self.key.macro_identity = selected.identity().clone();
        self.key.input_tokens =
            crate::service::macro_session::procedural_input_fingerprint(&request)?;
        self.key.consumer_compile_env = catalog.environment();
        self.key.engine_version = crate::service::macro_session::procedural_engine_version();
        self.key.procedural_epoch = Some(self.items.epoch());
        self.key.validate()?;
        self.diagnostic_origin = Some(Arc::new(
            self.capture_procedural_origin(&request, catalog, providers)?,
        ));
        self.procedural_catalog = Some(catalog.fingerprint());
        self.procedural = Some(request);
        self.attribute = None;
        self.derive = None;
        self.procedural_bytes()?;
        Ok(self)
    }
    fn capture_procedural_origin(
        &self,
        request: &lrr_expand_adapter::proc_client::ProcInvocation,
        catalog: &crate::frontend::procedural_exports::PreparedProcCatalog,
        providers: &[&super::source_declarative::SourceDeclarative],
    ) -> Result<crate::syntax::origin::MacroOrigin, Diagnostic> {
        use crate::syntax::{
            origin::{DefinitionOrigin, ExpansionOrigin, MacroOrigin, SourceOrigin},
            spans::SourceRange,
            token_wire::{Key, SourceIdentity},
        };
        let limit = || {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "generated procedural origin storage limit".into(),
            )
        };
        let call = request
            .input
            .spans()
            .iter()
            .find(|span| span.key() == request.call_site)
            .ok_or_else(|| fault("generated procedural diagnostic call site absent"))?;
        let at = SourceOrigin {
            source: SourceIdentity {
                unit: Key(call.source.unit.0),
                content: Key(call.source.content.0),
                path: String::new(),
            },
            range: SourceRange::checked(call.start, call.end)?,
        };
        let mut path = String::new();
        path.try_reserve_exact(call.source.path.len())
            .map_err(|_| limit())?;
        path.push_str(&call.source.path);
        let invocation = SourceOrigin {
            source: SourceIdentity { path, ..at.source },
            ..at
        };
        let definition = catalog
            .definition_site(Digest(request.artifact.0))
            .and_then(|site| {
                let kind = match request.form {
                    lrr_expand_adapter::proc_client::ProcForm::FunctionLike => {
                        crate::expand::identity::ProceduralKind::Function
                    }
                    lrr_expand_adapter::proc_client::ProcForm::Attribute => {
                        crate::expand::identity::ProceduralKind::Attribute
                    }
                    lrr_expand_adapter::proc_client::ProcForm::Derive => {
                        crate::expand::identity::ProceduralKind::Derive
                    }
                };
                site.diagnostic_definition(kind, &request.export)
                    .map(|range| (site, range))
            })
            .map(|(site, range)| {
                let mut name = String::new();
                let budget = request
                    .export
                    .len()
                    .checked_add(site.source.path.len())
                    .ok_or_else(limit)?;
                let _ = budget;
                name.try_reserve_exact(request.export.len())
                    .map_err(|_| limit())?;
                name.push_str(&request.export);
                let mut path = String::new();
                path.try_reserve_exact(site.source.path.len())
                    .map_err(|_| limit())?;
                path.push_str(&site.source.path);
                Ok::<_, Diagnostic>(DefinitionOrigin {
                    identity: self.definition.identity,
                    name,
                    location: SourceOrigin {
                        source: SourceIdentity {
                            unit: site.source.unit,
                            content: site.source.content,
                            path,
                        },
                        range,
                    },
                })
            })
            .transpose()?;
        let parent_key = self.items.record().key();
        let mut parent_at: Option<SourceOrigin> = None;
        for provider in providers {
            let source = provider.module_source();
            for call in source.outline().item_invocations.iter().filter(|call| {
                call.item_input == parent_key.invocation && call.attribute_input.is_none()
            }) {
                let selected = source.selection();
                if let Some(old) = &parent_at {
                    if old.source.unit.0 != selected.outline.crate_key.0
                        || old.source.content.0 != selected.outline.parse.content.0
                        || old.source.path != selected.path.as_str()
                        || old.range != call.range
                    {
                        return Err(fault("generated procedural parent owners conflict"));
                    }
                    continue;
                }
                let mut path = String::new();
                path.try_reserve_exact(selected.path.as_str().len())
                    .map_err(|_| limit())?;
                path.push_str(selected.path.as_str());
                parent_at = Some(SourceOrigin {
                    source: SourceIdentity {
                        unit: Key(selected.outline.crate_key.0),
                        content: Key(selected.outline.parse.content.0),
                        path,
                    },
                    range: call.range,
                });
            }
        }
        let mut parents = Vec::new();
        parents.try_reserve_exact(1).map_err(|_| limit())?;
        parents.push(ExpansionOrigin {
            expansion: parent_key.fingerprint(),
            invocation_identity: Some(parent_key.invocation),
            invocation: parent_at,
        });
        let origin = MacroOrigin {
            invocation_identity: self.key.invocation,
            invocation: Some(invocation),
            definition,
            failure_kind: None,
            failure_span_key: None,
            failure_span: None,
            parents,
        };
        Ok(origin)
    }
    pub(crate) fn with_diagnostic_parents(
        mut self,
        parents: Vec<crate::syntax::origin::ExpansionOrigin>,
    ) -> Result<Self, Diagnostic> {
        let origin = self
            .diagnostic_origin
            .as_mut()
            .and_then(Arc::get_mut)
            .ok_or_else(|| fault("generated procedural origin is absent or already shared"))?;
        origin.parents = parents;
        Ok(self)
    }
    pub(crate) fn diagnostic_origin(&self) -> Option<Arc<crate::syntax::origin::MacroOrigin>> {
        self.diagnostic_origin.clone()
    }
    pub fn procedural_request(&self) -> Option<&lrr_expand_adapter::proc_client::ProcInvocation> {
        self.procedural.as_ref()
    }
    pub(crate) fn consumed_compiler_tests(&self) -> usize {
        self.consumed_compiler_tests
    }
    pub(crate) fn procedural_bytes(&self) -> Result<usize, Diagnostic> {
        let Some(request) = &self.procedural else {
            return Ok(0);
        };
        let mut bytes = request
            .input
            .allocated_bytes()
            .map_err(|e| fault(&e.message))?
            .checked_add(request.export.capacity())
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<
                    lrr_expand_adapter::proc_client::ProcInvocation,
                >())
            })
            .ok_or_else(|| fault("generated procedural metadata overflow"))?;
        if let Some(attributes) = &request.attributes {
            bytes = bytes
                .checked_add(
                    attributes
                        .allocated_bytes()
                        .map_err(|e| fault(&e.message))?,
                )
                .ok_or_else(|| fault("generated procedural attribute metadata overflow"))?;
        }
        for source in &request.sources {
            bytes = bytes
                .checked_add(source.heap_bytes().map_err(|e| fault(&e.message))?)
                .ok_or_else(|| fault("generated procedural metadata overflow"))?;
        }
        bytes = bytes
            .checked_add(
                self.diagnostic_origin
                    .as_ref()
                    .map_or(Some(0), |origin| origin.heap_bytes())
                    .ok_or_else(|| fault("generated procedural origin metadata overflow"))?,
            )
            .ok_or_else(|| fault("generated procedural origin metadata overflow"))?;
        bytes = bytes
            .checked_add(
                request
                    .sources
                    .capacity()
                    .checked_mul(std::mem::size_of::<
                        lrr_expand_adapter::proc_client::CapturedSpanSource,
                    >())
                    .ok_or_else(|| fault("generated procedural source metadata overflow"))?,
            )
            .ok_or_else(|| fault("generated procedural metadata limit"))?;
        Ok(bytes)
    }
    pub(crate) fn procedural_catalog(&self) -> Option<Digest> {
        self.procedural_catalog
    }
    pub fn is_compiler(&self) -> bool {
        self.compiler
    }
    pub(crate) fn compiler_input(
        &self,
    ) -> (
        &MacroDefinition,
        &crate::syntax::token_wire::TokenForest,
        crate::syntax::token_wire::Key,
    ) {
        let call = &self.items.calls()[self.call];
        (&self.definition, &call.input, call.site)
    }
    pub fn parent_record(&self) -> &ExpansionRecord {
        self.items.record()
    }
    pub fn key(&self) -> &ExpansionKey {
        &self.key
    }
    /// Borrow only the immutable inputs of this genuine issued request. This
    /// transports source identity to common receipt capture; it executes nothing.
    pub(crate) fn syntax_input(
        &self,
    ) -> Result<
        (
            Digest,
            std::borrow::Cow<'_, lrr_expand_adapter::token_wire::TokenForest>,
            lrr_expand_adapter::token_wire::Key,
        ),
        Diagnostic,
    > {
        let owner = self.definition.defining_unit;
        if let Some(request) = &self.procedural {
            return Ok((
                owner,
                std::borrow::Cow::Borrowed(&request.input),
                request.call_site,
            ));
        }
        let (input, site) = if let Some(attribute) = &self.attribute {
            (&attribute.item, attribute.site)
        } else if let Some((input, site, _)) = &self.derive {
            (input, *site)
        } else {
            let call = self
                .items
                .calls()
                .get(self.call)
                .ok_or_else(|| fault("generated syntax request call input absent"))?;
            (&call.input, call.site)
        };
        // The two pinned wire representations have distinct Rust types. Use
        // their exact codec bridge, preserving token/context/site identities.
        let bytes = input.encode().map_err(|error| fault(&error.message))?;
        let input = lrr_expand_adapter::token_wire::TokenForest::decode(&bytes)
            .map_err(|error| fault(&error.message))?;
        Ok((
            owner,
            std::borrow::Cow::Owned(input),
            lrr_expand_adapter::token_wire::Key(site.0),
        ))
    }
    pub fn expand(&self) -> Result<ExpansionRecord, Diagnostic> {
        if self.is_builtin_derive() {
            return self.enclosing.as_ref().ok_or_else(|| fault("builtin derive request absent"))?.expand();
        }
        if self.procedural.is_some() || self.attribute.is_some() || self.derive.is_some() {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-PERMISSION",
                "generated procedural execution requires the authorized owning MacroRun".into(),
            ));
        }
        if self.compiler {
            return Err(fault(
                "generated compiler expansion requires explicit capture and adoption",
            ));
        }
        if let Some(request) = &self.enclosing {
            return request.expand();
        }
        let input = &self.items.calls()[self.call];
        self.body_items.expand_macro_body(
            self.definition.identity,
            self.definition.defining_unit,
            &self.key,
            &input.input,
            input.site,
        )
    }
}
