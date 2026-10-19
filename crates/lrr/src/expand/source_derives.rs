//! Original-source compiler annotation proofs; no synthetic expansion records.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{scopes::MacroDefinition, source_declarative::SourceDeclarative},
    frontend::macro_names::{MacroNamespaces, PathLookup},
    service::item_macro_inputs::{self, ResolvedDeriveAnnotations},
    source::{changes::PreparedSourceView, manifest::ItemKind},
    syntax::{parse::ParsedFile, token_wire::TokenForest},
};

#[derive(PartialEq, Eq)]
pub(crate) struct SourceEmptyDeriveProof {
    pub(crate) item: Digest,
    pub(crate) fingerprint: Digest,
    original_macro: Option<OriginalMacroProof>,
}
#[derive(PartialEq, Eq)]
struct OriginalMacroProof {
    source: Digest,
    producer: Digest,
    item_ordinal: usize,
    dialect: crate::syntax::macro_calls::MacroDefinitionDialect,
}
fn source_identity(source: &PreparedSourceView) -> Digest {
    let mut e = StableEncoder::new("lrr/v1/original-no-transform-source");
    e.string(source.selection().path.as_str());
    e.record(&source.selection().outline);
    e.finish()
}
impl SourceEmptyDeriveProof {
    pub(crate) fn original_macro<'a>(
        &self,
        source: &'a PreparedSourceView,
        producer: Digest,
    ) -> Result<
        Option<(
            usize,
            &'a crate::syntax::outline::ItemOutline,
            crate::syntax::macro_calls::MacroDefinitionDialect,
        )>,
        Diagnostic,
    > {
        let Some(proof) = &self.original_macro else {
            return Ok(None);
        };
        if proof.source != source_identity(source) || proof.producer != producer {
            return Err(fault("original no-transform macro source/producer differs"));
        }
        let item = source
            .outline()
            .items
            .get(proof.item_ordinal)
            .filter(|item| {
                item.def_key.fingerprint() == self.item && item.def_key.item_kind == ItemKind::Macro
            })
            .ok_or_else(|| fault("original no-transform macro is absent"))?;
        Ok(Some((proof.item_ordinal, item, proof.dialect)))
    }
}
#[derive(PartialEq, Eq)]
pub(crate) struct SourceDeriveCompletionProof {
    pub(crate) item: Digest,
    pub(crate) fingerprint: Digest,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-EXPANSION-INPUT", message.into())
}
fn selected_definition(lookup: PathLookup) -> Result<Option<MacroDefinition>, Diagnostic> {
    match lookup {
        PathLookup::Defined(definition) => Ok(Some(definition)),
        PathLookup::Ambiguous(candidates) => Err(Diagnostic::new(
            "LRR-E-MACRO-AMBIGUOUS",
            format!("original source derive annotation or path is ambiguous: {candidates:?}"),
        )),
        PathLookup::Inaccessible(_) => Err(Diagnostic::new(
            "LRR-E-MACRO-INACCESSIBLE",
            "original source derive annotation or path is inaccessible".into(),
        )),
        PathLookup::Absent | PathLookup::NeedInput(_) => Ok(None),
    }
}
fn inert(
    parsed: &ParsedFile,
    attr: &crate::syntax::attributes::AttributeInput,
) -> Result<bool, Diagnostic> {
    let path = parsed.attribute_path_at(attr.range)?;
    Ok(!path.absolute()
        && path.segments().len() == 1
        && (attr.name == "cfg"
            || crate::expand::builtin_attributes::retained_descriptor(&attr.name)?.is_some()))
}
#[cfg(test)]
pub(crate) fn prepare_empty(
    source: &PreparedSourceView,
    provider: &SourceDeclarative,
    producer: Digest,
    names: &MacroNamespaces,
) -> Result<Option<SourceEmptyDeriveProof>, Diagnostic> {
    prepare_empty_with_annotations(source, provider, producer, names, None)
}
pub(crate) fn prepare_empty_with_annotations(
    source: &PreparedSourceView,
    provider: &SourceDeclarative,
    producer: Digest,
    names: &MacroNamespaces,
    annotations: Option<&super::source_declarative::original_annotations::OriginalAnnotationPlan>,
) -> Result<Option<SourceEmptyDeriveProof>, Diagnostic> {
    let item = if let Some(plan) = annotations {
        let ordinal = plan.item_ordinal(producer)?;
        source
            .outline()
            .items
            .get(ordinal)
            .ok_or_else(|| fault("original item annotation ordinal is absent"))?
    } else {
        source
            .outline()
            .items
            .iter()
            .find(|item| {
                crate::syntax::outline::potential_attribute_producer(item)
                    .is_some_and(|c| c.item_input == producer)
            })
            .ok_or_else(|| fault("original derive producer absent"))?
    };
    if let Some(plan) = annotations
        && provider
            .compiler
            .as_ref()
            .and_then(|c| c.attribute_catalog())
            .is_some()
        && let Some(rows) = plan.rows(source, producer, item.def_key.fingerprint())?
    {
        if !source.same_publication(provider.module_source())
            || source.selection() != provider.module_source().selection()
        {
            return Err(fault("original no-transform item provider/source differs"));
        }
        let selected_names = names.before_attribute(producer, item.header.fingerprint());
        let mut e = StableEncoder::new("lrr/v1/original-inert-item-completion");
        e.digest(source_identity(source));
        e.digest(producer);
        e.record(&item.def_key);
        e.record(&item.header);
        let mut complete = true;
        let mut count = 0usize;
        for row in rows {
            let Some(mut proof) =
                super::nested_inert::NestedInertProof::select_original_plan(plan, row.scan_row)?
            else {
                complete = false;
                break;
            };
            // Test/derive actions retain their existing handlers. Only genuine
            // pinned inert descriptors certify an unchanged original item.
            if super::builtin_attributes::inert_descriptor(proof.name()?)?.is_none() {
                complete = false;
                break;
            }
            let lookup = selected_names.lookup_invocation_segments(
                &row.segments,
                row.absolute,
                &item.scope,
                source.outline().profile.edition,
            )?;
            let Some(binding) = super::builtin_attributes::certify_inert_binding(
                proof.name()?,
                &lookup,
                provider.procedural.as_deref(),
                provider
                    .compiler
                    .as_ref()
                    .and_then(|c| c.attribute_catalog()),
            )?
            else {
                complete = false;
                break;
            };
            proof = proof.with_attribute_facet(binding);
            e.digest(proof.fingerprint());
            count += 1;
        }
        if complete
            && count
                == item
                    .header
                    .attributes
                    .iter()
                    .filter(|attribute| crate::syntax::outline::attribute_may_name_macro(attribute))
                    .count()
        {
            let original_macro = if item.def_key.item_kind == ItemKind::Macro {
                let shape = source.macro_definition_shape_at(item.declaration_span)?;
                e.u8(match shape.dialect() {
                    crate::syntax::macro_calls::MacroDefinitionDialect::Rules => 0,
                    crate::syntax::macro_calls::MacroDefinitionDialect::Macro2 => 1,
                });
                Some(OriginalMacroProof {
                    source: source_identity(source),
                    producer,
                    item_ordinal: plan.item_ordinal(producer)?,
                    dialect: shape.dialect(),
                })
            } else {
                e.u8(2);
                None
            };
            return Ok(Some(SourceEmptyDeriveProof {
                item: item.def_key.fingerprint(),
                fingerprint: e.finish(),
                original_macro,
            }));
        }
    }
    if item.def_key.item_kind == ItemKind::Function
        && item.header.attributes.iter().any(|a| a.name == "test")
    {
        let names = names.before_attribute(producer, item.header.fingerprint());
        let mut encoder =
            StableEncoder::new("lrr/v2/compiler-test-selected-action-and-inert-facet-proof");
        encoder.digest(
            crate::db::key::QueryKey::Outline(source.selection().outline.clone()).fingerprint(),
        );
        encoder.record(&item.header);
        encoder.digest(producer);
        for attribute in &item.header.attributes {
            if attribute.name == "test" {
                let lookup = names.lookup_invocation_segments(
                    &[crate::syntax::imports::ImportSegment::Name("test".into())],
                    false,
                    &item.scope,
                    source.outline().profile.edition,
                )?;
                match lookup {
                    PathLookup::Defined(definition) => {
                        if !super::builtin_attributes::permits_builtin_test_marker(
                            &definition,
                            provider.procedural.as_deref(),
                            provider
                                .compiler
                                .as_ref()
                                .and_then(|c| c.attribute_catalog()),
                        )? {
                            return Ok(None);
                        }
                        encoder.digest(definition.identity);
                        encoder.digest(definition.defining_unit);
                    }
                    PathLookup::Absent => {}
                    _ => return Ok(None),
                }
                let descriptor = super::builtin_attributes::descriptor("test")?
                    .ok_or_else(|| fault("compiler test facet absent"))?;
                encoder.digest(descriptor.definition_identity());
            } else if super::builtin_attributes::inert_descriptor(&attribute.name)?.is_some() {
                let lookup = names.lookup_invocation_segments(
                    &[crate::syntax::imports::ImportSegment::Name(
                        attribute.name.clone(),
                    )],
                    false,
                    &item.scope,
                    source.outline().profile.edition,
                )?;
                let Some(binding) = super::builtin_attributes::certify_inert_binding(
                    &attribute.name,
                    &lookup,
                    provider.procedural.as_deref(),
                    provider
                        .compiler
                        .as_ref()
                        .and_then(|c| c.attribute_catalog()),
                )?
                else {
                    return Ok(None);
                };
                encoder.digest(binding);
            } else if attribute.name != "cfg" {
                return Ok(None);
            }
        }
        return Ok(Some(SourceEmptyDeriveProof {
            item: item.def_key.fingerprint(),
            fingerprint: encoder.finish(),
            original_macro: None,
        }));
    }
    let Some(builtin) = provider
        .compiler
        .as_ref()
        .and_then(|c| c.attribute_catalog())
    else {
        return Ok(None);
    };
    if !matches!(
        item.def_key.item_kind,
        ItemKind::Struct | ItemKind::Enum | ItemKind::Union
    ) {
        return Ok(None);
    }
    let target = source
        .original_item_at(item.declaration_span)?
        .ok_or_else(|| fault("source derive canonical item target absent"))?;
    let item = target.item();
    let parsed = source.retained_declaration_parser()?.as_ref();
    let facts =
        parsed.derive_item_inputs_at(item.declaration_span, source.captured_macro_cfg()?)?;
    if !facts.item.enabled {
        return Err(fault("original derive inactive item or annotation limit"));
    }
    let registry = provider.owned_forest()?;
    let forest = TokenForest::decode(&registry.encode().map_err(|e| fault(&e.message))?)
        .map_err(|e| fault(&e.message))?;
    let identity = &forest
        .spans()
        .first()
        .ok_or_else(|| fault("original derive span source absent"))?
        .source;
    let mut certified = Vec::new();
    let mut definitions: Vec<(usize, MacroDefinition)> = Vec::new();
    certified
        .try_reserve_exact(facts.item.attributes.len())
        .map_err(|_| fault("original annotation proof allocation"))?;
    definitions
        .try_reserve_exact(facts.item.attributes.len())
        .map_err(|_| fault("original annotation definition allocation"))?;
    for (index, attr) in facts.item.attributes.iter().enumerate() {
        if inert(&parsed, attr)? {
            continue;
        }
        let path = parsed.attribute_path_at(attr.range)?;
        let Some(definition) = selected_definition(names.lookup_invocation_segments(
            path.segments(),
            path.absolute(),
            &item.scope,
            source.outline().profile.edition,
        )?)?
        else {
            return Ok(None);
        };
        let Some(descriptor) = builtin.select(&definition)? else {
            return Ok(None);
        };
        if descriptor.action() != super::builtin_attributes::BuiltinAttributeAction::Derive {
            return Ok(None);
        }
        let arguments = super::attributes::argument_forest(&facts.item, index, identity, &forest)?
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-MACRO-OUTPUT",
                    "derive annotation arguments absent".into(),
                )
            })?;
        let arguments = lrr_expand_adapter::token_wire::TokenForest::decode(
            &arguments.encode().map_err(|e| fault(&e.to_string()))?,
        )
        .map_err(|e| Diagnostic::new(e.code, e.message))?;
        let paths = lrr_expand_adapter::derive_paths::parse(&arguments)
            .map_err(|e| Diagnostic::new(e.code, e.message))?;
        if !paths.is_empty() {
            return Ok(None);
        }
        certified.push((index, descriptor.clone()));
        definitions.push((index, definition));
    }
    let Some(&(index, _)) = certified.first() else {
        return Ok(None);
    };
    for member in facts.members.iter().filter(|m| m.enabled) {
        for attr in &member.attributes {
            if !inert(&parsed, attr)? {
                return Ok(None);
            }
        }
    }
    let owner = item.def_key.fingerprint();
    let annotations = ResolvedDeriveAnnotations::new(owner, &facts, certified)?;
    let input =
        item_macro_inputs::derive_resolved(owner, &facts, index, &annotations, identity, &forest)?;
    if !input.selections.is_empty() {
        return Err(fault("empty source annotation proof differs"));
    }
    let mut e = StableEncoder::new("lrr/v2/source-empty-derive-proof");
    e.digest(crate::db::key::QueryKey::Outline(source.selection().outline.clone()).fingerprint());
    e.string(source.selection().path.as_str());
    e.record(&item.header);
    e.digest(producer);
    e.digest(input.identity);
    e.u64(
        u64::try_from(definitions.len())
            .map_err(|_| fault("original annotation count overflow"))?,
    );
    for (index, definition) in definitions {
        e.u64(u64::try_from(index).map_err(|_| fault("original annotation index overflow"))?);
        e.digest(definition.identity);
        e.digest(definition.defining_unit);
        e.u32(u32::from(definition.definition_edition));
    }
    Ok(Some(SourceEmptyDeriveProof {
        item: owner,
        fingerprint: e.finish(),
        original_macro: None,
    }))
}

pub(crate) enum SourceDerivePoll {
    NeedDeclaration(
        std::sync::Arc<crate::frontend::generated_derive_exports::DeriveDeclarationRequest>,
    ),
    Ready(std::sync::Arc<crate::service::source_derive_requests::SourceDeriveSelection>),
    Attribute(std::sync::Arc<crate::service::source_attribute_requests::SourceAttributeSelection>),
    Complete(SourceDeriveCompletionProof),
    Pending,
}
/// Replay completed occurrences and resolve the next group with exact receipts.
type DeclarationReceipts = std::collections::BTreeMap<
    Digest,
    (
        std::sync::Arc<crate::frontend::generated_derive_exports::DeriveDeclarationRequest>,
        std::sync::Arc<super::derive_receipt::PreparedDeriveDeclaration>,
    ),
>;
/// One resolved derive path of a group: a procedural export with its exact
/// helper declaration receipt, or a library builtin derive whose helper
/// metadata is the compiler-owned descriptor's.
enum SourceRowKind {
    Procedural(
        crate::frontend::generated_derive_exports::DeriveDeclarationRequest,
        std::sync::Arc<super::derive_receipt::PreparedDeriveDeclaration>,
    ),
    Builtin {
        definition: MacroDefinition,
        kind: lrr_expand_adapter::builtin_macros::BuiltinDeriveKind,
        demand: Digest,
        receipt: Digest,
        helpers: &'static [&'static str],
    },
}
struct SourceRow {
    kind: SourceRowKind,
    site: crate::syntax::token_wire::Key,
}
impl SourceRow {
    fn demand(&self) -> Digest {
        match &self.kind {
            SourceRowKind::Procedural(request, _) => request.demand(),
            SourceRowKind::Builtin { demand, .. } => *demand,
        }
    }
    fn receipt(&self) -> Digest {
        match &self.kind {
            SourceRowKind::Procedural(_, receipt) => receipt.fingerprint(),
            SourceRowKind::Builtin { receipt, .. } => *receipt,
        }
    }
    fn helpers(&self) -> Vec<&str> {
        match &self.kind {
            SourceRowKind::Procedural(_, receipt) => {
                receipt.helpers().iter().map(String::as_str).collect()
            }
            SourceRowKind::Builtin { helpers, .. } => helpers.to_vec(),
        }
    }
}
type SourceRows = Vec<SourceRow>;
enum SourceRowsPoll {
    Ready(SourceRows),
    NeedDeclaration(
        std::sync::Arc<crate::frontend::generated_derive_exports::DeriveDeclarationRequest>,
    ),
    Pending,
}
struct SourceGroups<'a> {
    source: &'a PreparedSourceView,
    item: &'a crate::syntax::outline::ItemOutline,
    producer: Digest,
    facts: &'a crate::syntax::attributes::ItemAttributeInputs,
    forest: &'a TokenForest,
    names: &'a MacroNamespaces,
    catalog: Option<&'a std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>>,
    compiler: Option<&'a crate::frontend::compile_input_catalog::CompileInputCatalog>,
    resolved: &'a DeclarationReceipts,
}
impl SourceGroups<'_> {
    fn rows(&self, index: usize) -> Result<SourceRowsPoll, Diagnostic> {
        let source = self.source;
        let item = self.item;
        let producer = self.producer;
        let facts = self.facts;
        let forest = self.forest;
        let names = self.names;
        let resolved = self.resolved;
        let source_identity = &forest
            .spans()
            .first()
            .ok_or_else(|| fault("source derive span absent"))?
            .source;
        let arguments = super::attributes::argument_forest(facts, index, source_identity, forest)?
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-MACRO-OUTPUT",
                    "derive annotation arguments absent".into(),
                )
            })?;
        let arguments = lrr_expand_adapter::token_wire::TokenForest::decode(
            &arguments.encode().map_err(|e| fault(&e.to_string()))?,
        )
        .map_err(|e| Diagnostic::new(e.code, e.message))?;
        let paths = lrr_expand_adapter::derive_paths::parse(&arguments)
            .map_err(|e| Diagnostic::new(e.code, e.message))?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(paths.len())
            .map_err(|_| fault("source derive receipt allocation"))?;
        for (ordinal, path) in paths.iter().enumerate() {
            let segments = path
                .segments
                .iter()
                .map(|s| crate::syntax::imports::ImportSegment::Name(s.clone()))
                .collect::<Vec<_>>();
            let Some(selected) = selected_definition(names.lookup_invocation_segments(
                &segments,
                path.absolute,
                &item.scope,
                source.outline().profile.edition,
            )?)?
            else {
                return Ok(SourceRowsPoll::Pending);
            };
            let mut e = StableEncoder::new("lrr/v2/source-derive-occurrence");
            e.digest(
                crate::db::key::QueryKey::Outline(source.selection().outline.clone()).fingerprint(),
            );
            e.string(source.selection().path.as_str());
            e.digest(producer);
            e.u64(u64::try_from(index).map_err(|_| fault("source annotation index overflow"))?);
            e.u64(u64::try_from(ordinal).map_err(|_| fault("source derive occurrence overflow"))?);
            e.bytes(&path.first_span.0);
            e.bytes(&path.last_span.0);
            e.u8(u8::from(path.absolute));
            e.sequence(&path.segments, |e, s| e.string(s));
            let occurrence = e.finish();
            let site = crate::syntax::token_wire::Key(path.last_span.0);
            // A library `#[rustc_builtin_macro]` derive: compiler-owned helper
            // metadata, expanded in process by the pinned adapter.
            if let Some(export) = self
                .compiler
                .map(|compiler| {
                    super::builtin_derives::BuiltinDeriveExport::select(
                        compiler, &selected, occurrence,
                    )
                })
                .transpose()?
                .flatten()
            {
                rows.push(SourceRow {
                    kind: SourceRowKind::Builtin {
                        definition: selected,
                        kind: export.kind,
                        demand: export.demand,
                        receipt: export.receipt,
                        helpers: export.helpers,
                    },
                    site,
                });
                continue;
            }
            let Some(catalog) = self.catalog else {
                return Ok(SourceRowsPoll::Pending);
            };
            let Some(request) =
                crate::frontend::generated_derive_exports::DeriveDeclarationRequest::from_source(
                    source, item, producer, occurrence, selected, catalog,
                )?
            else {
                return Ok(SourceRowsPoll::Pending);
            };
            if let Some((old, receipt)) = resolved.get(&request.demand()) {
                if old.fingerprint() != request.fingerprint() {
                    return Err(fault("source derive receipt context changed"));
                }
                receipt.validate_request(&request)?;
                rows.push(SourceRow {
                    kind: SourceRowKind::Procedural(request, receipt.clone()),
                    site,
                });
            } else {
                return Ok(SourceRowsPoll::NeedDeclaration(std::sync::Arc::new(
                    request,
                )));
            }
        }
        Ok(SourceRowsPoll::Ready(rows))
    }
}
#[derive(Default)]
struct SourceHelperScope {
    providers: std::collections::BTreeMap<
        (super::scopes::MacroName, crate::syntax::token_wire::Key),
        std::collections::BTreeSet<usize>,
    >,
    receipt_demands: std::collections::BTreeSet<Digest>,
    receipts: Vec<Digest>,
    retained: usize,
}
impl SourceHelperScope {
    fn admit(
        &mut self,
        index: usize,
        rows: &SourceRows,
        forest: &TokenForest,
        edition: u16,
    ) -> Result<(), Diagnostic> {
        for row in rows {
            let site = &row.site;
            if self.receipt_demands.contains(&row.demand()) {
                continue;
            }
            self.retained = self
                .retained
                .checked_add(256)
                .ok_or_else(|| fault("source helper receipt metadata limit"))?;
            let context = forest
                .spans()
                .iter()
                .find(|span| span.key() == *site)
                .ok_or_else(|| fault("source helper introduction span absent"))?
                .context;
            for name in row.helpers() {
                self.retained = self
                    .retained
                    .checked_add(name.len())
                    .and_then(|n| n.checked_add(256))
                    .ok_or_else(|| fault("source helper scope metadata limit"))?;
                self.providers
                    .entry((super::scopes::MacroName::parse(name, edition)?, context))
                    .or_default()
                    .insert(index);
            }
            self.receipts
                .try_reserve_exact(1)
                .map_err(|_| fault("source helper receipt allocation"))?;
            self.receipts.push(row.receipt());
            self.receipt_demands.insert(row.demand());
        }
        Ok(())
    }
    fn member(
        &self,
        name: &super::scopes::MacroName,
        context: crate::syntax::token_wire::Key,
    ) -> bool {
        self.providers.contains_key(&(name.clone(), context))
    }
    fn outer(
        &self,
        name: &super::scopes::MacroName,
        context: crate::syntax::token_wire::Key,
        at: usize,
    ) -> bool {
        self.providers
            .get(&(name.clone(), context))
            .is_some_and(|positions| positions.iter().any(|index| *index < at))
    }
}
/// rustc `has_derive_copy`/`has_derive_ord`: whether any derive annotation
/// of the item names the builtin `Copy`/`Ord`. `None` while any derive path of
/// the item is unresolved.
#[allow(clippy::too_many_arguments)]
fn container_facts(
    source: &PreparedSourceView,
    item: &crate::syntax::outline::ItemOutline,
    parsed: &ParsedFile,
    facts: &crate::syntax::attributes::ItemAttributeInputs,
    forest: &TokenForest,
    names: &MacroNamespaces,
    builtin: &crate::frontend::builtin_attributes::BuiltinAttributeCatalog,
    compiler: Option<&crate::frontend::compile_input_catalog::CompileInputCatalog>,
) -> Result<Option<lrr_expand_adapter::builtin_macros::DeriveContainerFacts>, Diagnostic> {
    use lrr_expand_adapter::builtin_macros::{BuiltinDeriveKind, DeriveContainerFacts};
    let Some(compiler) = compiler else {
        return Ok(None);
    };
    let edition = source.outline().profile.edition;
    let source_identity = &forest
        .spans()
        .first()
        .ok_or_else(|| fault("source derive span absent"))?
        .source;
    let mut container = DeriveContainerFacts::default();
    for (index, attr) in facts.attributes.iter().enumerate() {
        if inert(parsed, attr)? {
            continue;
        }
        let path = parsed.attribute_path_at(attr.range)?;
        let PathLookup::Defined(definition) = names.lookup_invocation_segments(
            path.segments(),
            path.absolute(),
            &item.scope,
            edition,
        )?
        else {
            continue;
        };
        if !builtin.select(&definition)?.is_some_and(|descriptor| {
            descriptor.action() == super::builtin_attributes::BuiltinAttributeAction::Derive
        }) {
            continue;
        }
        let Some(arguments) =
            super::attributes::argument_forest(facts, index, source_identity, forest)?
        else {
            continue;
        };
        let arguments = lrr_expand_adapter::token_wire::TokenForest::decode(
            &arguments.encode().map_err(|e| fault(&e.to_string()))?,
        )
        .map_err(|e| Diagnostic::new(e.code, e.message))?;
        for path in lrr_expand_adapter::derive_paths::parse(&arguments)
            .map_err(|e| Diagnostic::new(e.code, e.message))?
        {
            let segments = path
                .segments
                .iter()
                .map(|s| crate::syntax::imports::ImportSegment::Name(s.clone()))
                .collect::<Vec<_>>();
            let Some(definition) = selected_definition(names.lookup_invocation_segments(
                &segments,
                path.absolute,
                &item.scope,
                edition,
            )?)?
            else {
                return Ok(None);
            };
            match compiler.builtin_derive(&definition) {
                Some(BuiltinDeriveKind::Copy) => container.derives_copy = true,
                Some(BuiltinDeriveKind::Ord) => container.derives_ord = true,
                _ => {}
            }
        }
    }
    Ok(Some(container))
}
pub(crate) fn prepare_group(
    source: &PreparedSourceView,
    provider: &SourceDeclarative,
    producer: Digest,
    names: &MacroNamespaces,
    completed: &std::collections::BTreeMap<
        Digest,
        std::sync::Arc<crate::service::source_derive_requests::SourceDeriveSelection>,
    >,
    resolved: &std::collections::BTreeMap<
        Digest,
        (
            std::sync::Arc<crate::frontend::generated_derive_exports::DeriveDeclarationRequest>,
            std::sync::Arc<super::derive_receipt::PreparedDeriveDeclaration>,
        ),
    >,
) -> Result<SourceDerivePoll, Diagnostic> {
    let Some(builtin) = provider
        .compiler
        .as_ref()
        .and_then(|c| c.attribute_catalog())
    else {
        return Ok(SourceDerivePoll::Pending);
    };
    // Procedural derives need the run's catalog; builtin derives do not.
    let catalog = provider.procedural.as_ref();
    let Some(target) = source.original_annotation_producer(producer)? else {
        return Ok(SourceDerivePoll::Pending);
    };
    let item = target.item();
    if !matches!(
        item.def_key.item_kind,
        ItemKind::Struct | ItemKind::Enum | ItemKind::Union
    ) {
        return Ok(SourceDerivePoll::Pending);
    }
    let parsed = source.retained_declaration_parser()?.as_ref();
    let inputs =
        parsed.derive_item_inputs_at(item.declaration_span, source.captured_macro_cfg()?)?;
    let facts = &inputs.item;
    if !facts.enabled {
        return Err(fault("source derive annotation limit or inactive producer"));
    }
    let registry = provider.owned_forest()?;
    let forest = TokenForest::decode(&registry.encode().map_err(|e| fault(&e.message))?)
        .map_err(|e| fault(&e.message))?;
    let source_identity = &forest
        .spans()
        .first()
        .ok_or_else(|| fault("source derive span absent"))?
        .source;
    let mut certified = Vec::new();
    certified
        .try_reserve_exact(facts.attributes.len())
        .map_err(|_| fault("source derive proof allocation"))?;
    let groups = SourceGroups {
        source,
        item,
        producer,
        facts,
        forest: &forest,
        names,
        catalog,
        compiler: provider.compiler.as_deref(),
        resolved,
    };
    let container = container_facts(
        source,
        item,
        &parsed,
        facts,
        &forest,
        names,
        builtin,
        provider.compiler.as_deref(),
    )?;
    let mut helpers = SourceHelperScope::default();
    let mut completed_count = 0usize;
    let mut completion = StableEncoder::new("lrr/v2/source-derive-completion");
    completion.digest(
        crate::db::key::QueryKey::Outline(source.selection().outline.clone()).fingerprint(),
    );
    completion.string(source.selection().path.as_str());
    completion.digest(producer);
    completion.record(&item.header);
    completion.digest(item_macro_inputs::derive_facts_fingerprint(&inputs)?);
    for (index, attr) in facts.attributes.iter().enumerate() {
        completion
            .u64(u64::try_from(index).map_err(|_| fault("source annotation index overflow"))?);
        if inert(&parsed, attr)? {
            completion.u8(0);
            continue;
        }
        let path = parsed.attribute_path_at(attr.range)?;
        if !path.absolute()
            && let [crate::syntax::imports::ImportSegment::Name(name)] = path.segments()
        {
            let span = forest
                .spans()
                .iter()
                .filter(|span| {
                    span.start >= attr.range.start
                        && span.end <= attr.range.end
                        && &span.source == source_identity
                })
                .min_by_key(|span| (span.start, span.end))
                .ok_or_else(|| fault("source outer helper span absent"))?;
            let name = super::scopes::MacroName::parse(name, source.outline().profile.edition)?;
            if helpers.outer(&name, span.context, index) {
                completion.u8(1);
                completion.string(name.as_str());
                completion.bytes(&span.context.0);
                continue;
            }
        }
        let Some(definition) = selected_definition(names.lookup_invocation_segments(
            path.segments(),
            path.absolute(),
            &item.scope,
            source.outline().profile.edition,
        )?)?
        else {
            return Ok(SourceDerivePoll::Pending);
        };
        let Some(descriptor) = builtin.select(&definition)? else {
            let Some(catalog) = catalog else {
                return Ok(SourceDerivePoll::Pending);
            };
            if certified.is_empty()
                || catalog
                    .select(&definition, super::identity::ProceduralKind::Attribute)?
                    .is_none()
            {
                return Ok(SourceDerivePoll::Pending);
            }
            // Every preceding nonempty occurrence has been replayed against its
            // retained completed selection before this continuation can be issued.
            completion.u8(3);
            completion.digest(definition.identity);
            completion.u64(
                u64::try_from(completed_count)
                    .map_err(|_| fault("source completed count overflow"))?,
            );
            completion.sequence(&helpers.receipts, |e, fingerprint| e.digest(*fingerprint));
            let proof =
                ResolvedDeriveAnnotations::new(item.def_key.fingerprint(), &inputs, certified)?;
            let input = item_macro_inputs::attribute_after_derives(
                item.def_key.fingerprint(),
                &inputs,
                index,
                &proof,
                source_identity,
                &forest,
            )?;
            let prefix = completion.finish();
            let mut names = Vec::new();
            for ((name, context), positions) in &helpers.providers {
                // Lookahead metadata never introduces a later group's helper.
                if positions.iter().any(|at| *at < index) {
                    names
                        .try_reserve_exact(1)
                        .map_err(|_| fault("source inherited helper allocation"))?;
                    names.push((
                        name.as_str().to_owned(),
                        provider.item_name_context(*context)?,
                    ));
                }
            }
            let environment = std::sync::Arc::new(
                crate::frontend::generated_derive_helpers::OriginalDeriveHelpers::new(
                    producer, prefix, names,
                )?,
            );
            let selection =
                crate::service::source_attribute_requests::SourceAttributeSelection::from_resolved(
                    source,
                    provider,
                    producer,
                    definition,
                    input,
                    Some(prefix),
                    Some(environment),
                )?;
            return Ok(SourceDerivePoll::Attribute(std::sync::Arc::new(selection)));
        };
        if descriptor.action() != super::builtin_attributes::BuiltinAttributeAction::Derive {
            return Ok(SourceDerivePoll::Pending);
        }
        completion.u8(2);
        completion.digest(definition.identity);
        completion.digest(definition.defining_unit);
        completion.u32(u32::from(definition.definition_edition));
        certified.push((index, descriptor.clone()));
        let rows = match groups.rows(index)? {
            SourceRowsPoll::Ready(rows) => rows,
            SourceRowsPoll::NeedDeclaration(request) => {
                return Ok(SourceDerivePoll::NeedDeclaration(request));
            }
            SourceRowsPoll::Pending => return Ok(SourceDerivePoll::Pending),
        };
        // Later groups and replacing attributes remain in the input. Enabled
        // Member annotations may demand exact helper declarations from later groups;
        // outer annotations still require a preceding introduction.
        if !rows.is_empty() {
            helpers.admit(index, &rows, &forest, source.outline().profile.edition)?;
            for member in inputs.members.iter().filter(|m| m.enabled) {
                for attr in &member.attributes {
                    if inert(&parsed, attr)? {
                        continue;
                    }
                    let path = parsed.attribute_path_at(attr.range)?;
                    let [crate::syntax::imports::ImportSegment::Name(name)] = path.segments()
                    else {
                        return Ok(SourceDerivePoll::Pending);
                    };
                    if path.absolute() {
                        return Ok(SourceDerivePoll::Pending);
                    }
                    let span = forest
                        .spans()
                        .iter()
                        .filter(|span| {
                            span.start >= attr.range.start
                                && span.end <= attr.range.end
                                && &span.source == source_identity
                        })
                        .min_by_key(|span| (span.start, span.end))
                        .ok_or_else(|| fault("source member helper span absent"))?;
                    let name =
                        super::scopes::MacroName::parse(name, source.outline().profile.edition)?;
                    if !helpers.member(&name, span.context) {
                        for later in index + 1..facts.attributes.len() {
                            let attr = &facts.attributes[later];
                            if inert(&parsed, attr)? {
                                continue;
                            }
                            let path = parsed.attribute_path_at(attr.range)?;
                            let PathLookup::Defined(definition) = names
                                .lookup_invocation_segments(
                                    path.segments(),
                                    path.absolute(),
                                    &item.scope,
                                    source.outline().profile.edition,
                                )?
                            else {
                                continue;
                            };
                            if !builtin.select(&definition)?.is_some_and(|descriptor| {
                                descriptor.action()
                                    == super::builtin_attributes::BuiltinAttributeAction::Derive
                            }) {
                                continue;
                            }
                            match groups.rows(later)? {
                                SourceRowsPoll::NeedDeclaration(request) => {
                                    return Ok(SourceDerivePoll::NeedDeclaration(request));
                                }
                                SourceRowsPoll::Pending => continue,
                                SourceRowsPoll::Ready(rows) => helpers.admit(
                                    later,
                                    &rows,
                                    &forest,
                                    source.outline().profile.edition,
                                )?,
                            }
                            if helpers.member(&name, span.context) {
                                break;
                            }
                        }
                        if !helpers.member(&name, span.context) {
                            return Ok(SourceDerivePoll::Pending);
                        }
                    }
                }
            }
            let proof = ResolvedDeriveAnnotations::new(
                item.def_key.fingerprint(),
                &inputs,
                certified.clone(),
            )?;
            for (ordinal, row) in rows.iter().enumerate() {
                let input = item_macro_inputs::derive_occurrence_resolved(
                    item.def_key.fingerprint(),
                    &inputs,
                    (index, ordinal),
                    &proof,
                    source_identity,
                    &forest,
                )?;
                let selection = match &row.kind {
                    SourceRowKind::Procedural(request, _) => {
                        crate::service::source_derive_requests::SourceDeriveSelection::new(
                            source,
                            item.scope.clone(),
                            input,
                            (*registry).clone(),
                            &request.clone(),
                            catalog
                                .ok_or_else(|| fault("procedural derive catalog absent"))?
                                .clone(),
                            helpers.receipts.clone(),
                        )?
                    }
                    SourceRowKind::Builtin {
                        definition, kind, ..
                    } => {
                        use lrr_expand_adapter::builtin_macros::BuiltinDeriveKind;
                        // Only `Clone` and `PartialOrd` consult the container's
                        // other derives (rustc `has_derive_copy`/`has_derive_ord`).
                        let facts = match kind {
                            BuiltinDeriveKind::Clone | BuiltinDeriveKind::PartialOrd => {
                                let Some(facts) = container else {
                                    return Ok(SourceDerivePoll::Pending);
                                };
                                facts
                            }
                            _ => Default::default(),
                        };
                        crate::service::source_derive_requests::SourceDeriveSelection::builtin(
                            source,
                            item.scope.clone(),
                            input,
                            (*registry).clone(),
                            crate::expand::derive_origin::DeriveDeclarationOrigin::Source {
                                selection: source.selection().clone(),
                                producer,
                                item: item.def_key.fingerprint(),
                                header: item.header.fingerprint(),
                            },
                            definition.clone(),
                            *kind,
                            facts,
                            helpers.receipts.clone(),
                        )?
                    }
                };
                if let Some(old) = completed.get(&selection.input().identity()) {
                    if old.fingerprint() != selection.fingerprint() {
                        return Err(fault("completed original derive selection changed"));
                    }
                    completed_count = completed_count
                        .checked_add(1)
                        .ok_or_else(|| fault("source completion occurrence limit"))?;
                    completion.digest(selection.fingerprint());
                    continue;
                }
                return Ok(SourceDerivePoll::Ready(std::sync::Arc::new(selection)));
            }
        }
    }
    if completed_count == 0 {
        return Ok(SourceDerivePoll::Pending);
    }
    completion
        .u64(u64::try_from(completed_count).map_err(|_| fault("source completed count overflow"))?);
    completion.sequence(&helpers.receipts, |e, fingerprint| e.digest(*fingerprint));
    Ok(SourceDerivePoll::Complete(SourceDeriveCompletionProof {
        item: item.def_key.fingerprint(),
        fingerprint: completion.finish(),
    }))
}

#[cfg(test)]
mod compiler_test_inert_binding_tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        db::key::OutlineKey,
        expand::{identity::ProceduralKind, scopes::MacroName},
        frontend::{
            macro_names::MacroPath,
            procedural_exports::{PreparedProcBinding, PreparedProcCatalog},
        },
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    use std::{collections::BTreeMap, sync::Arc};

    fn captured(
        name: &str,
        kind: ProceduralKind,
    ) -> (
        PreparedSourceView,
        SourceDeclarative,
        MacroNamespaces,
        Digest,
    ) {
        let profile = ProfileIdentity::current();
        let mut cfg = CfgSet::guest_default();
        cfg.atoms.insert("test".into());
        let blob = SourceBlob::new(format!("#[test] #[{name}] fn selected() {{}}\n").into_bytes())
            .unwrap();
        let path = SourcePath::new("inert-attribute-proof.rs").unwrap();
        let unit = Digest::of_bytes(b"inert attribute proof consumer");
        let selection = SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let inputs = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile,
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
            panic!("captured test source absent");
        };
        // Model captured export metadata only. These pure tests neither execute
        // a macro nor substitute for the separate actual-server fixture.
        let catalog = Arc::new(
            PreparedProcCatalog::from_exports(
                [(
                    Digest::of_bytes(b"inert proof tool artifact"),
                    Digest::of_bytes(b"inert proof producer"),
                    2024,
                    "identity_attr",
                    kind,
                )],
                PreparedProcBinding {
                    consumer: unit,
                    graph: Digest::of_bytes(b"inert proof graph"),
                    epoch: 1,
                    environment: Digest::of_bytes(b"inert proof environment"),
                    preparation: Digest::of_bytes(b"inert proof preparation"),
                    aliases: BTreeMap::new(),
                },
            )
            .unwrap(),
        );
        let mut names = MacroNamespaces::default();
        names
            .define(
                MacroPath::new(unit, vec![MacroName::parse(name, 2024).unwrap()]).unwrap(),
                catalog.exports().next().unwrap().definition().clone(),
            )
            .unwrap();
        let mut provider = SourceDeclarative::new(view.share()).unwrap();
        provider.install_procedural(catalog).unwrap();
        let producer =
            crate::syntax::outline::potential_attribute_producer(&view.outline().items[0])
                .unwrap()
                .item_input;
        (view, provider, names, producer)
    }

    #[test]
    fn compiler_test_proof_rejects_competing_inert_attribute_facet() {
        for name in ["ignore", "should_panic"] {
            let (view, provider, names, producer) = captured(name, ProceduralKind::Attribute);
            let result = prepare_empty(&view, &provider, producer, &names);
            let error = match result {
                Err(error) => error,
                Ok(_) => {
                    panic!("competing {name} Attribute facet was certified as builtin metadata")
                }
            };
            assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
        }
    }

    #[test]
    fn compiler_test_proof_keeps_same_named_function_like_facet() {
        for name in ["ignore", "should_panic"] {
            let (view, provider, names, producer) = captured(name, ProceduralKind::Function);
            assert!(
                prepare_empty(&view, &provider, producer, &names)
                    .unwrap()
                    .is_some()
            );
        }
    }

    #[test]
    fn compiler_test_proof_waits_for_unresolved_inert_attribute_binding() {
        let (view, provider, mut names, producer) = captured("ignore", ProceduralKind::Function);
        names
            .pending(
                MacroPath::new(
                    view.outline().root_scope.crate_key,
                    vec![MacroName::parse("ignore", 2024).unwrap()],
                )
                .unwrap(),
                Digest::of_bytes(b"unresolved competing inert attribute producer"),
            )
            .unwrap();
        assert!(
            prepare_empty(&view, &provider, producer, &names)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn compiler_test_proof_fingerprint_binds_negative_attribute_facet_identity() {
        let (view, provider, names, producer) = captured("ignore", ProceduralKind::Function);
        let present = prepare_empty(&view, &provider, producer, &names)
            .unwrap()
            .unwrap();
        let absent = prepare_empty(&view, &provider, producer, &MacroNamespaces::default())
            .unwrap()
            .unwrap();
        assert_ne!(present.fingerprint, absent.fingerprint);
    }
    #[test]
    fn compiler_owned_attribute_alias_cannot_certify_inert_test_metadata() {
        for export in ["test", "derive"] {
            for name in ["ignore", "should_panic"] {
                let (view, mut provider, _, producer) = captured(name, ProceduralKind::Function);
                let compiler = Arc::new(
                    crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                        &ProfileIdentity::current(),
                    )
                    .unwrap(),
                );
                let catalog = compiler.attribute_catalog().unwrap();
                let definition = catalog
                    .definitions()
                    .find(|d| catalog.select(d).unwrap().unwrap().name() == export)
                    .unwrap()
                    .clone();
                let mut names = MacroNamespaces::default();
                names
                    .define(
                        MacroPath::new(
                            view.outline().root_scope.crate_key,
                            vec![MacroName::parse(name, 2024).unwrap()],
                        )
                        .unwrap(),
                        definition,
                    )
                    .unwrap();
                provider.procedural = None;
                provider.compiler = Some(compiler);
                let error = match prepare_empty(&view, &provider, producer, &names) {
                    Err(error) => error,
                    Ok(_) => panic!(
                        "compiler-owned {export} aliased as {name} was certified as inert metadata"
                    ),
                };
                assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
            }
        }
    }
    #[test]
    fn compiler_owned_bang_alias_preserves_inert_test_metadata() {
        for name in ["ignore", "should_panic"] {
            let (view, mut provider, _, producer) = captured(name, ProceduralKind::Function);
            let compiler = Arc::new(
                crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                    &ProfileIdentity::current(),
                )
                .unwrap(),
            );
            let definition = compiler
                .definitions()
                .find(|d| compiler.select(d).unwrap().unwrap().name() == "concat")
                .unwrap()
                .clone();
            let mut names = MacroNamespaces::default();
            names
                .define(
                    MacroPath::new(
                        view.outline().root_scope.crate_key,
                        vec![MacroName::parse(name, 2024).unwrap()],
                    )
                    .unwrap(),
                    definition,
                )
                .unwrap();
            provider.procedural = None;
            provider.compiler = Some(compiler);
            assert!(
                prepare_empty(&view, &provider, producer, &names)
                    .unwrap()
                    .is_some()
            );
        }
    }
}
