//! Immutable source-header macro index. Bodies and macro token trees stay opaque;
//! generation is explicit input demand, never an expansion/query side effect.
use super::{
    hygiene::HygieneFacts,
    macro_origins::ExpansionAncestry,
    macro_scopes::{HygienicLookup, MacroDeclarations, MacroReference},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    expand::scopes::{MacroDefinition, MacroName, ScopeKey, TextualScopes},
    syntax::{
        outline::{Outline, ProducerKind, ScopeKey as SourceScope, TextualEventKind},
        token_wire::{Edition, Key},
    },
};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[derive(Clone)]
pub struct SourceMacroIndex {
    item_attributes: BTreeMap<(SourceScope, u32, u32), Vec<crate::syntax::spans::SourceRange>>,
    item_invocations: BTreeMap<(SourceScope, u32, u32), Option<Digest>>,
    attribute_candidates: BTreeMap<(SourceScope, u32, u32), crate::syntax::spans::SourceRange>,
    source_key: crate::syntax::parse::ParseKey,
    cfg: Digest,
    scopes: std::sync::Arc<TextualScopes>,
    scope_keys: BTreeMap<SourceScope, ScopeKey>,
    root: SourceScope,
    declarations: std::sync::Arc<MacroDeclarations>,
    definitions: BTreeMap<Digest, MacroDefinition>,
    producers: BTreeMap<Digest, ProducerKind>,
    module_inputs: BTreeMap<Digest, SourceScope>,
    macro_use_inputs: BTreeSet<Digest>,
    imported_definitions: BTreeSet<Digest>,
    restored_original:
        BTreeMap<Digest, (usize, crate::syntax::macro_calls::MacroDefinitionDialect)>,
    imported_bytes: usize,
    facts: std::sync::Arc<HygieneFacts>,
    context: Key,
    ancestry: std::sync::Arc<ExpansionAncestry>,
    /// Names this index state, process-wide: assigned at construction and
    /// replaced by every `&mut self` method. Clones share a stamp only while
    /// they hold identical state, so equal stamps imply equal state.
    stamp: u64,
}
fn next_stamp() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
/// Borrowed actual declaration facts. This grants neither namespace selection
/// nor a body/source entry; current selected-expansion authority is joined by
/// the owning module receipt service.
pub(crate) struct SourceMacroDefinitionModule<'a> {
    item: &'a crate::syntax::outline::ItemOutline,
    context: Key,
}
impl SourceMacroDefinitionModule<'_> {
    pub(crate) fn item(&self) -> &crate::syntax::outline::ItemOutline {
        self.item
    }
    pub(crate) fn context(&self) -> Key {
        self.context
    }
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
fn edition(value: Edition) -> u16 {
    match value {
        Edition::E2015 => 2015,
        Edition::E2018 => 2018,
        Edition::E2021 => 2021,
        Edition::E2024 => 2024,
    }
}
impl SourceMacroIndex {
    /// The process-unique name of this index state (see the `stamp` field).
    pub(crate) fn stamp(&self) -> u64 {
        self.stamp
    }
    /// Explicit cold identity of the actual immutable macro environment. Cache
    /// the result at coordinator receipt mint; never rehash it during dispatch.
    pub(crate) fn prepare_semantic_identity(&self) -> Result<Digest, Diagnostic> {
        self.metadata_bytes()?;
        let mut e = crate::core::digest::StableEncoder::new("lrr/v1/source-macro-environment");
        e.record(&self.source_key);
        e.digest(self.cfg);
        e.record(&self.root);
        e.digest(Digest(self.context.0));
        e.digest(self.scopes.semantic_fingerprint()?);
        e.digest(self.declarations.semantic_fingerprint()?);
        e.digest(self.facts.semantic_fingerprint()?);
        e.digest(self.ancestry.semantic_fingerprint()?);
        e.u64(self.scope_keys.len() as u64);
        for (source, key) in &self.scope_keys {
            e.record(source);
            e.record(key);
        }
        e.u64(self.item_attributes.len() as u64);
        for ((scope, start, end), ranges) in &self.item_attributes {
            e.record(scope);
            e.u32(*start);
            e.u32(*end);
            e.sequence(ranges, |e, range| {
                e.u32(range.start);
                e.u32(range.end);
            });
        }
        e.u64(self.item_invocations.len() as u64);
        for ((scope, start, end), producer) in &self.item_invocations {
            e.record(scope);
            e.u32(*start);
            e.u32(*end);
            e.option(producer, |e, p| e.digest(*p));
        }
        e.u64(self.attribute_candidates.len() as u64);
        for ((scope, start, end), range) in &self.attribute_candidates {
            e.record(scope);
            e.u32(*start);
            e.u32(*end);
            e.u32(range.start);
            e.u32(range.end);
        }
        e.u64(self.definitions.len() as u64);
        for (id, definition) in &self.definitions {
            e.digest(*id);
            e.digest(definition.identity);
            e.digest(definition.defining_unit);
            e.u16(definition.definition_edition);
        }
        e.u64(self.producers.len() as u64);
        for (id, kind) in &self.producers {
            e.digest(*id);
            e.u8(match kind {
                ProducerKind::ItemMacro => 0,
                ProducerKind::Attribute => 1,
                ProducerKind::ModuleSource => 2,
            });
        }
        e.u64(self.module_inputs.len() as u64);
        for (id, scope) in &self.module_inputs {
            e.digest(*id);
            e.record(scope);
        }
        e.u64(self.macro_use_inputs.len() as u64);
        for id in &self.macro_use_inputs {
            e.digest(*id);
        }
        e.u64(self.imported_definitions.len() as u64);
        for id in &self.imported_definitions {
            e.digest(*id);
        }
        e.u64(self.restored_original.len() as u64);
        for (id, (ordinal, dialect)) in &self.restored_original {
            e.digest(*id);
            e.u64(*ordinal as u64);
            e.u8(match dialect {
                crate::syntax::macro_calls::MacroDefinitionDialect::Rules => 0,
                crate::syntax::macro_calls::MacroDefinitionDialect::Macro2 => 1,
            });
        }
        Ok(e.finish())
    }

    pub(crate) fn retain_outer(
        &self,
        outer: super::macro_scopes::OuterMacroScope<'_>,
    ) -> Result<Option<super::macro_boundary::RetainedMacroScope>, Diagnostic> {
        if !std::ptr::eq(outer.scopes, self.scopes.as_ref())
            || !std::ptr::eq(outer.declarations, self.declarations.as_ref())
        {
            return Ok(None);
        }
        Ok(Some(super::macro_boundary::RetainedMacroScope::new(
            std::sync::Arc::clone(&self.scopes),
            std::sync::Arc::clone(&self.declarations),
            std::sync::Arc::clone(&self.facts),
            std::sync::Arc::clone(&self.ancestry),
            outer.scope,
            outer.before,
        )?))
    }
    pub(crate) fn definition_origin(
        &self,
        definition: &MacroDefinition,
    ) -> Result<Option<Option<Key>>, Diagnostic> {
        self.declarations.origin_of(definition)
    }
    pub fn from_outline(
        outline: &Outline,
        context: Key,
        facts: HygieneFacts,
    ) -> Result<Self, Diagnostic> {
        let authority = facts.context(context)?;
        if authority.parent.is_some()
            || authority.defining_unit.0 != outline.root_scope.crate_key.0
            || edition(authority.edition) != outline.profile.edition
        {
            return Err(error("source macro root authority/edition mismatch"));
        }
        facts.name_item(context)?;
        let mut scopes = TextualScopes::default();
        let root = ScopeKey::root(outline.root_scope.fingerprint());
        scopes.add_root(root)?;
        let mut invocation_bytes = 0usize;
        let mut item_invocations = BTreeMap::new();
        for call in &outline.item_invocations {
            invocation_bytes = invocation_bytes
                .checked_add(std::mem::size_of::<crate::syntax::outline::ItemInvocation>())
                .and_then(|n| n.checked_add(call.scope.heap_bytes()))
                .ok_or_else(|| error("source item invocation metadata exceeded"))?;
            if item_invocations
                .insert(
                    (call.scope.clone(), call.range.start, call.range.end),
                    call.attribute_input,
                )
                .is_some()
            {
                return Err(error("duplicate source item invocation range"));
            }
        }
        let mut index = Self {
            item_attributes: BTreeMap::new(),
            item_invocations,
            attribute_candidates: BTreeMap::new(),
            source_key: crate::syntax::parse::ParseKey {
                content: outline.content,
                profile: outline.profile.clone(),
            },
            cfg: outline.cfg,
            scopes: std::sync::Arc::new(scopes),
            scope_keys: BTreeMap::from([(outline.root_scope.clone(), root)]),
            root: outline.root_scope.clone(),
            declarations: std::sync::Arc::new(MacroDeclarations::default()),
            definitions: BTreeMap::new(),
            producers: BTreeMap::new(),
            module_inputs: BTreeMap::new(),
            macro_use_inputs: BTreeSet::new(),
            imported_definitions: BTreeSet::new(),
            restored_original: BTreeMap::new(),
            imported_bytes: 0,
            facts: std::sync::Arc::new(facts),
            context,
            ancestry: std::sync::Arc::new(ExpansionAncestry::default()),
            stamp: next_stamp(),
        };
        let mut attribute_bytes = 0usize;
        for item in &outline.items {
            attribute_bytes = item
                .scope
                .heap_bytes()
                .checked_add(128)
                .and_then(|amount| attribute_bytes.checked_add(amount))
                .ok_or_else(|| error("original item attribute metadata limit"))?;
            let mut ordered = Vec::new();
            for attribute in &item.header.attributes {
                let range = attribute
                    .source_meta
                    .ok_or_else(|| error("original attribute source attachment is missing"))?;
                if range.is_empty()
                    || range.start > range.end
                    || !item.declaration_span.contains(range)
                {
                    return Err(error("original attribute attachment escapes owning item"));
                }
                attribute_bytes = item
                    .scope
                    .heap_bytes()
                    .checked_add(128)
                    .and_then(|amount| attribute_bytes.checked_add(amount))
                    .ok_or_else(|| error("original attribute candidate metadata limit"))?;
                if index
                        .attribute_candidates
                        .insert(
                            (item.scope.clone(), range.start, range.end),
                            item.declaration_span,
                        )
                        .is_some() {
                    return Err(error("duplicate or excessive original attribute candidate"));
                }
                ordered.push(range);
            }
            if ordered.windows(2).any(|pair| pair[0].end > pair[1].start) {
                return Err(error(
                    "original effective attributes are not in lexical order",
                ));
            }
            if index
                .item_attributes
                .insert(
                    (
                        item.scope.clone(),
                        item.declaration_span.start,
                        item.declaration_span.end,
                    ),
                    ordered,
                )
                .is_some()
            {
                return Err(error("duplicate original item declaration range"));
            }
        }
        if outline.scopes.values().any(|scope| {
            scope
                .blockers
                .iter()
                .any(|blocker| blocker.kind == crate::syntax::outline::BlockerKind::ParseBoundary)
        }) {
            return Err(error("source textual declaration boundary is unreliable"));
        }
        let items: BTreeMap<_, _> = outline
            .items
            .iter()
            .map(|item| (item.def_key.fingerprint(), item))
            .collect();
        for event in &outline.textual_events {
            let scope = *index
                .scope_keys
                .get(&event.scope)
                .ok_or_else(|| error("textual source scope opening is missing"))?;
            match &event.kind {
                TextualEventKind::OpenScope { child, ordinal } => {
                    if index.scope_keys.contains_key(child) {
                        return Err(error(&format!(
                            "duplicate source textual scope {}{} at byte {}",
                            child
                                .module_path
                                .iter()
                                .take(16)
                                .map(|part| part.chars().take(64).collect::<String>())
                                .collect::<Vec<_>>()
                                .join("::"),
                            if child.owner.is_some() { " (item owner)" } else { "" },
                            event.at,
                        )));
                    }
                    // A transforming attribute belongs to the opening item.
                    // Include its frontier in the captured parent visibility.
                    let before = event
                        .at
                        .checked_add(1)
                        .ok_or_else(|| error("scope opening position overflow"))?;
                    let key = std::sync::Arc::make_mut(&mut index.scopes)
                        .add_child(scope, before, *ordinal)?;
                    index.scope_keys.insert(child.clone(), key);
                }
                TextualEventKind::Producer { identity, kind } => {
                    if *kind == ProducerKind::ModuleSource {
                        index.module_inputs.insert(*identity, event.scope.clone());
                    }
                    std::sync::Arc::make_mut(&mut index.scopes)
                        .block(scope, event.at, *identity)?;
                    if index.producers.insert(*identity, *kind).is_some() {
                        return Err(error("duplicate source producer identity"));
                    }
                }
                TextualEventKind::Definition {
                    identity,
                    source_name,
                }
                | TextualEventKind::ModuleDefinition {
                    identity,
                    source_name,
                } => {
                    let item = items
                        .get(identity)
                        .ok_or_else(|| error("source macro definition is missing"))?;
                    let definition = MacroDefinition {
                        identity: *identity,
                        defining_unit: item.def_key.crate_key,
                        definition_edition: outline.profile.edition,
                    };
                    std::sync::Arc::make_mut(&mut index.declarations).register(
                        definition.clone(),
                        context,
                        None,
                        &index.facts,
                        &index.ancestry,
                    )?;
                    if matches!(event.kind, TextualEventKind::Definition { .. }) {
                        std::sync::Arc::make_mut(&mut index.scopes).define(
                            scope,
                            event.at,
                            MacroName::parse(source_name, outline.profile.edition)?,
                            definition.clone(),
                        )?;
                    }
                    index.definitions.insert(*identity, definition);
                }
            }
        }
        for item in &outline.items {
            if item.def_key.item_kind != crate::source::manifest::ItemKind::Module
                || !item.header.attributes.iter().any(|attribute| {
                    attribute.name == "macro_use"
                        && attribute.class == crate::syntax::outline::AttributeClass::Harmless
                })
            {
                continue;
            }
            let mut child = item.scope.clone();
            child.module_path.push(item.def_key.declared_name.clone());
            let parent = *index
                .scope_keys
                .get(&item.scope)
                .ok_or_else(|| error("macro-use parent scope absent"))?;
            if let Some(input) = index.module_input_for(&child) {
                index.macro_use_inputs.insert(input);
            }
            let child = *index
                .scope_keys
                .get(&child)
                .ok_or_else(|| error("macro-use child scope absent"))?;
            std::sync::Arc::make_mut(&mut index.scopes).macro_use_module(
                parent,
                child,
                u64::from(item.declaration_span.end),
            )?;
        }
        Ok(index)
    }
    pub(crate) fn reserve_test_attribute(
        &mut self,
        call: &crate::syntax::outline::ItemInvocation,
    ) -> Result<(), Diagnostic> {
        self.stamp = next_stamp();
        if self.producers.contains_key(&call.item_input) {
            return Err(error("test attribute already reserved"));
        }
        if !self.item_attributes.contains_key(&(
            call.scope.clone(),
            call.range.start,
            call.range.end,
        )) {
            return Err(error("test attribute source attachment or count differs"));
        }
        let scope = *self
            .scope_keys
            .get(&call.scope)
            .ok_or_else(|| error("test attribute source scope absent"))?;
        std::sync::Arc::make_mut(&mut self.scopes).block(
            scope,
            u64::from(call.range.start),
            call.item_input,
        )?;
        self.producers
            .insert(call.item_input, ProducerKind::Attribute);
        Ok(())
    }
    pub(crate) fn complete_empty_item_producer(
        &mut self,
        scope: &SourceScope,
        at: u64,
        identity: Digest,
    ) -> Result<(), Diagnostic> {
        self.stamp = next_stamp();
        if !matches!(
            self.producers.get(&identity),
            Some(ProducerKind::ItemMacro | ProducerKind::Attribute)
        ) {
            return Err(error(
                "source item producer does not match completed declarative output",
            ));
        }
        let key = *self
            .scope_keys
            .get(scope)
            .ok_or_else(|| error("completed source macro scope absent"))?;
        std::sync::Arc::make_mut(&mut self.scopes).publish(key, at, identity, vec![])?;
        self.producers.remove(&identity);
        Ok(())
    }
    /// Publish one unchanged original declaration only after the generic item
    /// annotation service has certified its exact source and complete header.
    pub(crate) fn complete_original_inert_macro(
        &mut self,
        source: &crate::source::changes::PreparedSourceView,
        producer: Digest,
        proof: &crate::expand::source_derives::SourceEmptyDeriveProof,
    ) -> Result<bool, Diagnostic> {
        self.stamp = next_stamp();
        let Some((ordinal, item, dialect)) = proof.original_macro(source, producer)? else {
            return Ok(false);
        };
        if self.source_key != source.selection().outline.parse || self.cfg != source.selection().outline.cfg || self.root != source.outline().root_scope || self.producers.get(&producer) != Some(&ProducerKind::Attribute) || self.definitions.contains_key(&proof.item) || self.restored_original.contains_key(&proof.item) {
            return Err(error("original inert macro source/index/producer differs"));
        }
        // Account the new declaration, definition, restored-row and textual
        // position before either shared table changes. The removed producer
        // and pending row deliberately receive no credit during admission.
        self.metadata_bytes()?
            .checked_add(2048)
            .and_then(|bytes| bytes.checked_add(item.def_key.declared_name.len().checked_mul(4)?))
            .ok_or_else(|| error("original inert macro index budget"))?;
        let scope = *self
            .scope_keys
            .get(&item.scope)
            .ok_or_else(|| error("original inert macro scope absent"))?;
        let definition = MacroDefinition {
            identity: proof.item,
            defining_unit: item.def_key.crate_key,
            definition_edition: source.outline().profile.edition,
        };
        let bindings = if dialect == crate::syntax::macro_calls::MacroDefinitionDialect::Rules {
            vec![(
                super::source_macro_routes::semantic(
                    &item.def_key.declared_name,
                    definition.definition_edition,
                )?,
                definition.clone(),
            )]
        } else {
            Vec::new()
        };
        self.declarations.preflight_register(
            &definition,
            self.context,
            None,
            &self.facts,
            &self.ancestry,
        )?;
        self.scopes.preflight_publish(
            scope,
            u64::from(item.declaration_span.start),
            producer,
            &bindings,
        )?;
        // The two shared preflights admit the same immutable tables; each
        // mutation below repeats only its own already-admitted checks.
        Arc::make_mut(&mut self.declarations).register(
            definition.clone(),
            self.context,
            None,
            &self.facts,
            &self.ancestry,
        )?;
        Arc::make_mut(&mut self.scopes).publish(
            scope,
            u64::from(item.declaration_span.start),
            producer,
            bindings,
        )?;
        self.definitions.insert(definition.identity, definition);
        self.restored_original
            .insert(proof.item, (ordinal, dialect));
        self.producers.remove(&producer);
        Ok(true)
    }
    pub(crate) fn restored_original_macros(
        &self,
    ) -> impl Iterator<
        Item = (
            Digest,
            usize,
            crate::syntax::macro_calls::MacroDefinitionDialect,
        ),
    > + '_ {
        self.restored_original
            .iter()
            .map(|(identity, (ordinal, dialect))| (*identity, *ordinal, *dialect))
    }
    pub(crate) fn complete_generated_item_producer(
        &mut self,
        scope: &SourceScope,
        at: u64,
        identity: Digest,
        catalog: &super::generated_macros::GeneratedMacroCatalog,
    ) -> Result<(), Diagnostic> {
        self.stamp = next_stamp();
        self.publish_generated_item_producer(scope, at, identity, catalog, false)
    }
    pub(crate) fn publish_derive_prefix(
        &mut self,
        scope: &SourceScope,
        at: u64,
        producer: Digest,
        occurrence: Digest,
        catalog: &super::generated_macros::GeneratedMacroCatalog,
    ) -> Result<(), Diagnostic> {
        self.stamp = next_stamp();
        if catalog.invocation() != occurrence {
            return Err(error("derive prefix occurrence differs"));
        }
        self.publish_generated_item_producer(scope, at, producer, catalog, true)
    }
    pub(crate) fn derive_prefix_snapshot(
        &self,
        scope: &SourceScope,
        at: u64,
        producer: Digest,
    ) -> Result<Self, Diagnostic> {
        let key = *self
            .scope_keys
            .get(scope)
            .ok_or_else(|| error("source derive scope absent"))?;
        if self.producers.get(&producer) != Some(&ProducerKind::Attribute) {
            return Err(error("source derive producer absent"));
        }
        let mut snapshot = self.snapshot_clone()?;
        snapshot.scopes = std::sync::Arc::new(self.scopes.prefix_view(key, at, producer)?);
        snapshot.producers.remove(&producer);
        Ok(snapshot)
    }
    fn publish_generated_item_producer(
        &mut self,
        scope: &SourceScope,
        at: u64,
        identity: Digest,
        catalog: &super::generated_macros::GeneratedMacroCatalog,
        prefix: bool,
    ) -> Result<(), Diagnostic> {
        self.stamp = next_stamp();
        if !matches!(
            self.producers.get(&identity),
            Some(ProducerKind::ItemMacro | ProducerKind::Attribute)
        ) || !catalog.ready_for_outward()
            || catalog.owner() != scope
            || (!prefix && catalog.invocation() != identity)
        {
            return Err(error("generated outward macro owner/producer differs"));
        }
        let key = *self
            .scope_keys
            .get(scope)
            .ok_or_else(|| error("outward source scope absent"))?;
        self.definitions
            .len()
            .checked_add(catalog.outward().len())
            .ok_or_else(|| error("outward macro definition count limit"))?;
        for entry in catalog.outward() {
            if self.definitions.contains_key(&entry.definition.identity) {
                return Err(error("outward macro identity was already registered"));
            }
        }
        std::sync::Arc::make_mut(&mut self.facts).extend(catalog.facts())?;
        std::sync::Arc::make_mut(&mut self.ancestry).extend(catalog.ancestry())?;
        let bindings = catalog.root_bindings()?;
        for entry in catalog.outward() {
            std::sync::Arc::make_mut(&mut self.declarations).register(
                entry.definition.clone(),
                entry.context,
                entry.origin,
                &self.facts,
                &self.ancestry,
            )?;
            self.definitions
                .insert(entry.definition.identity, entry.definition.clone());
        }
        if prefix {
            std::sync::Arc::make_mut(&mut self.scopes)
                .publish_prefix(key, at, identity, bindings)?;
        } else {
            std::sync::Arc::make_mut(&mut self.scopes).publish(key, at, identity, bindings)?;
            self.producers.remove(&identity);
        }
        Ok(())
    }
    pub(crate) fn outer_scope(
        &self,
        scope: &SourceScope,
        before: u64,
    ) -> Result<super::macro_scopes::OuterMacroScope<'_>, Diagnostic> {
        let key = *self
            .scope_keys
            .get(scope)
            .ok_or_else(|| error("generated call enclosing source scope absent"))?;
        Ok(super::macro_scopes::OuterMacroScope {
            scopes: &self.scopes,
            scope: key,
            before,
            declarations: &self.declarations,
        })
    }
    pub(crate) fn hygiene_facts(&self) -> &HygieneFacts {
        &self.facts
    }
    pub(crate) fn expansion_ancestry(&self) -> &ExpansionAncestry {
        &self.ancestry
    }
    pub fn producer_kind(&self, key: Digest) -> Option<ProducerKind> {
        self.producers.get(&key).copied()
    }
    pub fn definition(&self, key: Digest) -> Option<&MacroDefinition> {
        self.definitions.get(&key)
    }
    /// One cold exact identity join to the source that actually built this
    /// index. Imported aliases and unrelated source/header spellings cannot
    /// supply a defining module. Generated declarations need their real
    /// canonical declaration/context receipt, not original root substitution.
    pub(crate) fn source_definition_module<'a>(
        &self,
        source: &'a crate::source::changes::PreparedSourceView,
        definition: &MacroDefinition,
    ) -> Result<Option<SourceMacroDefinitionModule<'a>>, Diagnostic> {
        if self.source_key != source.selection().outline.parse
            || self.cfg != source.selection().outline.cfg
            || self.root != source.outline().root_scope
        {
            return Err(error("macro declaration module source/index differs"));
        }
        if self.definition(definition.identity) != Some(definition)
            || !self.owns_definition(definition.identity)
        {
            return Ok(None);
        }
        // This is a positive physical-source proof. Missing actual generated
        // declaration attachment stays pending in the receipt service.
        if source.require_original_hygiene().is_err() {
            return Ok(None);
        }
        let mut rows = source
            .outline()
            .items
            .iter()
            .filter(|item| item.def_key.fingerprint() == definition.identity);
        let Some(item) = rows.next() else {
            return Ok(None);
        };
        if rows.next().is_some()
            || item.def_key.item_kind != crate::source::manifest::ItemKind::Macro
            || item.scope.owner.is_some()
            || !source.outline().scopes.contains_key(&item.scope)
            || !self.scope_keys.contains_key(&item.scope)
            || definition.defining_unit != item.def_key.crate_key
            || definition.definition_edition != source.outline().profile.edition
        {
            return Err(error("macro declaration module identity/scope differs"));
        }
        let mut events = source.outline().textual_events.iter().filter(|event| {
            matches!(event.kind,
                TextualEventKind::Definition { identity, .. }
                | TextualEventKind::ModuleDefinition { identity, .. }
                if identity == definition.identity)
        });
        match events.next() {
            Some(event) if event.scope == item.scope && events.next().is_none() => {}
            Some(_) => return Err(error("macro declaration module event/scope differs")),
            None => {
                // The existing generic annotation proof can restore an actual
                // unchanged declaration whose outline event was its producer.
                // Reuse that admitted row; do not reinterpret its attributes.
                let Some((ordinal, _)) = self.restored_original.get(&definition.identity) else {
                    return Ok(None);
                };
                if source.outline().items.get(*ordinal).is_none_or(|row| {
                    row.def_key.fingerprint() != definition.identity
                        || row.scope != item.scope
                        || row.declaration_span != item.declaration_span
                }) {
                    return Err(error("restored macro declaration module row differs"));
                }
            }
        }
        let (context, origin) = self.declarations.metadata_of(definition)?;
        let authority = self.facts.context(context)?;
        if origin.is_some()
            || context != self.context
            || authority.parent.is_some()
            || authority.defining_unit.0 != definition.defining_unit.0
        {
            return Err(error("original macro declaration context differs"));
        }
        // Existing exact syntax capture checks the actual declaration AST name;
        // no whole-file syntax search or name-to-module mapping is introduced.
        let (label, name) = source.macro_definition_header(definition.identity)?;
        if !item.declaration_span.contains(label) || name != item.def_key.declared_name {
            return Err(error("macro declaration exact syntax label differs"));
        }
        Ok(Some(SourceMacroDefinitionModule { item, context }))
    }
    pub fn definition_count(&self) -> usize {
        self.definitions.len()
    }
    /// Original item attribute candidate routing; sealing remains a query-provider duty.
    pub fn lookup_attribute_candidate(
        &self,
        attribute: &crate::syntax::macro_calls::SourceAttributePath,
        namespaces: &super::macro_names::MacroNamespaces,
        scope: &SourceScope,
    ) -> Result<super::macro_names::PathLookup, Diagnostic> {
        use super::macro_names::PathLookup;
        if !self.attribute_eligible(attribute, scope)? {
            return Ok(PathLookup::Absent);
        }
        match namespaces.source_segments_path(
            attribute.segments(),
            attribute.absolute(),
            scope,
            attribute.key().profile.edition,
        )? {
            Some(path) => namespaces.lookup_from(&path, scope),
            None => Ok(PathLookup::Absent),
        }
    }
    pub(crate) fn cfg(&self) -> Digest {
        self.cfg
    }
    pub(crate) fn original_item_attributes(
        &self,
        key: &crate::syntax::parse::ParseKey,
        scope: &SourceScope,
        item: crate::syntax::spans::SourceRange,
    ) -> Result<Option<&[crate::syntax::spans::SourceRange]>, Diagnostic> {
        if key != &self.source_key
            || scope.crate_key != self.root.crate_key
            || scope.owner.is_some()
            || item.start > item.end
        {
            return Err(error(
                "original item attribute cursor source/scope mismatch",
            ));
        }
        Ok(self
            .item_attributes
            .get(&(scope.clone(), item.start, item.end))
            .map(Vec::as_slice))
    }
    pub(crate) fn attribute_eligible(
        &self,
        attribute: &crate::syntax::macro_calls::SourceAttributePath,
        scope: &SourceScope,
    ) -> Result<bool, Diagnostic> {
        if attribute.key() != &self.source_key
            || scope.crate_key != self.root.crate_key
            || scope.owner.is_some()
        {
            return Err(error("source attribute identity or scope mismatch"));
        }
        if attribute.requires_body_entry() || attribute.requires_container_entry() {
            return Err(error(
                "source attribute requires entered body/container context",
            ));
        }
        let mut module = self.root.module_path.clone();
        module
            .len()
            .checked_add(attribute.module_suffix().len())            .ok_or_else(|| error("source attribute module depth exceeded"))?;
        module.extend_from_slice(attribute.module_suffix());
        if module != scope.module_path {
            return Err(error("source attribute enclosing module mismatch"));
        }
        match self.attribute_candidates.get(&(
            scope.clone(),
            attribute.range().start,
            attribute.range().end,
        )) {
            Some(owner) if *owner == attribute.owner() => {}
            Some(_) => {
                return Err(error(
                    "source attribute owning item differs from indexed declaration",
                ));
            }
            None => return Ok(false),
        }
        Ok(true)
    }
    /// Original item-position calls only; executable bodies need an entered frame.
    pub fn lookup_invocation(
        &self,
        call: &crate::syntax::macro_calls::SourceMacroCall,
        namespaces: &super::macro_names::MacroNamespaces,
        scope: &SourceScope,
    ) -> Result<super::macro_names::PathLookup, Diagnostic> {
        self.lookup_invocation_using(call, namespaces, scope, |module, before, name| {
            self.lookup(module, before, name)
        })
    }
    /// Pure declaration discovery may resolve body syntax without an execution grant.
    /// The caller must attach the call to its captured declaration inventory.
    pub(crate) fn lookup_declaration_invocation_using(
        &self,
        call: &crate::syntax::macro_calls::SourceMacroCall,
        namespaces: &super::macro_names::MacroNamespaces,
        scope: &SourceScope,
        lookup: impl FnOnce(&[String], u64, &str) -> Result<HygienicLookup, Diagnostic>,
    ) -> Result<super::macro_names::PathLookup, Diagnostic> {
        use super::macro_names::PathLookup;
        if call.key() != &self.source_key
            || scope.crate_key != self.root.crate_key
            || scope.owner.is_some()
        {
            return Err(error(
                "declaration invocation source or module scope differs",
            ));
        }
        let mut module = self.root.module_path.clone();
        module
            .len()
            .checked_add(call.module_suffix().len())            .ok_or_else(|| error("declaration module depth limit"))?;
        module.extend_from_slice(call.module_suffix());
        if module != scope.module_path {
            return Err(error("declaration invocation module differs"));
        }
        let Some(crate::syntax::imports::ImportSegment::Name(name)) = call.segments().last() else {
            return Err(error("declaration macro member missing"));
        };
        if !call.absolute() && call.segments().len() == 1 {
            match lookup(&scope.module_path, u64::from(call.range().start), name)? {
                HygienicLookup::NeedInput(input) => return Ok(PathLookup::NeedInput(input)),
                HygienicLookup::Selection(super::macro_origins::MacroSelection::Defined(
                    definition,
                )) => return Ok(PathLookup::Defined(definition)),
                HygienicLookup::Selection(super::macro_origins::MacroSelection::Ambiguous(
                    definitions,
                )) => return Ok(PathLookup::Ambiguous(definitions)),
                HygienicLookup::Selection(super::macro_origins::MacroSelection::Absent) => {}
            }
        }
        namespaces.lookup_invocation_path(call, scope)
    }
    pub(crate) fn lookup_invocation_using(
        &self,
        call: &crate::syntax::macro_calls::SourceMacroCall,
        namespaces: &super::macro_names::MacroNamespaces,
        scope: &SourceScope,
        lookup: impl FnOnce(&[String], u64, &str) -> Result<HygienicLookup, Diagnostic>,
    ) -> Result<super::macro_names::PathLookup, Diagnostic> {
        use super::macro_names::PathLookup;
        if call.key() != &self.source_key
            || scope.crate_key != self.root.crate_key
            || scope.owner.is_some()
        {
            return Err(error("source invocation identity or scope mismatch"));
        }
        if call.requires_body_entry() {
            return Err(error(
                "source invocation requires an entered executable body",
            ));
        }
        let mut module = self.root.module_path.clone();
        module
            .len()
            .checked_add(call.module_suffix().len())            .ok_or_else(|| error("source invocation module depth exceeded"))?;
        module.extend_from_slice(call.module_suffix());
        if module != scope.module_path {
            return Err(error("source invocation enclosing module mismatch"));
        }
        let Some(attribute_input) =
            self.item_invocations
                .get(&(scope.clone(), call.range().start, call.range().end))
        else {
            return Ok(PathLookup::Absent);
        };
        let Some(crate::syntax::imports::ImportSegment::Name(name)) = call.segments().last() else {
            return Err(error("source invocation member missing"));
        };
        if let Some(input) = attribute_input {
            return Ok(PathLookup::NeedInput(*input));
        }
        // Qualified paths resolve in the path namespace, independently of textual names.
        if !call.absolute() && call.segments().len() == 1 {
            match lookup(&scope.module_path, u64::from(call.range().start), name)? {
                HygienicLookup::NeedInput(input) => return Ok(PathLookup::NeedInput(input)),
                HygienicLookup::Selection(super::macro_origins::MacroSelection::Defined(def)) => {
                    return Ok(PathLookup::Defined(def));
                }
                HygienicLookup::Selection(super::macro_origins::MacroSelection::Ambiguous(
                    defs,
                )) => return Ok(PathLookup::Ambiguous(defs)),
                _ => {}
            }
        }
        namespaces.lookup_invocation_path(call, scope)
    }
    pub fn lookup(
        &self,
        module_path: &[String],
        before: u64,
        name: &str,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.lookup_with_external(module_path, before, name, &[], &self.facts, &self.ancestry)
    }
    pub(crate) fn owns_definition(&self, identity: Digest) -> bool {
        self.definitions.contains_key(&identity) && !self.imported_definitions.contains(&identity)
    }
    pub(crate) fn completed_root_bindings(
        &self,
    ) -> Result<Vec<(MacroName, MacroDefinition)>, Diagnostic> {
        if !self.producers.is_empty() {
            return Err(error("module macro producers remain pending"));
        }
        let root = *self
            .scope_keys
            .get(&self.root)
            .ok_or_else(|| error("module macro root absent"))?;
        match self.scopes.local_bindings(root, u64::MAX)? {
            crate::expand::scopes::LocalBindings::Ready(bindings) => Ok(bindings),
            crate::expand::scopes::LocalBindings::NeedInput(_) => {
                Err(error("module root bindings remain pending"))
            }
        }
    }
    pub(crate) fn declarations(&self) -> &MacroDeclarations {
        &self.declarations
    }
    pub(crate) fn pending_producers(&self) -> impl Iterator<Item = Digest> + '_ {
        self.producers.keys().copied()
    }
    /// A captured `mod name;` child whose textual completion is still pending.
    pub(crate) fn pending_captured_module(&self, input: Digest) -> bool {
        self.producers.get(&input) == Some(&ProducerKind::ModuleSource)
    }
    pub(crate) fn pending_macro_use(&self, input: Digest) -> bool {
        self.macro_use_inputs.contains(&input)
            && self.producers.get(&input) == Some(&ProducerKind::ModuleSource)
    }
    pub(crate) fn complete_captured_module(
        &mut self,
        input: Digest,
        child: &Self,
    ) -> Result<bool, Diagnostic> {
        self.stamp = next_stamp();
        if self.producers.get(&input) != Some(&ProducerKind::ModuleSource) {
            return Ok(false);
        }
        if self.module_inputs.get(&input) != Some(&child.root)
            || self.source_key.profile != child.source_key.profile
            || self.cfg != child.cfg
        {
            return Err(error("ordinary captured module scope/profile/cfg differs"));
        }
        if child.pending_producers().next().is_some() {
            return Ok(false);
        }
        if self.pending_macro_use(input) {
            return self.complete_captured_macro_use(input, child);
        }
        let scope = *self
            .scope_keys
            .get(&child.root)
            .ok_or_else(|| error("ordinary module placeholder scope absent"))?;
        std::sync::Arc::make_mut(&mut self.scopes).publish(scope, 0, input, Vec::new())?;
        self.producers.remove(&input);
        Ok(true)
    }
    pub(crate) fn complete_captured_macro_use(
        &mut self,
        input: Digest,
        child: &Self,
    ) -> Result<bool, Diagnostic> {
        self.stamp = next_stamp();
        if !self.pending_macro_use(input) {
            return Ok(false);
        }
        let scope = self
            .module_inputs
            .get(&input)
            .ok_or_else(|| error("macro-use module input absent"))?;
        if scope != &child.root
            || self.source_key.profile != child.source_key.profile
            || self.cfg != child.cfg
        {
            return Err(error("macro-use captured module scope/profile/cfg differs"));
        }
        let root = *child
            .scope_keys
            .get(&child.root)
            .ok_or_else(|| error("macro-use captured root absent"))?;
        let crate::expand::scopes::LocalBindings::Ready(bindings) =
            child.scopes.local_bindings(root, u64::MAX)?
        else {
            return Ok(false);
        };
        let bytes = bindings
            .iter()
            .try_fold(self.imported_bytes, |bytes, (name, _)| {
                bytes
                    .checked_add(name.as_str().len())
                    .and_then(|n| n.checked_add(512))
            })
            .ok_or_else(|| error("captured macro-use binding metadata limit"))?;
        let identities = bindings
            .iter()
            .map(|(_, definition)| definition.identity)
            .collect::<BTreeSet<_>>();
        self.definitions
            .len()
            .checked_add(identities.len())
            .ok_or_else(|| error("captured macro-use definition count limit"))?;
        let target = *self
            .scope_keys
            .get(scope)
            .ok_or_else(|| error("macro-use placeholder scope absent"))?;
        std::sync::Arc::make_mut(&mut self.facts).extend(&child.facts)?;
        std::sync::Arc::make_mut(&mut self.ancestry).extend(&child.ancestry)?;
        std::sync::Arc::make_mut(&mut self.declarations).copy_selected(
            &child.declarations,
            &identities,
            &self.facts,
            &self.ancestry,
        )?;
        for identity in identities {
            let definition = child
                .definitions
                .get(&identity)
                .ok_or_else(|| error("macro-use captured definition absent"))?;
            if let Some(old) = self.definitions.get(&identity) {
                if old != definition {
                    return Err(error("macro-use definition identity conflicts"));
                }
            } else {
                self.definitions.insert(identity, definition.clone());
                self.imported_definitions.insert(identity);
            }
        }
        std::sync::Arc::make_mut(&mut self.scopes).publish(target, 0, input, bindings)?;
        self.producers.remove(&input);
        self.imported_bytes = bytes;
        Ok(true)
    }
    pub(crate) fn argument_snapshot(
        &self,
        scope: &SourceScope,
        at: u64,
    ) -> Result<Self, Diagnostic> {
        let mut snapshot = self.snapshot_clone()?;
        let key = *snapshot
            .scope_keys
            .get(scope)
            .ok_or_else(|| error("argument source scope absent"))?;
        std::sync::Arc::make_mut(&mut snapshot.scopes).omit_invocation_position(key, at)?;
        Ok(snapshot)
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .scopes
            .metadata_bytes()?
            .checked_add(self.declarations.metadata_bytes()?)
            .and_then(|n| n.checked_add(self.root.heap_bytes()))
            .and_then(|n| n.checked_add(self.source_key.profile.semantics.capacity()))
            .ok_or_else(|| error("source macro snapshot metadata overflow"))?;
        for scope in self.scope_keys.keys().chain(self.module_inputs.values()) {
            bytes = bytes
                .checked_add(scope.heap_bytes())
                .and_then(|n| n.checked_add(160))
                .ok_or_else(|| error("source macro snapshot metadata overflow"))?;
        }
        for ((scope, _, _), ranges) in &self.item_attributes {
            bytes = ranges
                .capacity()
                .checked_mul(std::mem::size_of::<crate::syntax::spans::SourceRange>())
                .and_then(|n| n.checked_add(scope.heap_bytes()))
                .and_then(|n| n.checked_add(160))
                .and_then(|n| bytes.checked_add(n))
                .ok_or_else(|| error("source macro snapshot attribute metadata overflow"))?;
        }
        for (scope, _, _) in self
            .item_invocations
            .keys()
            .chain(self.attribute_candidates.keys())
        {
            bytes = bytes
                .checked_add(scope.heap_bytes())
                .and_then(|n| n.checked_add(160))
                .ok_or_else(|| error("source macro snapshot invocation metadata overflow"))?;
        }
        for count in [
            self.definitions.len(),
            self.producers.len(),
            self.macro_use_inputs.len(),
            self.imported_definitions.len(),
            self.restored_original.len(),
        ] {
            bytes = count
                .checked_mul(256)
                .and_then(|n| bytes.checked_add(n))
                .ok_or_else(|| error("source macro snapshot index metadata overflow"))?;
        }
        let facts_bytes = self.facts.metadata_bytes()?;
        bytes = bytes
            .checked_add(self.ancestry.metadata_bytes()?)
            .and_then(|n| n.checked_add(facts_bytes))
            .ok_or_else(|| error("source macro snapshot metadata limit"))?;
        Ok(bytes)
    }
    pub(crate) fn immutable_roots(
        &self,
    ) -> (
        &Arc<TextualScopes>,
        &Arc<MacroDeclarations>,
        &Arc<HygieneFacts>,
        &Arc<ExpansionAncestry>,
    ) {
        (
            &self.scopes,
            &self.declarations,
            &self.facts,
            &self.ancestry,
        )
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.metadata_bytes()?
            .checked_sub(self.scopes.metadata_bytes()?)
            .and_then(|n| n.checked_sub(self.declarations.metadata_bytes().ok()?))
            .and_then(|n| n.checked_sub(self.facts.metadata_bytes().ok()?))
            .and_then(|n| n.checked_sub(self.ancestry.metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(crate::runtime::arena::resource)
    }
    pub(crate) fn snapshot_clone(&self) -> Result<Self, Diagnostic> {
        self.metadata_bytes()?;
        Ok(self.clone())
    }
    pub(crate) fn module_input_for(&self, scope: &SourceScope) -> Option<Digest> {
        self.module_inputs
            .iter()
            .find_map(|(id, actual)| (actual == scope).then_some(*id))
    }
    pub(crate) fn module_outer(
        &self,
        input: Digest,
    ) -> Result<super::macro_scopes::OuterMacroScope<'_>, Diagnostic> {
        let child = self
            .module_inputs
            .get(&input)
            .ok_or_else(|| error("captured external module input absent"))?;
        let key = *self
            .scope_keys
            .get(child)
            .ok_or_else(|| error("captured external module scope absent"))?;
        let (scope, before) = self
            .scopes
            .parent(key)?
            .ok_or_else(|| error("captured external module parent absent"))?;
        Ok(super::macro_scopes::OuterMacroScope {
            scopes: &self.scopes,
            scope,
            before,
            declarations: &self.declarations,
        })
    }
    pub(crate) fn lookup_with_external(
        &self,
        module_path: &[String],
        before: u64,
        name: &str,
        outers: &[super::macro_scopes::OuterMacroScope<'_>],
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.lookup_with_external_using(module_path, before, name, outers, facts, ancestry, false)
    }
    pub(crate) fn lookup_with_external_for_discovery(
        &self,
        module_path: &[String],
        before: u64,
        name: &str,
        outers: &[super::macro_scopes::OuterMacroScope<'_>],
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.lookup_with_external_using(module_path, before, name, outers, facts, ancestry, true)
    }
    #[expect(clippy::too_many_arguments, reason = "bounded captured lookup context")]
    fn lookup_with_external_using(
        &self,
        module_path: &[String],
        before: u64,
        name: &str,
        outers: &[super::macro_scopes::OuterMacroScope<'_>],
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
        discovery: bool,
    ) -> Result<HygienicLookup, Diagnostic> {
        let module_path = crate::syntax::ast_adapter::semantic_module_path(module_path)
            .ok_or_else(|| error("invalid or oversized source macro module path"))?;
        let scope = SourceScope {
            crate_key: self.root.crate_key,
            module_path,
            owner: None,
        };
        let scope = *self
            .scope_keys
            .get(&scope)
            .ok_or_else(|| error("source textual module is missing"))?;
        let name = MacroName::parse(name, edition(self.facts.context(self.context)?.edition))?;
        let reference = MacroReference {
            scope,
            before,
            name: &name,
            context: self.context,
            origin: None,
        };
        if discovery {
            self.declarations.resolve_for_discovery(
                &self.scopes,
                reference,
                outers,
                facts,
                ancestry,
            )
        } else {
            self.declarations
                .resolve_with_outers(&self.scopes, reference, outers, facts, ancestry)
        }
    }
}

/// Source-index entry point backed by an exact immutable publication lease.
pub struct PublishedMacroIndex {
    pub(super) view: crate::source::changes::PreparedSourceView,
    index: Arc<SourceMacroIndex>,
    runtime_outers: Arc<Vec<OwnedRuntimeOuter>>,
    runtime_bytes: usize,
}
pub(crate) struct OwnedRuntimeOuter {
    scopes: Arc<TextualScopes>,
    declarations: Arc<MacroDeclarations>,
    scope: ScopeKey,
    before: u64,
}
impl OwnedRuntimeOuter {
    pub(crate) fn scopes(&self) -> &Arc<TextualScopes> {
        &self.scopes
    }
    pub(crate) fn declarations(&self) -> &Arc<MacroDeclarations> {
        &self.declarations
    }
    fn borrowed(&self) -> super::macro_scopes::OuterMacroScope<'_> {
        super::macro_scopes::OuterMacroScope {
            scopes: &self.scopes,
            declarations: &self.declarations,
            scope: self.scope,
            before: self.before,
        }
    }
}
impl Clone for PublishedMacroIndex {
    fn clone(&self) -> Self {
        Self {
            view: self.view.share(),
            index: Arc::clone(&self.index),
            runtime_outers: Arc::clone(&self.runtime_outers),
            runtime_bytes: self.runtime_bytes,
        }
    }
}
impl PublishedMacroIndex {
    pub(crate) fn source_index_root(&self) -> &Arc<SourceMacroIndex> {
        &self.index
    }
    pub(crate) fn runtime_outer_root(&self) -> &Arc<Vec<OwnedRuntimeOuter>> {
        &self.runtime_outers
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(
                self.view
                    .selection()
                    .metadata_bytes()?
                    .checked_mul(2)
                    .ok_or_else(crate::runtime::arena::resource)?,
            )
            .ok_or_else(crate::runtime::arena::resource)
    }
    /// Coordinator-only cold encoding of this exact retained parent and outers.
    /// Source membership alone never substitutes for these actual roots.
    pub(crate) fn prepare_semantic_identity(&self) -> Result<Digest, Diagnostic> {
        let mut e = crate::core::digest::StableEncoder::new("lrr/v1/published-macro-environment");
        e.string(self.source().path.as_str());
        e.record(&self.source().outline);
        e.digest(self.index.prepare_semantic_identity()?);
        e.u64(self.runtime_outers.len() as u64);
        for outer in self.runtime_outers.iter() {
            e.record(&outer.scope);
            e.u64(outer.before);
            e.digest(outer.scopes.semantic_fingerprint()?);
            e.digest(outer.declarations.semantic_fingerprint()?);
        }
        Ok(e.finish())
    }

    pub(crate) fn from_completed_index(
        view: crate::source::changes::PreparedSourceView,
        index: SourceMacroIndex,
    ) -> Result<Self, Diagnostic> {
        if index.source_key != view.selection().outline.parse
            || index.cfg != view.selection().outline.cfg
            || index.root != view.outline().root_scope
        {
            return Err(error("completed macro index captured context differs"));
        }
        Ok(Self {
            view,
            index: Arc::new(index),
            runtime_outers: Arc::new(Vec::new()),
            runtime_bytes: 0,
        })
    }
    pub fn from_view(
        view: crate::source::changes::PreparedSourceView,
        context: Key,
        facts: HygieneFacts,
    ) -> Result<Self, Diagnostic> {
        let index = SourceMacroIndex::from_outline(view.outline(), context, facts)?;
        Ok(Self {
            view,
            index: Arc::new(index),
            runtime_outers: Arc::new(Vec::new()),
            runtime_bytes: 0,
        })
    }
    pub(crate) fn runtime_snapshot(
        view: crate::source::changes::PreparedSourceView,
        index: &SourceMacroIndex,
        facts: &HygieneFacts,
        ancestry: &ExpansionAncestry,
        outers: &[super::macro_scopes::OuterMacroScope<'_>],
        budget: usize,
    ) -> Result<Self, Diagnostic> {
        let mut bytes = index
            .metadata_bytes()?
            .checked_add(facts.metadata_bytes()?)
            .and_then(|n| n.checked_add(ancestry.metadata_bytes().ok()?))
            .and_then(|n| {
                n.checked_add(
                    outers
                        .len()
                        .checked_mul(std::mem::size_of::<OwnedRuntimeOuter>())?,
                )
            })
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .filter(|n| *n <= budget)
            .ok_or_else(|| error("runtime source index metadata budget"))?;
        for outer in outers {
            bytes = bytes
                .checked_add(outer.scopes.metadata_bytes()?)
                .and_then(|n| n.checked_add(outer.declarations.metadata_bytes().ok()?))
                .filter(|n| *n <= budget)
                .ok_or_else(|| error("runtime source outer metadata budget"))?;
        }
        let mut snapshot = index.snapshot_clone()?;
        snapshot.facts = std::sync::Arc::new(facts.clone());
        snapshot.ancestry = std::sync::Arc::new(ancestry.clone());
        let mut result = Self::from_completed_index(view, snapshot)?;
        Arc::get_mut(&mut result.runtime_outers)
            .expect("fresh runtime outer vector")
            .try_reserve_exact(outers.len())
            .map_err(|_| error("runtime source outer allocation"))?;
        Arc::get_mut(&mut result.runtime_outers)
            .expect("fresh runtime outer vector")
            .extend(outers.iter().map(|outer| OwnedRuntimeOuter {
                scopes: Arc::new(outer.scopes.clone()),
                declarations: Arc::new(outer.declarations.clone()),
                scope: outer.scope,
                before: outer.before,
            }));
        result.runtime_bytes = bytes;
        Ok(result)
    }
    pub(crate) fn runtime_bytes(&self) -> usize {
        self.runtime_bytes
    }
    pub(crate) fn require_publication(
        &self,
        source: &crate::source::changes::PreparedSourceView,
    ) -> Result<(), Diagnostic> {
        if self.view.same_publication(source) && self.source() == source.selection() {
            Ok(())
        } else {
            Err(error("runtime macro context source publication differs"))
        }
    }
    pub fn source(&self) -> &crate::source::changes::SourceSelection {
        self.view.selection()
    }
    pub fn producer_kind(&self, key: Digest) -> Option<ProducerKind> {
        self.index.producer_kind(key)
    }
    pub fn lookup(
        &self,
        expected: &crate::source::changes::SourceSelection,
        module_path: &[String],
        before: u64,
        name: &str,
    ) -> Result<HygienicLookup, Diagnostic> {
        if expected.canonical()? != *self.view.selection() {
            return Err(error(
                "macro lookup source context/revision differs from retained publication",
            ));
        }
        self.index.lookup(module_path, before, name)
    }
}

/// Borrowed entered-body index keeps its exact published parent/source alive.
pub struct EnteredMacroIndex<'a> {
    pub(super) module_scope: SourceScope,
    pub(super) parent: Cow<'a, PublishedMacroIndex>,
    pub(super) body: crate::source::manifest::BodyKey,
    pub(super) facts: std::sync::Arc<crate::syntax::entered_macros::EnteredMacroFacts>,
    pub(super) body_context:
        Option<std::sync::Arc<super::source_body_context::PreparedBodyContext>>,
    pub(super) namespace: Option<Arc<super::body_namespace::Namespace>>,
    scopes: Arc<TextualScopes>,
    pub(super) keys: Arc<BTreeMap<u32, ScopeKey>>,
    declarations: Arc<MacroDeclarations>,
    ancestry: Arc<ExpansionAncestry>,
    producers: Arc<BTreeMap<Digest, ProducerKind>>,
    retired_no_item: Arc<BTreeSet<Digest>>,
    module_outer: Option<(ScopeKey, u64)>,
    body_outers: Vec<super::macro_boundary::RetainedMacroScope>,
    ancestor_producers: Vec<Arc<BTreeMap<Digest, ProducerKind>>>,
}
impl PublishedMacroIndex {
    pub fn enter_body(
        &self,
        expected: &crate::source::changes::SourceSelection,
        body: &crate::source::manifest::BodyKey,
    ) -> Result<EnteredMacroIndex<'_>, Diagnostic> {
        if expected.canonical()? != *self.source() {
            return Err(error("entered body source revision mismatch"));
        }
        let facts = self.view.entered_macro_facts(body)?;
        let item = self
            .view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == body.definition)
            .ok_or_else(|| error("entered body declaration is missing"))?;
        let outer = *self
            .index
            .scope_keys
            .get(&item.scope)
            .ok_or_else(|| error("entered body parent scope is missing"))?;
        let before = u64::from(
            facts
                .scopes
                .first()
                .ok_or_else(|| error("entered body root scope is missing"))?
                .range
                .start,
        );
        self.build_entered(facts, item.scope.clone(), outer, before, body)
    }
    pub(super) fn build_entered(
        &self,
        facts: crate::syntax::entered_macros::EnteredMacroFacts,
        module_scope: SourceScope,
        outer: ScopeKey,
        before: u64,
        body: &crate::source::manifest::BodyKey,
    ) -> Result<EnteredMacroIndex<'_>, Diagnostic> {
        self.build_entered_with_context(
            std::sync::Arc::new(facts),
            module_scope,
            Some((outer, before)),
            body,
            None,
        )
    }
    fn build_entered_with_context(
        &self,
        facts: std::sync::Arc<crate::syntax::entered_macros::EnteredMacroFacts>,
        module_scope: SourceScope,
        module_outer: Option<(ScopeKey, u64)>,
        body: &crate::source::manifest::BodyKey,
        body_context: Option<std::sync::Arc<super::source_body_context::PreparedBodyContext>>,
    ) -> Result<EnteredMacroIndex<'_>, Diagnostic> {
        let hygiene = body_context
            .as_ref()
            .and_then(|context| context.macro_hygiene())
            .unwrap_or(&self.index.facts);
        hygiene.context(self.index.context)?;
        let ancestry = match &body_context {
            Some(context) => context.prepare_macro_ancestry(&self.index.ancestry)?,
            None => Arc::clone(&self.index.ancestry),
        };
        let mut e = crate::core::digest::StableEncoder::new("lrr/v2/entered-macro-root");
        e.digest(body.definition);
        let root = ScopeKey::root(e.finish());
        let mut scopes = TextualScopes::default();
        scopes.add_root(root)?;
        let mut keys = BTreeMap::from([(0, root)]);
        for scope in &facts.scopes[1..] {
            let parent = *keys
                .get(
                    &scope
                        .parent
                        .ok_or_else(|| error("body child lacks parent"))?,
                )
                .ok_or_else(|| error("body scope parent is missing"))?;
            keys.insert(
                scope.ordinal,
                scopes.add_child(parent, u64::from(scope.range.start), scope.ordinal)?,
            );
        }
        let mut declarations = MacroDeclarations::default();
        let mut producers = BTreeMap::new();
        for event in &facts.events {
            let key = *keys
                .get(&event.scope)
                .ok_or_else(|| error("body event scope is missing"))?;
            match &event.kind {
                crate::syntax::entered_macros::BodyMacroEventKind::Definition {
                    identity,
                    name,
                    ..
                } => {
                    let (definition_context, definition_edition, definition_origin) =
                        if let Some(context) = &body_context {
                            let parsed = context.source().parsed();
                            let shape = parsed.macro_definition_shape_at(event.range)?;
                            let name_range = shape.name_range();
                            if shape.name() != name.as_str() {
                                return Err(error("prepared macro definition name differs"));
                            }
                            let raw = context
                                .source()
                                .identifier_context_or_original(name_range, self.index.context)?;
                            (
                                raw,
                                parsed.token_edition(name_range)?,
                                context.reference_origin(name_range)?,
                            )
                        } else {
                            (
                                self.index.context,
                                self.source().outline.parse.profile.edition,
                                None,
                            )
                        };
                    let definition = MacroDefinition {
                        identity: *identity,
                        defining_unit: module_scope.crate_key,
                        definition_edition,
                    };
                    declarations.register(
                        definition.clone(),
                        definition_context,
                        definition_origin,
                        hygiene,
                        &ancestry,
                    )?;
                    scopes.define(
                        key,
                        event.at,
                        MacroName::parse(name, definition.definition_edition)?,
                        definition,
                    )?;
                }
                crate::syntax::entered_macros::BodyMacroEventKind::Producer { identity, kind } => {
                    scopes.block(key, event.at, *identity)?;
                    producers.insert(*identity, *kind);
                }
            }
        }
        Ok(EnteredMacroIndex {
            module_scope,
            parent: Cow::Borrowed(self),
            body: body.clone(),
            facts,
            body_context,
            namespace: None,
            scopes: Arc::new(scopes),
            keys: Arc::new(keys),
            declarations: Arc::new(declarations),
            ancestry,
            producers: Arc::new(producers),
            retired_no_item: Arc::new(BTreeSet::new()),
            module_outer,
            body_outers: vec![],
            ancestor_producers: vec![],
        })
    }
}
impl EnteredMacroIndex<'_> {
    pub(super) fn local_hygiene(&self) -> (Key, std::sync::Arc<HygieneFacts>) {
        let facts = self
            .body_context
            .as_ref()
            .and_then(|context| context.macro_hygiene())
            .unwrap_or(&self.parent.index.facts);
        (self.parent.index.context, std::sync::Arc::clone(facts))
    }
    pub(super) fn identifier_context(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<Key, Diagnostic> {
        let Some(context) = &self.body_context else {
            return Ok(self.parent.index.context);
        };
        context
            .source()
            .identifier_context_or_original(range, self.parent.index.context)
    }

    pub(super) fn identifier_edition(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<u16, Diagnostic> {
        match &self.body_context {
            Some(context) => context.source().parsed().token_edition(range),
            None => Ok(self.parent.source().outline.parse.profile.edition),
        }
    }
    pub fn enter_nested_body(
        &self,
        expected: &crate::source::changes::SourceSelection,
        parent_body: &crate::source::manifest::BodyKey,
        child_body: &crate::source::manifest::BodyKey,
    ) -> Result<EnteredMacroIndex<'_>, Diagnostic> {
        if expected.canonical()? != *self.parent.source() || *parent_body != self.body {
            return Err(error("nested body parent source/revision mismatch"));
        }
        let item = self
            .facts
            .items
            .iter()
            .find(|i| {
                i.item
                    .body
                    .as_ref()
                    .is_some_and(|body| body.key() == *child_body)
            })
            .ok_or_else(|| error("nested body is absent or changed"))?;
        if item.item.def_key.item_kind != crate::source::manifest::ItemKind::Function {
            return Err(error("nested macro entry requires a function body"));
        }
        let handle = item
            .item
            .body
            .as_ref()
            .expect("matched retained child body");
        let facts = self
            .parent
            .view
            .retained_body_facts(handle, &item.item.scope)?;
        self.child_from_facts(facts, item, child_body, self.module_scope.clone(), true)
    }
    pub(super) fn child_from_facts(
        &self,
        facts: crate::syntax::entered_macros::EnteredMacroFacts,
        item: &crate::syntax::body_items::BodyItem,
        child_body: &crate::source::manifest::BodyKey,
        module_scope: SourceScope,
        capture_imports: bool,
    ) -> Result<EnteredMacroIndex<'_>, Diagnostic> {
        self.child_from_facts_with_context(
            Arc::new(facts),
            item,
            child_body,
            module_scope,
            capture_imports,
            None,
        )
    }
    pub(super) fn enter_prepared_nested(
        &self,
        context: Arc<super::source_body_context::PreparedBodyContext>,
    ) -> Result<EnteredMacroIndex<'_>, Diagnostic> {
        let parent = context
            .source()
            .local_parent()
            .ok_or_else(|| error("prepared nested body lacks its actual parent source plan"))?;
        let source = context.source();
        if parent.context().source().index().body() != &self.body
            || self.body_context.as_ref().is_none_or(|actual| {
                actual.source().identity() != parent.context().source().identity()
            })
        {
            return Err(error(
                "prepared nested body belongs to another entered parent",
            ));
        }
        let mut rows = self.facts.declarations().filter(|row| {
            row.item.body.as_ref() == Some(source.body())
                && row.item.def_key.fingerprint() == source.body().def_key
        });
        let item = rows
            .next()
            .ok_or_else(|| error("prepared nested declaration is absent"))?;
        if rows.next().is_some()
            || item.attribute_input.is_some()
            || item.item.header != *source.header()
            || item.item.scope != *source.scope()
            || item.item.declaration_span != source.context().range
        {
            return Err(error("prepared nested declaration source facts differ"));
        }
        self.child_from_facts_with_context(
            Arc::clone(context.facts()),
            item,
            &source.body().key(),
            self.module_scope.clone(),
            true,
            Some(Arc::clone(&context)),
        )
    }
    fn child_from_facts_with_context(
        &self,
        facts: Arc<crate::syntax::entered_macros::EnteredMacroFacts>,
        item: &crate::syntax::body_items::BodyItem,
        child_body: &crate::source::manifest::BodyKey,
        module_scope: SourceScope,
        capture_imports: bool,
        context: Option<Arc<super::source_body_context::PreparedBodyContext>>,
    ) -> Result<EnteredMacroIndex<'_>, Diagnostic> {
        if context.is_none() && self.module_outer.is_none() {
            return Err(error(
                "prepared nested body needs its own current owning context",
            ));
        }
        let mut child = self.parent.build_entered_with_context(
            facts,
            module_scope,
            self.module_outer,
            child_body,
            context,
        )?;
        if capture_imports {
            child.namespace = Some(Arc::new(super::body_namespace::Namespace::capture(
                self,
                &child.facts,
                item.scope,
            )?));
        }
        child
            .body_outers
            .try_reserve(self.body_outers.len() + 1)
            .map_err(|_| error("nested body outer scope allocation failed"))?;
        child
            .body_outers
            .push(super::macro_boundary::RetainedMacroScope::new(
                Arc::clone(&self.scopes),
                Arc::clone(&self.declarations),
                self.hygiene_root(),
                self.ancestry_root(),
                self.keys[&item.scope],
                u64::from(item.item.declaration_span.start),
            )?);
        child.body_outers.extend(self.body_outers.iter().cloned());
        child
            .ancestor_producers
            .try_reserve(self.ancestor_producers.len() + 1)
            .map_err(|_| error("nested body producer ancestry allocation failed"))?;
        child.ancestor_producers.push(Arc::clone(&self.producers));
        child
            .ancestor_producers
            .extend(self.ancestor_producers.iter().cloned());
        Ok(child)
    }
    pub(super) fn no_item_retired(&self, identity: Digest) -> bool {
        self.retired_no_item.contains(&identity)
    }
    pub fn facts(&self) -> &crate::syntax::entered_macros::EnteredMacroFacts {
        &self.facts
    }
    pub fn producer_kind(&self, key: Digest) -> Option<ProducerKind> {
        self.producers
            .get(&key)
            .copied()
            .or_else(|| {
                self.ancestor_producers
                    .iter()
                    .find_map(|map| map.get(&key).copied())
            })
            .or_else(|| self.parent.producer_kind(key))
    }
    pub fn lookup_invocation(
        &self,
        expected: &crate::source::changes::SourceSelection,
        body: &crate::source::manifest::BodyKey,
        range: crate::syntax::spans::SourceRange,
        namespaces: &super::macro_names::MacroNamespaces,
    ) -> Result<super::macro_names::PathLookup, Diagnostic> {
        self.lookup_invocation_using(expected, body, range, namespaces, |scope, at, name| {
            match scope {
                Some(row) => self.resolve_import(row, name, &[]),
                None => self.lookup(expected, body, at, name),
            }
        })
    }
    pub fn accept_no_item_proof(
        &mut self,
        expected: &crate::source::changes::SourceSelection,
        body: &crate::source::manifest::BodyKey,
        proof: &super::runtime_macro_plan::RuntimeNoItemProof,
        catalog: &super::runtime_macro_catalog::RuntimeMacroCatalog,
    ) -> Result<(), Diagnostic> {
        if expected.canonical()? != *self.parent.source()
            || *body != self.body
            || proof.body() != body
            || proof.source() != &expected.outline.parse
            || catalog.select(proof.definition())?.is_none()
        {
            return Err(error("no-item proof source/body/compiler identity differs"));
        }
        let event = self.facts.events.iter().find(|event| {
            event.range == proof.invocation()
                && matches!(
                    event.kind,
                    crate::syntax::entered_macros::BodyMacroEventKind::Producer { .. }
                )
        });
        let Some(event) = event else {
            // Expression-position intrinsics have a retained invocation but no
            // declaration producer to publish. The sealed source/body/compiler
            // proof remains checked above on every phase; nothing is retired.
            if self.facts.invocations.iter().any(|invocation| {
                invocation.range == proof.invocation() && invocation.attribute_input.is_none()
            }) {
                return Ok(());
            }
            return Err(error("no-item proof invocation producer absent"));
        };
        let crate::syntax::entered_macros::BodyMacroEventKind::Producer { identity, kind } =
            event.kind
        else {
            return Err(error("no-item proof is not a producer"));
        };
        if kind != ProducerKind::ItemMacro {
            return Err(error("no-item proof cannot retire this producer kind"));
        }
        let scope = *self
            .keys
            .get(&event.scope)
            .ok_or_else(|| error("no-item producer scope absent"))?;
        Arc::make_mut(&mut self.scopes).publish(scope, event.at, identity, vec![])?;
        // Retain only the exact validated empty declaration producer. Ordinary
        // namespace/local indexes must not reconstruct its pending frontier.
        Arc::make_mut(&mut self.retired_no_item).insert(identity);
        Ok(())
    }
    /// Resolve the actual entered invocation before capturing intrinsic arguments.
    /// Textual shadowing and unresolved expansion demands remain typed outcomes.
    pub fn prepare_runtime_macro(
        &self,
        expected: &crate::source::changes::SourceSelection,
        body: &crate::source::manifest::BodyKey,
        range: crate::syntax::spans::SourceRange,
        namespaces: &super::macro_names::MacroNamespaces,
        catalog: &super::runtime_macro_catalog::RuntimeMacroCatalog,
    ) -> Result<super::runtime_macro_plan::RuntimeMacroPreparation, Diagnostic> {
        let lookup = self.lookup_invocation(expected, body, range, namespaces)?;
        self.parent
            .view
            .prepare_runtime_macro(range, lookup, catalog)
    }
    fn lookup_invocation_using(
        &self,
        expected: &crate::source::changes::SourceSelection,
        body: &crate::source::manifest::BodyKey,
        range: crate::syntax::spans::SourceRange,
        namespaces: &super::macro_names::MacroNamespaces,
        mut lookup: impl FnMut(
            Option<&crate::syntax::entered_macros::BodyImport>,
            u64,
            &str,
        ) -> Result<HygienicLookup, Diagnostic>,
    ) -> Result<super::macro_names::PathLookup, Diagnostic> {
        use super::macro_names::PathLookup;
        if expected.canonical()? != *self.parent.source() || *body != self.body {
            return Err(error("body invocation source/body revision mismatch"));
        }
        if range.start >= range.end
            || self
                .facts
                .scopes
                .first()
                .is_none_or(|scope| !scope.range.contains(range))
        {
            return Err(error("body invocation range outside entered body"));
        }
        if self
            .facts
            .opaque
            .iter()
            .any(|opaque| opaque.start < range.end && range.start < opaque.end)
        {
            return Err(error(
                "body invocation requires separate entry into opaque syntax",
            ));
        }
        let call = match &self.body_context {
            Some(context) => context.source().parsed().generated_macro_call_at(range)?,
            None => self.parent.view.macro_call_at(range)?,
        };
        let Some(invocation) = self
            .facts
            .invocations
            .iter()
            .find(|call| call.range == range)
        else {
            return Ok(PathLookup::Absent);
        };
        let Some(crate::syntax::imports::ImportSegment::Name(name)) = call.segments().last() else {
            return Err(error("entered invocation member is missing"));
        };
        let textual = match &self.body_context {
            Some(_) => self.lookup_with_context_occurrence(
                expected,
                body,
                u64::from(range.start),
                name,
                self.identifier_context(call.name_range())?,
                self.identifier_edition(call.name_range())?,
                Some(call.name_range()),
            )?,
            None => lookup(None, u64::from(range.start), name)?,
        };
        if let HygienicLookup::NeedInput(input) = textual {
            return Ok(PathLookup::NeedInput(input));
        }
        if let Some(input) = invocation.attribute_input {
            return Ok(PathLookup::NeedInput(input));
        }
        if !call.absolute() && call.segments().len() == 1 {
            match textual {
                HygienicLookup::Selection(super::macro_origins::MacroSelection::Defined(def)) => {
                    return Ok(PathLookup::Defined(def));
                }
                HygienicLookup::Selection(super::macro_origins::MacroSelection::Ambiguous(
                    defs,
                )) => return Ok(PathLookup::Ambiguous(defs)),
                _ => {}
            }
        }
        if let Some(result) = super::body_imports::lookup(
            self.namespace
                .as_ref()
                .map(|n| &n.facts)
                .unwrap_or(&self.facts),
            invocation.scope,
            &call,
            namespaces,
            &self.module_scope,
            |row, name| lookup(Some(row), row.at, name),
        )? {
            return Ok(result);
        }
        namespaces.lookup_invocation_path(&call, &self.module_scope)
    }
    pub fn lookup(
        &self,
        expected: &crate::source::changes::SourceSelection,
        body: &crate::source::manifest::BodyKey,
        at: u64,
        name: &str,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.lookup_with_context(
            expected,
            body,
            at,
            name,
            self.parent.index.context,
            self.parent.source().outline.parse.profile.edition,
        )
    }
    pub(super) fn lookup_with_context(
        &self,
        expected: &crate::source::changes::SourceSelection,
        body: &crate::source::manifest::BodyKey,
        at: u64,
        name: &str,
        context: Key,
        edition: u16,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.lookup_with_context_occurrence(expected, body, at, name, context, edition, None)
    }
    fn lookup_with_context_occurrence(
        &self,
        expected: &crate::source::changes::SourceSelection,
        body: &crate::source::manifest::BodyKey,
        at: u64,
        name: &str,
        context: Key,
        edition: u16,
        occurrence: Option<crate::syntax::spans::SourceRange>,
    ) -> Result<HygienicLookup, Diagnostic> {
        if expected.canonical()? != *self.parent.source() || *body != self.body {
            return Err(error("body lookup source/body revision mismatch"));
        }
        if self
            .facts
            .opaque
            .iter()
            .any(|range| u64::from(range.start) <= at && at < u64::from(range.end))
        {
            return Err(error(
                "macro lookup requires separate entry into opaque nested syntax",
            ));
        }
        let scope = self
            .facts
            .scopes
            .iter()
            .rev()
            .find(|scope| u64::from(scope.range.start) <= at && at < u64::from(scope.range.end))
            .ok_or_else(|| error("body lookup position is outside entered scopes"))?;
        self.resolve_at_with(scope.ordinal, at, name, context, edition, occurrence, &[])
    }
    // Called only after public selection checks, with a retained invocation or import scope.
    fn resolve_import(
        &self,
        row: &crate::syntax::entered_macros::BodyImport,
        name: &str,
        extra: &[super::macro_scopes::OuterMacroScope<'_>],
    ) -> Result<HygienicLookup, Diagnostic> {
        let (raw_context, edition, origin) = match &self.body_context {
            Some(context) => {
                let occurrence = context.macro_import_occurrence(row, name)?;
                (
                    occurrence.context(context.source(), self.parent.index.context)?,
                    occurrence.edition(),
                    context.reference_origin(occurrence.range())?,
                )
            }
            None => (
                self.parent.index.context,
                self.parent.source().outline.parse.profile.edition,
                None,
            ),
        };
        self.resolve_namespace_with(row.scope, row.at, name, raw_context, edition, origin, extra)
    }
    fn resolve_namespace_with(
        &self,
        ordinal: u32,
        at: u64,
        name: &str,
        raw_context: Key,
        edition: u16,
        origin: Option<Key>,
        extra: &[super::macro_scopes::OuterMacroScope<'_>],
    ) -> Result<HygienicLookup, Diagnostic> {
        let name = MacroName::parse(name, edition)?;
        let context = self
            .namespace
            .as_ref()
            .and_then(|n| n.contexts.get(&ordinal));
        let start = context.map_or(0, |(frame, _)| frame + 1);
        let mut outers = self.body_outers[start..]
            .iter()
            .map(super::macro_boundary::RetainedMacroScope::outer)
            .collect::<Vec<_>>();
        self.append_module_outers(&mut outers, extra)?;
        let (scopes, declarations, key) = match context {
            Some((frame, key)) => {
                let captured = self
                    .body_outers
                    .get(*frame)
                    .ok_or_else(|| error("namespace frame missing"))?;
                (
                    captured.scopes().as_ref(),
                    captured.declarations().as_ref(),
                    *key,
                )
            }
            None => (
                self.scopes.as_ref(),
                self.declarations.as_ref(),
                *self
                    .keys
                    .get(&ordinal)
                    .ok_or_else(|| error("namespace scope missing"))?,
            ),
        };
        declarations.resolve_with_outers(
            scopes,
            MacroReference {
                scope: key,
                before: at,
                name: &name,
                context: raw_context,
                origin,
            },
            &outers,
            self.resolution_hygiene(),
            self.resolution_ancestry(),
        )
    }
    fn hygiene_root(&self) -> Arc<HygieneFacts> {
        Arc::clone(
            self.body_context
                .as_ref()
                .and_then(|context| context.macro_hygiene())
                .unwrap_or(&self.parent.index.facts),
        )
    }
    fn ancestry_root(&self) -> Arc<ExpansionAncestry> {
        Arc::clone(&self.ancestry)
    }
    pub(crate) fn source_macro_root(&self) -> &Arc<SourceMacroIndex> {
        &self.parent.index
    }
    /// Exact immutable declaration metadata, never a name or token-mark guess.
    pub(super) fn retained_definition(
        &self,
        identity: Digest,
    ) -> Result<Option<MacroDefinition>, Diagnostic> {
        let mut selected = self.parent.index.definitions.get(&identity).cloned();
        let mut join = |declarations: &MacroDeclarations| -> Result<(), Diagnostic> {
            if let Some(definition) = declarations.definition_by_identity(identity) {
                if selected.as_ref().is_some_and(|old| old != definition) {
                    return Err(error("retained macro definition metadata conflicts"));
                }
                selected = Some(definition.clone());
            }
            Ok(())
        };
        join(&self.declarations)?;
        join(&self.parent.index.declarations)?;
        for outer in self.parent.runtime_outers.iter() {
            join(&outer.declarations)?;
        }
        for outer in self.body_outers.iter().chain(
            self.body_context
                .iter()
                .flat_map(|context| context.macro_outers()),
        ) {
            join(outer.declarations())?;
        }
        Ok(selected)
    }
    pub(crate) fn runtime_outer_root(&self) -> &Arc<Vec<OwnedRuntimeOuter>> {
        &self.parent.runtime_outers
    }
    pub(crate) fn scope_root(&self) -> &Arc<TextualScopes> {
        &self.scopes
    }
    pub(crate) fn declaration_root(&self) -> &Arc<MacroDeclarations> {
        &self.declarations
    }
    pub(crate) fn scope_key_root(&self) -> &Arc<BTreeMap<u32, ScopeKey>> {
        &self.keys
    }
    pub(crate) fn retired_root(&self) -> &Arc<BTreeSet<Digest>> {
        &self.retired_no_item
    }
    pub(crate) fn namespace_root(&self) -> Option<&Arc<super::body_namespace::Namespace>> {
        self.namespace.as_ref()
    }
    pub(crate) fn producer_roots(
        &self,
    ) -> impl Iterator<Item = &Arc<BTreeMap<Digest, ProducerKind>>> {
        std::iter::once(&self.producers).chain(self.ancestor_producers.iter())
    }
    pub(crate) fn retained_scope_roots(
        &self,
    ) -> impl Iterator<Item = &super::macro_boundary::RetainedMacroScope> {
        self.body_outers.iter()
    }
    pub(crate) fn hygiene_root_handle(&self) -> &Arc<HygieneFacts> {
        self.body_context
            .as_ref()
            .and_then(|context| context.macro_hygiene())
            .unwrap_or(&self.parent.index.facts)
    }
    pub(crate) fn ancestry_root_handle(&self) -> &Arc<ExpansionAncestry> {
        &self.ancestry
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.module_scope.heap_bytes())
            .and_then(|n| {
                n.checked_add(
                    self.parent
                        .view
                        .selection()
                        .metadata_bytes()
                        .ok()?
                        .checked_mul(2)?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    self.body_outers
                        .capacity()
                        .checked_mul(std::mem::size_of::<
                            super::macro_boundary::RetainedMacroScope,
                        >())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    self.ancestor_producers
                        .capacity()
                        .checked_mul(std::mem::size_of::<Arc<BTreeMap<Digest, ProducerKind>>>())?,
                )
            })
            .ok_or_else(crate::runtime::arena::resource)
    }
    pub(super) fn same_environment(&self, other: &EnteredMacroIndex<'_>) -> bool {
        fn scope(
            a: &super::macro_boundary::RetainedMacroScope,
            b: &super::macro_boundary::RetainedMacroScope,
        ) -> bool {
            let left = a.outer();
            let right = b.outer();
            left.scope == right.scope
                && left.before == right.before
                && Arc::ptr_eq(a.scopes(), b.scopes())
                && Arc::ptr_eq(a.declarations(), b.declarations())
                && Arc::ptr_eq(a.facts(), b.facts())
                && Arc::ptr_eq(a.ancestry(), b.ancestry())
        }
        self.module_scope == other.module_scope
            && self.body == other.body
            && self.parent.view.same_publication(&other.parent.view)
            && self.parent.source() == other.parent.source()
            && Arc::ptr_eq(&self.parent.index, &other.parent.index)
            && Arc::ptr_eq(&self.parent.runtime_outers, &other.parent.runtime_outers)
            && Arc::ptr_eq(&self.facts, &other.facts)
            && Arc::ptr_eq(&self.scopes, &other.scopes)
            && Arc::ptr_eq(&self.keys, &other.keys)
            && Arc::ptr_eq(&self.declarations, &other.declarations)
            && Arc::ptr_eq(&self.ancestry, &other.ancestry)
            && Arc::ptr_eq(&self.producers, &other.producers)
            && Arc::ptr_eq(&self.retired_no_item, &other.retired_no_item)
            && self.module_outer == other.module_outer
            && match (&self.body_context, &other.body_context) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
            && match (&self.namespace, &other.namespace) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
            && self.body_outers.len() == other.body_outers.len()
            && self
                .body_outers
                .iter()
                .zip(&other.body_outers)
                .all(|(a, b)| scope(a, b))
            && self.ancestor_producers.len() == other.ancestor_producers.len()
            && self
                .ancestor_producers
                .iter()
                .zip(&other.ancestor_producers)
                .all(|(a, b)| Arc::ptr_eq(a, b))
    }
    pub(super) fn into_owned(self) -> EnteredMacroIndex<'static> {
        EnteredMacroIndex {
            module_scope: self.module_scope,
            parent: Cow::Owned(self.parent.into_owned()),
            body: self.body,
            facts: self.facts,
            body_context: self.body_context,
            namespace: self.namespace,
            scopes: self.scopes,
            keys: self.keys,
            declarations: self.declarations,
            ancestry: self.ancestry,
            producers: self.producers,
            retired_no_item: self.retired_no_item,
            module_outer: self.module_outer,
            body_outers: self.body_outers,
            ancestor_producers: self.ancestor_producers,
        }
    }
    fn resolution_hygiene(&self) -> &HygieneFacts {
        self.body_context
            .as_ref()
            .and_then(|context| context.macro_hygiene())
            .map_or(self.parent.index.facts.as_ref(), |facts| facts.as_ref())
    }
    fn resolution_ancestry(&self) -> &ExpansionAncestry {
        &self.ancestry
    }
    fn append_module_outers<'a>(
        &'a self,
        outers: &mut Vec<super::macro_scopes::OuterMacroScope<'a>>,
        extra: &[super::macro_scopes::OuterMacroScope<'a>],
    ) -> Result<(), Diagnostic> {
        let retained = self
            .body_context
            .as_ref()
            .map(|context| context.macro_outers())
            .unwrap_or(&[]);
        let additional = retained
            .len()
            .checked_add(extra.len())
            .and_then(|n| n.checked_add(self.parent.runtime_outers.len()))
            .and_then(|n| n.checked_add(usize::from(self.module_outer.is_some())))
            .ok_or_else(crate::runtime::arena::resource)?;
        outers
            .try_reserve(additional)
            .map_err(|_| crate::runtime::arena::resource())?;
        if let Some((scope, before)) = self.module_outer {
            outers.push(super::macro_scopes::OuterMacroScope {
                scopes: &self.parent.index.scopes,
                declarations: &self.parent.index.declarations,
                scope,
                before,
            });
            outers.extend(
                self.parent
                    .runtime_outers
                    .iter()
                    .map(OwnedRuntimeOuter::borrowed),
            );
        }
        outers.extend(
            retained
                .iter()
                .map(super::macro_boundary::RetainedMacroScope::outer),
        );
        outers.extend_from_slice(extra);
        Ok(())
    }
    fn resolve_at_with(
        &self,
        ordinal: u32,
        at: u64,
        name: &str,
        context: Key,
        edition: u16,
        occurrence: Option<crate::syntax::spans::SourceRange>,
        extra: &[super::macro_scopes::OuterMacroScope<'_>],
    ) -> Result<HygienicLookup, Diagnostic> {
        let scope = self
            .facts
            .scopes
            .iter()
            .find(|scope| {
                scope.ordinal == ordinal
                    && u64::from(scope.range.start) <= at
                    && at < u64::from(scope.range.end)
            })
            .ok_or_else(|| error("retained body declaration scope mismatch"))?;
        let name = MacroName::parse(name, edition)?;
        let mut outers = self
            .body_outers
            .iter()
            .map(super::macro_boundary::RetainedMacroScope::outer)
            .collect::<Vec<_>>();
        self.append_module_outers(&mut outers, extra)?;
        let origin = match &self.body_context {
            Some(context) => match occurrence {
                Some(range) => context.reference_origin(range)?,
                None => context.reference_origin_at(at, name.as_str())?,
            },
            None => None,
        };
        self.declarations.resolve_with_outers(
            &self.scopes,
            MacroReference {
                scope: self.keys[&scope.ordinal],
                before: at,
                name: &name,
                context,
                origin,
            },
            &outers,
            self.resolution_hygiene(),
            self.resolution_ancestry(),
        )
    }
}

pub struct LinkedModuleIndex<'a> {
    child: PublishedMacroIndex,
    outers: Vec<super::macro_scopes::OuterMacroScope<'a>>,
    ancestors: Vec<&'a crate::source::changes::SourceSelection>,
}
impl PublishedMacroIndex {
    pub fn module_input(&self, module_path: &[String]) -> Result<Digest, Diagnostic> {
        let module_path = crate::syntax::ast_adapter::semantic_module_path(module_path)
            .ok_or_else(|| error("invalid external module path"))?;
        self.index
            .module_inputs
            .iter()
            .find(|(_, scope)| scope.module_path == module_path)
            .map(|(key, _)| *key)
            .ok_or_else(|| error("external module input is missing"))
    }
    pub fn link_module(
        &self,
        expected: &crate::source::changes::SourceSelection,
        input: Digest,
        view: crate::source::changes::PreparedSourceView,
        context: Key,
        facts: HygieneFacts,
    ) -> Result<LinkedModuleIndex<'_>, Diagnostic> {
        if expected.canonical()? != *self.source() {
            return Err(error("module parent source revision mismatch"));
        }
        let scope = self
            .index
            .module_inputs
            .get(&input)
            .ok_or_else(|| error("source demand is not an external module"))?;
        if !self.view.same_publication(&view)
            || view.outline().root_scope != *scope
            || view.selection().outline.cfg != self.source().outline.cfg
            || view.selection().outline.parse.profile != self.source().outline.parse.profile
        {
            return Err(error("external module source/context/publication mismatch"));
        }
        let (outer, before) = self
            .index
            .scopes
            .parent(self.index.scope_keys[scope])?
            .ok_or_else(|| error("external module has no containing scope"))?;
        let child = PublishedMacroIndex::from_view(view, context, facts)?;
        if child.index.facts.name_item(self.index.context)?
            != self.index.facts.name_item(self.index.context)?
        {
            return Err(error("external module name facts disagree"));
        }
        Ok(LinkedModuleIndex {
            child,
            outers: vec![super::macro_scopes::OuterMacroScope {
                scopes: &self.index.scopes,
                scope: outer,
                before,
                declarations: &self.index.declarations,
            }],
            ancestors: vec![self.source()],
        })
    }
}
impl LinkedModuleIndex<'_> {
    pub fn source(&self) -> &crate::source::changes::SourceSelection {
        self.child.source()
    }
    pub fn sources(&self) -> impl Iterator<Item = &crate::source::changes::SourceSelection> {
        std::iter::once(self.source()).chain(self.ancestors.iter().copied())
    }
    pub fn module_input(&self, module_path: &[String]) -> Result<Digest, Diagnostic> {
        self.child.module_input(module_path)
    }
    fn check_sources(
        &self,
        expected: &[crate::source::changes::SourceSelection],
    ) -> Result<(), Diagnostic> {
        if expected.len() != self.ancestors.len() + 1 {
            return Err(error("module source chain length mismatch"));
        }
        for (expected, actual) in expected.iter().zip(self.sources()) {
            if expected.canonical()? != *actual {
                return Err(error("module source chain revision mismatch"));
            }
        }
        Ok(())
    }
    pub fn link_module(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        input: Digest,
        view: crate::source::changes::PreparedSourceView,
        context: Key,
        facts: HygieneFacts,
    ) -> Result<LinkedModuleIndex<'_>, Diagnostic> {
        self.check_sources(expected)?;
        let mut next = self
            .child
            .link_module(&expected[0], input, view, context, facts)?;
        next.outers
            .try_reserve(self.outers.len())
            .map_err(|_| error("module scope chain allocation failed"))?;
        next.ancestors
            .try_reserve(self.ancestors.len())
            .map_err(|_| error("module source chain allocation failed"))?;
        next.outers.extend(self.outers.iter().copied());
        next.ancestors.extend(self.ancestors.iter().copied());
        Ok(next)
    }
    pub fn lookup_invocation(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        range: crate::syntax::spans::SourceRange,
        namespaces: &super::macro_names::MacroNamespaces,
    ) -> Result<super::macro_names::PathLookup, Diagnostic> {
        self.check_sources(expected)?;
        let call = self.child.view.macro_call_at(range)?;
        let mut scope = self.child.index.root.clone();
        scope
            .module_path
            .len()
            .checked_add(call.module_suffix().len())            .ok_or_else(|| error("linked invocation module depth exceeded"))?;
        scope
            .module_path
            .try_reserve(call.module_suffix().len())
            .map_err(|_| error("linked invocation module allocation failed"))?;
        scope.module_path.extend_from_slice(call.module_suffix());
        self.child.index.lookup_invocation_using(
            &call,
            namespaces,
            &scope,
            |module, before, name| self.lookup(expected, module, before, name),
        )
    }
    pub fn lookup(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        module_path: &[String],
        before: u64,
        name: &str,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.check_sources(expected)?;
        let module_path = crate::syntax::ast_adapter::semantic_module_path(module_path)
            .ok_or_else(|| error("invalid linked module scope"))?;
        let scope = SourceScope {
            crate_key: self.source().outline.crate_key,
            module_path,
            owner: None,
        };
        let scope = *self
            .child
            .index
            .scope_keys
            .get(&scope)
            .ok_or_else(|| error("linked module scope is missing"))?;
        let name = MacroName::parse(name, self.source().outline.parse.profile.edition)?;
        self.child.index.declarations.resolve_with_outers(
            &self.child.index.scopes,
            MacroReference {
                scope,
                before,
                name: &name,
                context: self.child.index.context,
                origin: None,
            },
            &self.outers,
            &self.child.index.facts,
            &self.child.index.ancestry,
        )
    }
    pub fn enter_body(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        body: &crate::source::manifest::BodyKey,
    ) -> Result<LinkedEnteredMacroIndex<'_>, Diagnostic> {
        self.check_sources(expected)?;
        Ok(LinkedEnteredMacroIndex {
            entered: self.child.enter_body(&expected[0], body)?,
            module: self,
        })
    }
}
pub struct LinkedEnteredMacroIndex<'a> {
    entered: EnteredMacroIndex<'a>,
    module: &'a LinkedModuleIndex<'a>,
}
impl LinkedEnteredMacroIndex<'_> {
    pub fn local_index(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        body: &crate::source::manifest::BodyKey,
        resolutions: &BTreeMap<Digest, super::source_locals::PatternResolution>,
        expansions: &[(Key, MacroDefinition)],
    ) -> Result<LinkedSourceLocalIndex<'_>, Diagnostic> {
        self.local_index_with_publications(expected, body, resolutions, expansions, &[])
    }
    pub fn local_index_with_publications(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        body: &crate::source::manifest::BodyKey,
        resolutions: &BTreeMap<Digest, super::source_locals::PatternResolution>,
        expansions: &[(Key, MacroDefinition)],
        publications: &[super::source_locals::SourceLocalPublication],
    ) -> Result<LinkedSourceLocalIndex<'_>, Diagnostic> {
        self.module.check_sources(expected)?;
        Ok(LinkedSourceLocalIndex {
            module: self.module,
            locals: self.entered.local_index_with_publications(
                &expected[0],
                body,
                resolutions,
                expansions,
                publications,
            )?,
        })
    }

    pub fn enter_container(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        body: &crate::source::manifest::BodyKey,
        key: &crate::syntax::body_items::ContainerKey,
    ) -> Result<LinkedContainerCatalogue<'_>, Diagnostic> {
        self.module.check_sources(expected)?;
        Ok(LinkedContainerCatalogue {
            module: self.module,
            catalogue: self.entered.enter_container(&expected[0], body, key)?,
        })
    }

    pub fn enter_nested_body(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        parent_body: &crate::source::manifest::BodyKey,
        child_body: &crate::source::manifest::BodyKey,
    ) -> Result<LinkedEnteredMacroIndex<'_>, Diagnostic> {
        self.module.check_sources(expected)?;
        Ok(LinkedEnteredMacroIndex {
            module: self.module,
            entered: self
                .entered
                .enter_nested_body(&expected[0], parent_body, child_body)?,
        })
    }
    pub fn facts(&self) -> &crate::syntax::entered_macros::EnteredMacroFacts {
        self.entered.facts()
    }
    pub fn lookup_invocation(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        body: &crate::source::manifest::BodyKey,
        range: crate::syntax::spans::SourceRange,
        namespaces: &super::macro_names::MacroNamespaces,
    ) -> Result<super::macro_names::PathLookup, Diagnostic> {
        self.module.check_sources(expected)?;
        self.entered.lookup_invocation_using(
            &expected[0],
            body,
            range,
            namespaces,
            |scope, at, name| match scope {
                Some(row) => self.entered.resolve_import(row, name, &self.module.outers),
                None => self.lookup(expected, body, at, name),
            },
        )
    }
    pub fn lookup(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        body: &crate::source::manifest::BodyKey,
        at: u64,
        name: &str,
    ) -> Result<HygienicLookup, Diagnostic> {
        self.module.check_sources(expected)?;
        if *body != self.entered.body {
            return Err(error("linked body revision mismatch"));
        }
        if self
            .entered
            .facts
            .opaque
            .iter()
            .any(|range| u64::from(range.start) <= at && at < u64::from(range.end))
        {
            return Err(error("linked lookup requires separate opaque syntax entry"));
        }
        let scope = self
            .entered
            .facts
            .scopes
            .iter()
            .rev()
            .find(|scope| u64::from(scope.range.start) <= at && at < u64::from(scope.range.end))
            .ok_or_else(|| error("linked body position is outside entered scopes"))?;
        self.resolve_at(scope.ordinal, at, name)
    }
    fn resolve_at(&self, ordinal: u32, at: u64, name: &str) -> Result<HygienicLookup, Diagnostic> {
        self.entered.resolve_at_with(
            ordinal,
            at,
            name,
            self.entered.parent.index.context,
            self.module.source().outline.parse.profile.edition,
            None,
            &self.module.outers,
        )
    }
}

/// Explicit container entry keeps the same complete external-module source chain.
pub struct LinkedContainerCatalogue<'a> {
    catalogue: super::container_catalogue::ContainerCatalogue<'a>,
    module: &'a LinkedModuleIndex<'a>,
}
impl LinkedContainerCatalogue<'_> {
    pub fn key(&self) -> &crate::syntax::body_items::ContainerKey {
        self.catalogue.key()
    }
    pub fn facts(&self) -> &crate::syntax::entered_macros::EnteredMacroFacts {
        self.catalogue.facts()
    }
    pub fn enter_container(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        parent: &crate::syntax::body_items::ContainerKey,
        key: &crate::syntax::body_items::ContainerKey,
    ) -> Result<LinkedContainerCatalogue<'_>, Diagnostic> {
        self.module.check_sources(expected)?;
        Ok(LinkedContainerCatalogue {
            module: self.module,
            catalogue: self.catalogue.enter_container(&expected[0], parent, key)?,
        })
    }
    pub fn enter_body(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        parent: &crate::syntax::body_items::ContainerKey,
        body: &crate::source::manifest::BodyKey,
    ) -> Result<LinkedEnteredMacroIndex<'_>, Diagnostic> {
        self.module.check_sources(expected)?;
        Ok(LinkedEnteredMacroIndex {
            module: self.module,
            entered: self.catalogue.enter_body(&expected[0], parent, body)?,
        })
    }
}

/// Exact original local lookup with the complete external-module source lease.
pub struct LinkedSourceLocalIndex<'a> {
    module: &'a LinkedModuleIndex<'a>,
    locals: super::source_locals::SourceLocalIndex<'a>,
}
impl LinkedSourceLocalIndex<'_> {
    pub fn context(&self) -> Key {
        self.locals.context()
    }
    pub fn lookup(
        &self,
        expected: &[crate::source::changes::SourceSelection],
        body: &crate::source::manifest::BodyKey,
        at: u64,
        name: &str,
        context: Key,
    ) -> Result<super::local_scopes::LocalLookup, Diagnostic> {
        self.module.check_sources(expected)?;
        self.locals.lookup(&expected[0], body, at, name, context)
    }
}

impl PublishedMacroIndex {
    /// The original and retained declaration bridge supplies the real module
    /// macro boundary. Generated declarations need that same insertion receipt;
    /// a diagnostic origin or a generated byte offset cannot substitute for it.
    pub(crate) fn enter_prepared_body(
        &self,
        context: std::sync::Arc<super::source_body_context::PreparedBodyContext>,
    ) -> Result<EnteredMacroIndex<'_>, Diagnostic> {
        let source = std::sync::Arc::clone(context.source());
        let owner = context
            .module_source()
            .ok_or_else(|| error("admitted body lacks its module macro-boundary receipt"))?;
        self.require_publication(owner)?;
        if let Some(parent) = source.local_parent() {
            return parent.enter_prepared_local_body(context);
        }
        let module_outer = if context.generated_boundary().is_some() {
            // declaration_outer was checked by the opaque admission attach;
            // retained actual scopes carry the generated insertion boundary.
            None
        } else {
            let original = owner
                .outline()
                .declaration_rows()
                .find(|item| item.def_key.fingerprint() == source.body().def_key)
                .ok_or_else(|| error("admitted body module boundary declaration absent"))?;
            if original.scope != *source.scope() {
                return Err(error("admitted body module boundary scope differs"));
            }
            let original_body = original
                .body
                .as_ref()
                .ok_or_else(|| error("admitted body module boundary has no body"))?;
            let outer = *self
                .index
                .scope_keys
                .get(&original.scope)
                .ok_or_else(|| error("admitted module macro scope absent"))?;
            Some((outer, u64::from(original_body.range.start)))
        };
        context
            .exclusive_metadata_bytes()
            .checked_add(std::mem::size_of::<EnteredMacroIndex>())
            .ok_or_else(|| error("admitted local context metadata limit"))?;
        let facts = std::sync::Arc::clone(context.facts());
        self.build_entered_with_context(
            facts,
            source.scope().clone(),
            module_outer,
            &source.body().key(),
            Some(context),
        )
    }
}
