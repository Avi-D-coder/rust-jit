//! Captured item attribute selection. No helper execution or namespace completion.
use super::{
    expansion::DemandContext,
    function_macro_requests::PreparedFunctionRequest,
    item_macro_inputs::AttributeInvocationInput,
    item_macro_requests::{PreparedItemRequest, ResolvedItemTarget},
    macro_session::MacroSession,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{scopes::MacroDefinition, source_declarative::SourceDeclarative},
    frontend::{
        macro_names::{MacroNamespaces, PathLookup},
        procedural_exports::PreparedProcCatalog,
    },
    source::changes::PreparedSourceView,
    syntax::{outline::ScopeKey, token_wire::TokenForest},
};
use lrr_expand_adapter::{
    proc_client::{CapturedSpanSource, ProcForm, SourceDigestEncoding},
    token_wire::Key,
};
use std::sync::Arc;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
// This resolver currently routes only the retained consumer namespace. A
// comparison class cannot authorize routing through a foreign item context.
fn generated_item_context_matches_unit(
    facts: &crate::frontend::hygiene::HygieneFacts,
    context: crate::syntax::token_wire::Key,
    unit: Digest,
) -> Result<bool, Diagnostic> {
    Ok(facts.context(context)?.defining_unit.0 == unit.0)
}
fn generated_import_contexts_match(
    adapter: &lrr_expand_adapter::token_wire::TokenForest,
    range: crate::syntax::spans::SourceRange,
    facts: &crate::frontend::hygiene::HygieneFacts,
    expected: (
        crate::syntax::token_wire::NameContextKey,
        crate::syntax::token_wire::Key,
    ),
    grammar: lrr_expand_adapter::declaration_output::DeclarationGrammar,
) -> Result<bool, Diagnostic> {
    let name = lrr_expand_adapter::generated_attribute::import_binding_forest_for_grammar(
        adapter,
        range.start,
        range.end,
        grammar,
    )
    .map_err(|e| fault(&e.message))?;
    let paths = lrr_expand_adapter::generated_attribute::import_path_forests_for_grammar(
        adapter,
        range.start,
        range.end,
        grammar,
    )
    .map_err(|e| fault(&e.message))?;
    for original in std::iter::once(&name).chain(paths.iter()) {
        if original.spans().is_empty() {
            return Ok(false);
        }
        for span in original.spans() {
            let context = crate::syntax::token_wire::Key(span.context.0);
            if (facts.name_item(context)?, facts.item(context)?) != expected {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
pub use crate::syntax::outline::attribute_producer;
use crate::syntax::outline::potential_attribute_producer as selected_attribute_producer;
pub struct SourceAttributeSelection {
    source: PreparedSourceView,
    producer: Digest,
    scope: ScopeKey,
    grammar: crate::expand::identity::GrammarSlot,
    input: AttributeInvocationInput,
    registry: lrr_expand_adapter::token_wire::TokenForest,
    call_site: Key,
    definition: MacroDefinition,
    catalog: Arc<PreparedProcCatalog>,
    completed_derives: Option<Digest>,
    // Exact source positions and negative Attribute-facet bindings, replayed at
    // seal through select(). Registration metadata stays in the captured source.
    consumed_tests: Vec<(usize, Digest)>,
    source_item_index: Option<usize>,
    cold_scan: Option<Arc<crate::syntax::declaration_scan::DeclarationScan>>,
    generated_parent: Option<Arc<crate::frontend::impl_index::GeneratedDeclarationMacroIndex>>,
    generated_sources: Vec<CapturedSpanSource>,
    inherited_helpers:
        Option<Arc<crate::frontend::generated_derive_helpers::OriginalDeriveHelpers>>,
}
impl SourceAttributeSelection {
    pub(crate) fn select(
        source: &PreparedSourceView,
        provider: &SourceDeclarative,
        producer: Digest,
        namespaces: &MacroNamespaces,
    ) -> Result<Option<Self>, Diagnostic> {
        let Some(catalog) = &provider.procedural else {
            return Ok(None);
        };
        let item = source
            .outline()
            .items
            .iter()
            .find(|item| {
                selected_attribute_producer(item).is_some_and(|call| call.item_input == producer)
            })
            .ok_or_else(|| fault("source attribute producer absent"))?;
        let parsed = crate::syntax::parse::ParsedFile::parse(
            source.captured_macro_blob()?,
            source.outline().profile.clone(),
        )?;
        let facts =
            parsed.attribute_inputs_at(item.declaration_span, source.captured_macro_cfg()?)?;
        if !facts.enabled {
            return Err(fault("active source attribute became disabled"));
        }
        let mut consumed_tests = Vec::new();
        let inert_names = namespaces.before_attribute(producer, item.header.fingerprint());
        for (index, attribute) in facts.attributes.iter().enumerate() {
            if crate::expand::builtin_attributes::inert_descriptor(&attribute.name)?.is_some() {
                let path = parsed.attribute_path_at(attribute.range)?;
                if !path.absolute() && path.segments().len() == 1 {
                    let lookup = inert_names.lookup_invocation_segments(
                        path.segments(),
                        false,
                        &item.scope,
                        parsed.key().profile.edition,
                    )?;
                    if crate::expand::builtin_attributes::certify_inert_binding(
                        &attribute.name,
                        &lookup,
                        Some(catalog),
                        provider
                            .compiler
                            .as_ref()
                            .and_then(|c| c.attribute_catalog()),
                    )?
                    .is_none()
                    {
                        return Ok(None);
                    }
                    continue;
                }
            }
            // Derive/helper preparation is a separate unresolved transformation demand.
            // Its occurrence cannot be guessed inert to reach a later attribute.
            if crate::expand::builtin_attributes::descriptor(&attribute.name)?.is_some()
                && attribute.name != "test"
            {
                return Ok(None);
            }
            let path = parsed.attribute_path_at(attribute.range)?;
            let definition = match namespaces.lookup_invocation_segments(
                path.segments(),
                path.absolute(),
                &item.scope,
                parsed.key().profile.edition,
            )? {
                PathLookup::Defined(definition) => definition,
                PathLookup::Ambiguous(_) => {
                    return Err(Diagnostic::new(
                        "LRR-E-MACRO-AMBIGUOUS",
                        "source replacing attribute is ambiguous".into(),
                    ));
                }
                PathLookup::Inaccessible(_) => {
                    return Err(Diagnostic::new(
                        "LRR-E-MACRO-INACCESSIBLE",
                        "source replacing attribute is inaccessible".into(),
                    ));
                }
                _ => return Ok(None),
            };
            if attribute.name == "test"
                && crate::expand::builtin_attributes::permits_builtin_test_marker(
                    &definition,
                    Some(catalog),
                    provider
                        .compiler
                        .as_ref()
                        .and_then(|c| c.attribute_catalog()),
                )?
            {
                let descriptor = crate::expand::builtin_attributes::descriptor("test")?
                    .ok_or_else(|| fault("compiler Test descriptor absent"))?;
                if descriptor.action()
                    != crate::expand::builtin_attributes::BuiltinAttributeAction::Test
                    || consumed_tests.len() >= 4096
                {
                    return Err(fault("compiler Test prefix invalid"));
                }
                consumed_tests.push((index, definition.identity));
                continue;
            }
            if catalog
                .select(
                    &definition,
                    crate::expand::identity::ProceduralKind::Attribute,
                )?
                .is_none()
            {
                return Ok(None);
            }
            let registry = provider.owned_forest()?;
            let forest = TokenForest::decode(&registry.encode().map_err(|e| fault(&e.message))?)
                .map_err(|e| fault(&e.message))?;
            let source_identity = &forest
                .spans()
                .first()
                .ok_or_else(|| fault("source attribute span table absent"))?
                .source;
            let consumed: Vec<_> = consumed_tests.iter().map(|(index, _)| *index).collect();
            let input = super::item_macro_inputs::attribute_after_compiler_prefix(
                item.def_key.fingerprint(),
                &facts,
                index,
                &consumed,
                source_identity,
                &forest,
            )?;
            let mut selection =
                Self::from_resolved(source, provider, producer, definition, input, None, None)?;
            selection.consumed_tests = consumed_tests;
            selection.metadata_bytes()?;
            return Ok(Some(selection));
        }
        Ok(None)
    }
    pub(crate) fn cold_lookup(
        source: &PreparedSourceView,
        scan: &crate::syntax::declaration_scan::DeclarationScan,
        producer: Digest,
        namespaces: &MacroNamespaces,
    ) -> Result<PathLookup, Diagnostic> {
        if scan.parse() != &source.selection().outline.parse
            || scan.path() != &source.selection().path
            || scan.cfg() != source.selection().outline.cfg
        {
            return Err(fault("cold attribute scan/source differs"));
        }
        let row = scan
            .producers()
            .iter()
            .find(|row| {
                row.identity == producer
                    && row.kind
                        == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            })
            .ok_or_else(|| fault("cold attribute producer absent"))?;
        let parsed = crate::syntax::parse::ParsedFile::parse(
            source.captured_macro_blob()?,
            source.outline().profile.clone(),
        )?;
        // Namespace selection is independent of a target item's eligibility.
        // Root inert attributes have an exact source/scope attachment without
        // an ast::Item owner. Effectful selection still validates its target.
        let (segments, absolute) = parsed.attribute_lookup_path_at(row.range)?;
        crate::frontend::declaration_imports::DeclarationImportIndex::new(scan)?
            .lookup(producer, &segments, absolute, namespaces)
    }
    pub(crate) fn select_cold(
        source: &PreparedSourceView,
        provider: &SourceDeclarative,
        scan: Arc<crate::syntax::declaration_scan::DeclarationScan>,
        producer: Digest,
        namespaces: &MacroNamespaces,
    ) -> Result<Option<Self>, Diagnostic> {
        let Some(catalog) = &provider.procedural else {
            return Ok(None);
        };
        let lookup = Self::cold_lookup(source, &scan, producer, namespaces)?;
        let row = scan
            .producers()
            .iter()
            .find(|row| row.identity == producer)
            .ok_or_else(|| fault("cold attribute attachment absent"))?;
        if let Some(name) = row.unqualified_attribute.as_deref()
            && crate::expand::builtin_attributes::inert_descriptor(name)?.is_some()
        {
            crate::expand::builtin_attributes::certify_inert_binding(
                name,
                &lookup,
                Some(catalog),
                provider
                    .compiler
                    .as_ref()
                    .and_then(|c| c.attribute_catalog()),
            )?;
            return Ok(None);
        }
        let definition = match lookup {
            PathLookup::Defined(definition) => definition,
            PathLookup::Ambiguous(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "cold attribute namespace is ambiguous".into(),
                ));
            }
            PathLookup::Inaccessible(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-INACCESSIBLE",
                    "cold attribute namespace is inaccessible".into(),
                ));
            }
            _ => return Ok(None),
        };
        if !catalog.attribute_candidate(&definition)? {
            return Ok(None);
        }
        let row = scan
            .producers()
            .iter()
            .find(|row| row.identity == producer)
            .ok_or_else(|| fault("cold attribute attachment absent"))?;
        let parsed = crate::syntax::parse::ParsedFile::parse(
            source.captured_macro_blob()?,
            source.outline().profile.clone(),
        )?;
        let range = parsed
            .attribute_item_range_at(row.range)?
            .ok_or_else(|| fault("cold attribute owning item absent"))?;
        let facts = parsed.attribute_inputs_at(range, source.captured_macro_cfg()?)?;
        if !facts.enabled || facts.attributes.len() > 4096 {
            return Err(fault("cold attribute inactive item or count limit"));
        }
        let at = facts
            .attributes
            .iter()
            .position(|attribute| attribute.range == row.range)
            .ok_or_else(|| fault("cold attribute effective annotation absent"))?;
        let mut consumed_tests = Vec::new();
        for (index, attribute) in facts.attributes[..at].iter().enumerate() {
            if attribute.name == "cfg"
                || crate::expand::builtin_attributes::inert_descriptor(&attribute.name)?.is_some()
            {
                continue;
            }
            if attribute.name != "test" {
                return Ok(None);
            }
            let path = parsed.attribute_path_at(attribute.range)?;
            let binding = crate::frontend::declaration_imports::DeclarationImportIndex::new(&scan)?
                .lookup(producer, path.segments(), path.absolute(), namespaces)?;
            let PathLookup::Defined(binding) = binding else {
                return Ok(None);
            };
            if !crate::expand::builtin_attributes::permits_builtin_test_marker(
                &binding,
                Some(catalog),
                provider
                    .compiler
                    .as_ref()
                    .and_then(|c| c.attribute_catalog()),
            )? {
                return Ok(None);
            }
            let descriptor = crate::expand::builtin_attributes::descriptor("test")?
                .ok_or_else(|| fault("cold compiler Test descriptor absent"))?;
            if descriptor.action()
                != crate::expand::builtin_attributes::BuiltinAttributeAction::Test
            {
                return Err(fault("cold compiler Test facet differs"));
            }
            consumed_tests.push((index, binding.identity));
        }
        let registry = provider.owned_forest()?;
        let forest = TokenForest::decode(&registry.encode().map_err(|e| fault(&e.message))?)
            .map_err(|e| fault(&e.message))?;
        let source_identity = &forest
            .spans()
            .first()
            .ok_or_else(|| fault("cold attribute source span absent"))?
            .source;
        let consumed: Vec<_> = consumed_tests.iter().map(|(index, _)| *index).collect();
        let input = super::item_macro_inputs::attribute_after_compiler_prefix(
            producer,
            &facts,
            at,
            &consumed,
            source_identity,
            &forest,
        )?;
        let call_site = registry
            .spans()
            .iter()
            .filter(|span| span.start >= row.range.start && span.end <= row.range.end)
            .min_by_key(|span| (span.start, span.end))
            .ok_or_else(|| fault("cold attribute call site absent"))?
            .key();
        let selected = Self {
            source: source.share(),
            producer,
            scope: row.scope.clone(),
            grammar: row.grammar,
            input,
            registry,
            call_site,
            definition,
            catalog: catalog.clone(),
            completed_derives: None,
            consumed_tests,
            source_item_index: None,
            cold_scan: Some(scan),
            generated_parent: None,
            generated_sources: Vec::new(),
            inherited_helpers: None,
        };
        selected.metadata_bytes()?;
        Ok(Some(selected))
    }
    pub(crate) fn generated_lookup(
        parent: &crate::frontend::impl_index::GeneratedDeclarationMacroIndex,
        producer: Digest,
        namespaces: &MacroNamespaces,
        cfg: &crate::source::snapshot::CfgSet,
    ) -> Result<PathLookup, Diagnostic> {
        let output = parent.output();
        if cfg.fingerprint() != output.scan().cfg() {
            return Err(fault("generated attribute lookup cfg differs"));
        }
        let annotation = output
            .scan()
            .producers()
            .iter()
            .find(|row| {
                row.identity == producer
                    && row.kind
                        == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            })
            .ok_or_else(|| fault("generated attribute lookup producer absent"))?;
        let (segments, absolute) = output.attribute_lookup_path(producer)?;
        let grammar = crate::expand::generated_declarations::declaration_grammar(
            output.record().key().grammar,
        );
        let adapter = lrr_expand_adapter::token_wire::TokenForest::decode(output.record().wire())
            .map_err(|e| fault(&e.message))?;
        let path = lrr_expand_adapter::generated_attribute::path_forest_at_meta_for_grammar(
            &adapter,
            annotation.range.start,
            annotation.range.end,
            grammar,
        )
        .map_err(|e| fault(&e.message))?;
        let facts = output.hygiene_facts()?;
        let mut class = None;
        for span in path.spans() {
            let context = crate::syntax::token_wire::Key(span.context.0);
            let current = (facts.name_item(context)?, facts.item(context)?);
            if class.is_some_and(|old| old != current) {
                return Ok(PathLookup::NeedInput(producer));
            }
            class = Some(current);
        }
        if class.is_none() {
            return Ok(PathLookup::NeedInput(producer));
        }
        let expected = class.ok_or_else(|| fault("generated import caller context absent"))?;
        let caller = output
            .scan()
            .producers()
            .iter()
            .find(|row| row.identity == producer)
            .ok_or_else(|| fault("generated attribute caller scope absent"))?;
        if !generated_item_context_matches_unit(&facts, expected.1, caller.scope.crate_key)? {
            return Ok(PathLookup::NeedInput(producer));
        }
        crate::frontend::declaration_imports::DeclarationImportIndex::new(output.scan())?
            .lookup_validated(producer, &segments, absolute, namespaces, |import| {
                generated_import_contexts_match(
                    &adapter,
                    import.source.range,
                    &facts,
                    expected,
                    grammar,
                )
            })
    }
    pub(crate) fn select_generated(
        source: &PreparedSourceView,
        provider: &SourceDeclarative,
        parent: Arc<crate::frontend::impl_index::GeneratedDeclarationMacroIndex>,
        producer: Digest,
        namespaces: &MacroNamespaces,
        providers: &[SourceDeclarative],
    ) -> Result<Option<Self>, Diagnostic> {
        let original = parent.source_index()?.scan();
        if original.parse() != &source.selection().outline.parse
            || original.path() != &source.selection().path
            || original.cfg() != source.selection().outline.cfg
        {
            return Err(fault("generated attribute original attachment differs"));
        }
        let Some(catalog) = &provider.procedural else {
            return Ok(None);
        };
        let cfg = source.captured_macro_cfg()?;
        let lookup = Self::generated_lookup(&parent, producer, namespaces, cfg)?;
        let row = parent
            .output()
            .scan()
            .producers()
            .iter()
            .find(|row| row.identity == producer)
            .ok_or_else(|| fault("generated attribute attachment absent"))?;
        if let Some(name) = row.unqualified_attribute.as_deref()
            && crate::expand::builtin_attributes::inert_descriptor(name)?.is_some()
        {
            crate::expand::builtin_attributes::certify_inert_binding(
                name,
                &lookup,
                Some(catalog),
                provider
                    .compiler
                    .as_ref()
                    .and_then(|c| c.attribute_catalog()),
            )?;
            return Ok(None);
        }
        let definition = match lookup {
            PathLookup::Defined(definition) => definition,
            PathLookup::Ambiguous(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "generated cold attribute is ambiguous".into(),
                ));
            }
            PathLookup::Inaccessible(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-INACCESSIBLE",
                    "generated cold attribute is inaccessible".into(),
                ));
            }
            _ => return Ok(None),
        };
        if !catalog.attribute_candidate(&definition)? {
            return Ok(None);
        }
        let output = parent.output();
        let (facts, at) = output.attribute_facts(producer, cfg)?;
        let mut consumed_tests = Vec::new();
        for (index, attribute) in facts.attributes[..at].iter().enumerate() {
            if attribute.name == "cfg"
                || crate::expand::builtin_attributes::inert_descriptor(&attribute.name)?.is_some()
            {
                continue;
            }
            if attribute.name != "test" {
                return Ok(None);
            }
            let prefix = output
                .scan()
                .producers()
                .iter()
                .find(|row| {
                    row.range == attribute.range
                        && row.kind
                            == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                })
                .ok_or_else(|| fault("generated cold Test prefix attachment absent"))?;
            let PathLookup::Defined(binding) =
                Self::generated_lookup(&parent, prefix.identity, namespaces, cfg)?
            else {
                return Ok(None);
            };
            if !crate::expand::builtin_attributes::permits_builtin_test_marker(
                &binding,
                Some(catalog),
                provider
                    .compiler
                    .as_ref()
                    .and_then(|c| c.attribute_catalog()),
            )? {
                return Ok(None);
            }
            let descriptor = crate::expand::builtin_attributes::descriptor("test")?
                .ok_or_else(|| fault("generated cold compiler Test descriptor absent"))?;
            if descriptor.action()
                != crate::expand::builtin_attributes::BuiltinAttributeAction::Test
            {
                return Err(fault("generated cold compiler Test facet differs"));
            }
            consumed_tests.push((index, binding.identity));
        }
        let registry = lrr_expand_adapter::token_wire::TokenForest::decode(output.record().wire())
            .map_err(|e| fault(&e.message))?;
        let consumed: std::collections::BTreeSet<_> =
            consumed_tests.iter().map(|(index, _)| *index).collect();
        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(facts.attributes.len())
            .map_err(|_| fault("generated cold attribute range allocation"))?;
        let mut selected = None;
        for (index, attribute) in facts.attributes.iter().enumerate() {
            if consumed.contains(&index) {
                continue;
            }
            if index == at {
                selected = Some(ranges.len());
            }
            ranges.push(lrr_expand_adapter::generated_attribute::AttributeRange {
                start: attribute.range.start,
                end: attribute.range.end,
                wrapper_start: attribute.enclosing_attribute_range.start,
                wrapper_end: attribute.enclosing_attribute_range.end,
            });
        }
        let applied = selected.ok_or_else(|| fault("generated cold applied annotation absent"))?;
        let prepared = lrr_expand_adapter::generated_attribute::materialize_for_grammar(
            &registry,
            facts.item.start,
            facts.item.end,
            applied,
            &ranges,
            crate::expand::generated_declarations::declaration_grammar(
                output.record().key().grammar,
            ),
        )
        .map_err(|e| fault(&e.message))?;
        let decode = |tokens: &lrr_expand_adapter::token_wire::TokenForest| {
            TokenForest::decode(&tokens.encode().map_err(|e| fault(&e.message))?)
                .map_err(|e| fault(&e.message))
        };
        let item = decode(&prepared.item)?;
        let arguments = prepared.arguments.as_ref().map(decode).transpose()?;
        let mut identity = StableEncoder::new("lrr/v2/generated-cold-attribute-input");
        identity.digest(output.fingerprint());
        identity.digest(producer);
        identity.digest(Digest::of_bytes(
            &item.encode().map_err(|e| fault(&e.message))?,
        ));
        let identity = identity.finish();
        let input = AttributeInvocationInput {
            source_selection: super::item_macro_inputs::OriginalAttributeSelection {
                parse: facts.parse.clone(),
                owner: producer,
                cfg: facts.cfg,
                item: facts.item,
                meta: facts.attributes[at].range,
                index: at,
                identity,
            },
            identity,
            item,
            arguments,
        };
        let generated_sources = crate::expand::source_declarative::captured_procedural_sources(
            output.tokens(),
            providers,
        )?;
        let row = output
            .scan()
            .producers()
            .iter()
            .find(|row| row.identity == producer)
            .ok_or_else(|| fault("generated attribute producer absent"))?;
        let scope = row.scope.clone();
        let grammar = row.grammar;
        let selected = Self {
            source: source.share(),
            producer,
            scope,
            grammar,
            input,
            registry,
            call_site: prepared.site,
            definition,
            catalog: catalog.clone(),
            completed_derives: None,
            cold_scan: None,
            consumed_tests,
            source_item_index: None,
            generated_parent: Some(parent),
            generated_sources,
            inherited_helpers: None,
        };
        selected.metadata_bytes()?;
        Ok(Some(selected))
    }
    pub(crate) fn generated_parent(
        &self,
    ) -> Option<&Arc<crate::frontend::impl_index::GeneratedDeclarationMacroIndex>> {
        self.generated_parent.as_ref()
    }
    pub(crate) fn revalidate_generated(
        &self,
        provider: &SourceDeclarative,
        namespaces: &MacroNamespaces,
        providers: &[SourceDeclarative],
    ) -> Result<(), Diagnostic> {
        let parent = self
            .generated_parent
            .as_ref()
            .ok_or_else(|| fault("generated attribute parent absent"))?;
        let current = Self::select_generated(
            &self.source,
            provider,
            parent.clone(),
            self.producer,
            namespaces,
            providers,
        )?
        .ok_or_else(|| fault("generated cold attribute selection incomplete before sealing"))?;
        if current.fingerprint() != self.fingerprint() {
            return Err(fault("generated cold attribute changed before sealing"));
        }
        Ok(())
    }
    pub(crate) fn cold_scan(
        &self,
    ) -> Option<&Arc<crate::syntax::declaration_scan::DeclarationScan>> {
        self.cold_scan.as_ref()
    }
    pub(crate) fn item_range(&self) -> crate::syntax::spans::SourceRange {
        self.input.source_selection.item
    }
    pub(crate) fn from_resolved(
        source: &PreparedSourceView,
        provider: &SourceDeclarative,
        producer: Digest,
        definition: MacroDefinition,
        input: AttributeInvocationInput,
        completed_derives: Option<Digest>,
        inherited_helpers: Option<
            Arc<crate::frontend::generated_derive_helpers::OriginalDeriveHelpers>,
        >,
    ) -> Result<Self, Diagnostic> {
        let (item_index, item) = source
            .outline()
            .items
            .iter()
            .enumerate()
            .find(|(_, item)| {
                selected_attribute_producer(item).is_some_and(|call| call.item_input == producer)
            })
            .ok_or_else(|| fault("source attribute producer absent"))?;
        if input.source_selection.owner != item.def_key.fingerprint() {
            return Err(fault("source attribute input owner differs"));
        }
        let catalog = provider
            .procedural
            .as_ref()
            .ok_or_else(|| fault("source attribute catalog absent"))?;
        let registry = provider.owned_forest()?;
        let range = input.source_selection.meta;
        let call_site = registry
            .spans()
            .iter()
            .filter(|span| span.start >= range.start && span.end <= range.end)
            .min_by_key(|span| (span.start, span.end))
            .ok_or_else(|| fault("source attribute site absent"))?
            .key();
        let selected = Self {
            source: source.share(),
            producer,
            scope: item.scope.clone(),
            grammar: crate::expand::identity::GrammarSlot::Items,
            input,
            registry,
            call_site,
            definition,
            catalog: catalog.clone(),
            completed_derives,
            consumed_tests: Vec::new(),
            source_item_index: Some(item_index),
            cold_scan: None,
            generated_parent: None,
            generated_sources: Vec::new(),
            inherited_helpers,
        };
        selected.metadata_bytes()?;
        Ok(selected)
    }
    pub(crate) fn has_completed_derives(&self) -> bool {
        self.completed_derives.is_some()
    }
    pub(crate) fn inherited_helpers(
        &self,
    ) -> Option<&Arc<crate::frontend::generated_derive_helpers::OriginalDeriveHelpers>> {
        self.inherited_helpers.as_ref()
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v2/source-attribute-selection");
        e.digest(self.producer);
        e.record(&self.grammar);
        e.digest(self.input.identity);
        e.digest(self.definition.identity);
        e.digest(self.definition.defining_unit);
        e.u32(u32::from(self.definition.definition_edition));
        e.digest(self.catalog.fingerprint());
        e.option(&self.source_item_index, |e, index| {
            e.u64(u64::try_from(*index).expect("bounded source item index"))
        });
        e.sequence(&self.consumed_tests, |e, (index, definition)| {
            e.u64(u64::try_from(*index).expect("bounded compiler prefix index"));
            e.digest(*definition);
        });
        e.option(&self.cold_scan, |e, scan| e.digest(scan.fingerprint()));
        e.option(&self.generated_parent, |e, parent| {
            e.digest(parent.output().fingerprint())
        });
        e.option(&self.completed_derives, |e, prefix| e.digest(*prefix));
        e.option(&self.inherited_helpers, |e, helpers| {
            e.digest(helpers.fingerprint())
        });
        e.finish()
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .registry
            .allocated_bytes()
            .map_err(|e| fault(&e.message))?
            .checked_add(
                self.input
                    .item
                    .allocated_bytes()
                    .map_err(|e| fault(&e.message))?,
            )
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .and_then(|n| n.checked_add(self.scope.heap_bytes()))
            .and_then(|n| {
                self.consumed_tests
                    .capacity()
                    .checked_mul(std::mem::size_of::<(usize, Digest)>())
                    .and_then(|size| n.checked_add(size))
            })
            .and_then(|n| {
                n.checked_add(
                    self.cold_scan
                        .as_ref()
                        .map_or(0, |scan| scan.retained_bytes()),
                )
            })
            .and_then(|n| {
                n.checked_add(
                    self.inherited_helpers
                        .as_ref()
                        .map_or(0, |h| h.retained_bytes()),
                )
            })
            .ok_or_else(|| fault("source attribute metadata overflow"))?;
        if let Some(arguments) = &self.input.arguments {
            bytes = bytes
                .checked_add(arguments.allocated_bytes().map_err(|e| fault(&e.message))?)
                .ok_or_else(|| fault("source attribute metadata overflow"))?;
        }
        if let Some(parent) = &self.generated_parent {
            bytes = bytes
                .checked_add(parent.retained_metadata_bytes())
                .ok_or_else(|| fault("generated attribute parent size overflow"))?;
        }
        for source in &self.generated_sources {
            bytes = bytes
                .checked_add(source.heap_bytes().map_err(|e| fault(&e.message))?)
                .ok_or_else(|| fault("generated attribute source size overflow"))?;
        }
        if bytes > 32 * 1024 * 1024 {
            return Err(fault("source attribute metadata limit"));
        }
        Ok(bytes)
    }
    pub(crate) fn diagnostic_origin(
        &self,
        key: &crate::expand::identity::ExpansionKey,
    ) -> Result<crate::syntax::origin::MacroOrigin, Diagnostic> {
        use crate::syntax::{
            origin::{DefinitionOrigin, MacroOrigin, SourceOrigin},
            spans::SourceRange,
            token_wire::{Key as OwnedKey, SourceIdentity},
        };
        let export = self
            .catalog
            .select(
                &self.definition,
                crate::expand::identity::ProceduralKind::Attribute,
            )?
            .ok_or_else(|| fault("attribute diagnostic selected export absent"))?;
        if key.invocation != self.producer
            || key.grammar != self.grammar
            || &key.macro_identity != export.identity()
            || key.consumer_compile_env != self.catalog.environment()
            || key.procedural_epoch != Some(self.catalog.epoch())
        {
            return Err(fault(
                "attribute diagnostic differs from selected expansion",
            ));
        }
        let crate::expand::identity::MacroIdentity::Procedural {
            artifact,
            export: name,
            ..
        } = export.identity()
        else {
            return Err(fault("attribute diagnostic export kind differs"));
        };
        let site = self
            .registry
            .spans()
            .iter()
            .find(|span| span.key() == self.call_site)
            .ok_or_else(|| fault("attribute diagnostic call site absent"))?;
        // The proc API call_site remains its original token anchor. Diagnostic
        // labels can use the full selected meta only in that same original
        // capture; generated byte offsets cannot be applied to original text.
        let selected = self.source.selection();
        let original = &self.input.source_selection;
        let site_range = SourceRange::checked(site.start, site.end)?;
        let range = if original.parse == selected.outline.parse
            && site.source.unit.0 == selected.outline.crate_key.0
            && site.source.content.0 == selected.outline.parse.content.0
            && site.source.path == selected.path.as_str()
        {
            if !original.meta.contains(site_range) {
                return Err(fault("attribute diagnostic anchor escapes selected meta"));
            }
            original.meta
        } else {
            site_range
        };
        let invocation = SourceOrigin {
            source: SourceIdentity {
                unit: OwnedKey(site.source.unit.0),
                content: OwnedKey(site.source.content.0),
                path: String::new(),
            },
            range,
        };
        let limit = || {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "attribute diagnostic origin storage limit".into(),
            )
        };
        let retained = std::mem::size_of::<MacroOrigin>()
            .checked_add(site.source.path.len())
            .and_then(|n| n.checked_add(name.len()))
            .and_then(|n| {
                n.checked_add(
                    self.catalog
                        .definition_site(*artifact)
                        .map_or(0, |site| site.source.path.len()),
                )
            })
            .filter(|n| *n <= 64 * 1024)
            .ok_or_else(limit)?;
        let _ = retained;
        let mut path = String::new();
        path.try_reserve_exact(site.source.path.len())
            .map_err(|_| limit())?;
        path.push_str(&site.source.path);
        let invocation = SourceOrigin {
            source: SourceIdentity {
                path,
                ..invocation.source
            },
            ..invocation
        };
        let definition = self
            .catalog
            .definition_site(*artifact)
            .and_then(|site| {
                site.diagnostic_definition(crate::expand::identity::ProceduralKind::Attribute, name)
                    .map(|at| (site, at))
            })
            .map(|(site, at)| {
                let mut export = String::new();
                export.try_reserve_exact(name.len()).map_err(|_| limit())?;
                export.push_str(name);
                let mut path = String::new();
                path.try_reserve_exact(site.source.path.len())
                    .map_err(|_| limit())?;
                path.push_str(&site.source.path);
                Ok::<_, Diagnostic>(DefinitionOrigin {
                    identity: self.definition.identity,
                    name: export,
                    location: SourceOrigin {
                        source: SourceIdentity {
                            unit: site.source.unit,
                            content: site.source.content,
                            path,
                        },
                        range: at,
                    },
                })
            })
            .transpose()?;
        // A generated replacing attribute owns this explicit parent record.
        // Syntax-context ancestry cannot substitute for expansion ancestry.
        let mut parents = Vec::new();
        if let Some(parent) = &self.generated_parent {
            let record = parent.output().record();
            let parent_key = record.key();
            let mut invocation = None;
            for call in self
                .source
                .outline()
                .item_invocations
                .iter()
                .filter(|call| {
                    call.item_input == parent_key.invocation && call.attribute_input.is_none()
                })
            {
                let selected = self.source.selection();
                if selected.path.as_str().len() > 64 * 1024 {
                    return Err(limit());
                }
                let mut path = String::new();
                path.try_reserve_exact(selected.path.as_str().len())
                    .map_err(|_| limit())?;
                path.push_str(selected.path.as_str());
                let at = SourceOrigin {
                    source: SourceIdentity {
                        unit: OwnedKey(selected.outline.crate_key.0),
                        content: OwnedKey(selected.outline.parse.content.0),
                        path,
                    },
                    range: call.range,
                };
                if invocation.is_some() {
                    return Err(fault(
                        "generated attribute parent has duplicate source owners",
                    ));
                }
                invocation = Some(at);
            }
            parents.try_reserve_exact(1).map_err(|_| limit())?;
            parents.push(crate::syntax::origin::ExpansionOrigin {
                expansion: parent_key.fingerprint(),
                invocation_identity: Some(parent_key.invocation),
                invocation,
            });
        }
        let origin = MacroOrigin {
            invocation_identity: key.invocation,
            invocation: Some(invocation),
            definition,
            failure_kind: None,
            failure_span_key: None,
            failure_span: None,
            parents,
        };
        if origin.heap_bytes().is_none_or(|n| n > 64 * 1024) {
            return Err(limit());
        }
        Ok(origin)
    }
    pub(crate) fn matches_request(
        &self,
        request: &PreparedFunctionRequest,
    ) -> Result<bool, Diagnostic> {
        let selected = self
            .catalog
            .select(
                &self.definition,
                crate::expand::identity::ProceduralKind::Attribute,
            )?
            .ok_or_else(|| fault("source attribute selected export absent"))?;
        Ok(request.source() == self.source()
            && request.key().invocation == self.producer
            && request.key().grammar == self.grammar
            && &request.key().macro_identity == selected.identity()
            && request.key().consumer_compile_env == self.catalog.environment()
            && request.source_attribute_proof() == Some(self.fingerprint()))
    }
    pub(crate) fn definition(&self) -> &MacroDefinition {
        &self.definition
    }
    pub(crate) fn source(&self) -> &crate::source::changes::SourceSelection {
        self.source.selection()
    }
    pub(crate) fn producer(&self) -> Digest {
        self.producer
    }
    pub(crate) fn grammar(&self) -> crate::expand::identity::GrammarSlot {
        self.grammar
    }
    /// Test registration precedes subsequent replacing attributes. Its source
    /// metadata is separate from the token forest supplied to those attributes.
    pub(crate) fn consumed_test_registration(
        &self,
    ) -> Result<Option<&crate::syntax::outline::ItemOutline>, Diagnostic> {
        if self.consumed_tests.is_empty()
            || self.cold_scan.is_some()
            || self.generated_parent.is_some()
        {
            // Inner items are transformed but never registered in libtest's module catalog.
            return Ok(None);
        }
        if self.consumed_tests.len() != 1 {
            return Err(fault(
                "multiple compiler Test registrations require separate test descriptors",
            ));
        }
        let index = self
            .source_item_index
            .ok_or_else(|| fault("consumed compiler Test source index absent"))?;
        let item = self
            .source
            .outline()
            .items
            .get(index)
            .filter(|item| item.def_key.fingerprint() == self.input.source_selection.owner)
            .ok_or_else(|| fault("consumed compiler Test source item absent"))?;
        Ok(Some(item))
    }
    pub fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub(crate) fn prepare(
        &self,
        session: &MacroSession,
        context: &DemandContext,
    ) -> Result<PreparedFunctionRequest, Diagnostic> {
        if session.export_catalog(context)?.fingerprint() != self.catalog.fingerprint()
            || context.unit != self.scope.crate_key
        {
            return Err(fault("source attribute session/catalog/consumer differs"));
        }
        let selected = self
            .catalog
            .select(
                &self.definition,
                crate::expand::identity::ProceduralKind::Attribute,
            )?
            .ok_or_else(|| fault("source attribute export absent"))?;
        let crate::expand::identity::MacroIdentity::Procedural {
            artifact, export, ..
        } = selected.identity()
        else {
            return Err(fault("source attribute export kind differs"));
        };
        let site = self
            .registry
            .spans()
            .iter()
            .find(|span| span.key() == self.call_site)
            .ok_or_else(|| fault("source attribute call site absent"))?;
        let mut sources = if self.generated_parent.is_some() {
            self.generated_sources.clone()
        } else {
            vec![
                CapturedSpanSource::new(
                    site.source.clone(),
                    self.source.captured_macro_blob()?.text()?.into(),
                    SourceDigestEncoding::LrrSourceBytes,
                    None,
                )
                .map_err(|e| fault(&e.message))?,
            ]
        };
        let attributes = self
            .input
            .arguments
            .as_ref()
            .map(|tokens| {
                lrr_expand_adapter::token_wire::TokenForest::decode(
                    &tokens.encode().map_err(|e| fault(&e.message))?,
                )
                .map_err(|e| fault(&e.message))
            })
            .transpose()?;
        let definition = session
            .source_definition_site(*artifact)
            .ok_or_else(|| fault("source attribute definition site is not captured"))?;
        let registry = crate::expand::procedural_sites::merge(definition, &self.registry)?;
        let definition_source = crate::expand::procedural_sites::source(definition)?;
        if !sources
            .iter()
            .any(|row| row.source() == definition_source.source())
        {
            sources.push(definition_source);
        }
        let target = ResolvedItemTarget {
            artifact: Key(artifact.0),
            export: export.clone(),
            scope: Key(self.scope.fingerprint().0),
            parent_origin: Key(self
                .generated_parent
                .as_ref()
                .map(|p| p.output().record().key().fingerprint())
                .unwrap_or_else(|| {
                    crate::db::key::QueryKey::Outline(self.source.selection().outline.clone())
                        .fingerprint()
                })
                .0),
            registry: &registry,
            definition_site: Key(definition.site.0),
            call_site: self.call_site,
            mixed_site: self.call_site,
            sources,
        };
        let prepared = PreparedItemRequest::assemble_in_slot(
            session,
            context,
            self.producer,
            &self.input.item,
            attributes,
            (ProcForm::Attribute, self.grammar),
            target,
        )?;
        if &prepared.key().macro_identity != selected.identity() {
            return Err(fault("source attribute prepared identity differs"));
        }
        Ok(PreparedFunctionRequest::from_source_attribute(
            self.source.selection().clone(),
            prepared,
            self.fingerprint(),
        ))
    }
}

#[cfg(test)]
mod import_context_tests {
    use super::*;
    use lrr_expand_adapter::token_wire::{
        ContextRecord, Edition, SourceIdentity, SpanRecord, TokenTree, Transparency,
    };
    fn import_range(
        adapter: &lrr_expand_adapter::token_wire::TokenForest,
        slot: crate::expand::identity::GrammarSlot,
    ) -> crate::syntax::spans::SourceRange {
        let output = lrr_expand_adapter::declaration_output::materialize(
            adapter,
            crate::expand::generated_declarations::declaration_grammar(slot),
        )
        .unwrap();
        let parsed = crate::syntax::parse::ParsedFile::from_generated_declarations(
            &output.syntax,
            output.text.len(),
            Digest(*blake3::hash(&adapter.encode().unwrap()).as_bytes()),
            2024,
            slot,
        )
        .unwrap();
        let scan = parsed
            .scan_declarations(
                crate::source::snapshot::SourcePath::new("generated.rs").unwrap(),
                ScopeKey {
                    crate_key: Digest([29; 32]),
                    module_path: vec![],
                    owner: None,
                },
                &crate::source::snapshot::CfgSet::guest_default(),
                Default::default(),
            )
            .unwrap();
        scan.imports()[0].source.range
    }
    #[test]
    fn generated_import_target_context_cannot_borrow_local_alias_authority() {
        for (slot, text) in [
            (
                crate::expand::identity::GrammarSlot::Items,
                "use host::{one as keep};",
            ),
            (
                crate::expand::identity::GrammarSlot::Statements,
                "use host::{one as keep};",
            ),
            (
                crate::expand::identity::GrammarSlot::Expression,
                "{use host::{one as keep};0}",
            ),
        ] {
            let local = ContextRecord {
                parent: None,
                expansion: None,
                transparency: Transparency::Opaque,
                defining_unit: Key([29; 32]),
                edition: Edition::E2015,
            };
            let original = lrr_expand_adapter::token_wire::parse_source(
                text,
                SourceIdentity {
                    unit: Key([29; 32]),
                    content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
                    path: "generated.rs".into(),
                },
                local.clone(),
                Key([30; 32]),
            )
            .unwrap();
            let owned = TokenForest::decode(&original.encode().unwrap()).unwrap();
            let facts = crate::expand::source_hygiene::output_facts(&owned).unwrap();
            let local_key = crate::syntax::token_wire::Key(local.key().0);
            let expected = (
                facts.name_item(local_key).unwrap(),
                facts.item(local_key).unwrap(),
            );
            assert!(
                generated_import_contexts_match(
                    &original,
                    import_range(&original, slot),
                    &facts,
                    expected,
                    crate::expand::generated_declarations::declaration_grammar(slot)
                )
                .unwrap()
            );
            let foreign = ContextRecord {
                defining_unit: Key([31; 32]),
                ..local.clone()
            };
            let foreign_key = crate::syntax::token_wire::Key(foreign.key().0);
            let mut spans = original.spans().to_vec();
            let mut roots = original.roots().to_vec();
            fn replace(tokens: &mut [TokenTree], spans: &mut Vec<SpanRecord>, foreign: Key) {
                for token in tokens {
                    match token {
                        TokenTree::Group { children, .. } => replace(children, spans, foreign),
                        TokenTree::Ident { text, span, .. } if text == "host" => {
                            let original = spans.iter().find(|s| s.key() == *span).unwrap();
                            let changed = SpanRecord {
                                context: foreign,
                                ..original.clone()
                            };
                            *span = changed.key();
                            spans.push(changed);
                        }
                        _ => {}
                    }
                }
            }
            replace(&mut roots, &mut spans, foreign.key());
            let mut contexts = original.contexts().to_vec();
            contexts.push(foreign);
            let mixed =
                lrr_expand_adapter::token_wire::TokenForest::new(roots, contexts, spans).unwrap();
            let range = import_range(&mixed, slot);
            let alias = lrr_expand_adapter::generated_attribute::import_binding_forest_for_grammar(
                &mixed,
                range.start,
                range.end,
                crate::expand::generated_declarations::declaration_grammar(slot),
            )
            .unwrap();
            assert!(alias.spans().iter().all(|s| s.context == local.key()));
            let owned = TokenForest::decode(&mixed.encode().unwrap()).unwrap();
            let facts = crate::expand::source_hygiene::output_facts(&owned).unwrap();
            assert_eq!(
                facts.name_item(local_key).unwrap(),
                facts.name_item(foreign_key).unwrap()
            );
            assert_ne!(
                facts.item(local_key).unwrap(),
                facts.item(foreign_key).unwrap()
            );
            assert!(
                generated_item_context_matches_unit(
                    &facts,
                    facts.item(local_key).unwrap(),
                    Digest([29; 32])
                )
                .unwrap()
            );
            assert!(
                !generated_item_context_matches_unit(
                    &facts,
                    facts.item(foreign_key).unwrap(),
                    Digest([29; 32])
                )
                .unwrap()
            );
            assert!(
                !generated_import_contexts_match(
                    &mixed,
                    range,
                    &facts,
                    expected,
                    crate::expand::generated_declarations::declaration_grammar(slot)
                )
                .unwrap()
            );
        }
    }
}
