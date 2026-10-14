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
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone)]
pub struct SourceMacroIndex {
    item_attributes: BTreeMap<(SourceScope, u32, u32), Vec<crate::syntax::spans::SourceRange>>,
    item_invocations: BTreeMap<(SourceScope, u32, u32), Option<Digest>>,
    attribute_candidates: BTreeMap<(SourceScope, u32, u32), crate::syntax::spans::SourceRange>,
    source_key: crate::syntax::parse::ParseKey,
    cfg: Digest,
    scopes: TextualScopes,
    scope_keys: BTreeMap<SourceScope, ScopeKey>,
    root: SourceScope,
    declarations: MacroDeclarations,
    definitions: BTreeMap<Digest, MacroDefinition>,
    producers: BTreeMap<Digest, ProducerKind>,
    module_inputs: BTreeMap<Digest, SourceScope>,
    macro_use_inputs: BTreeSet<Digest>,
    imported_definitions: BTreeSet<Digest>,
    imported_bytes: usize,
    facts: std::sync::Arc<HygieneFacts>,
    context: Key,
    ancestry: ExpansionAncestry,
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
        if outline.item_invocations.len() > 100_000 {
            return Err(error("source item invocation count exceeded"));
        }
        for call in &outline.item_invocations {
            invocation_bytes = invocation_bytes
                .checked_add(std::mem::size_of::<crate::syntax::outline::ItemInvocation>())
                .and_then(|n| n.checked_add(call.scope.heap_bytes()))
                .filter(|n| *n <= 32 * 1024 * 1024)
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
            scopes,
            scope_keys: BTreeMap::from([(outline.root_scope.clone(), root)]),
            root: outline.root_scope.clone(),
            declarations: MacroDeclarations::default(),
            definitions: BTreeMap::new(),
            producers: BTreeMap::new(),
            module_inputs: BTreeMap::new(),
            macro_use_inputs: BTreeSet::new(),
            imported_definitions: BTreeSet::new(),
            imported_bytes: 0,
            facts: std::sync::Arc::new(facts),
            context,
            ancestry: ExpansionAncestry::default(),
        };
        if outline.textual_events.len() > 1_000_000 || outline.items.len() > 100_000 {
            return Err(error("source macro event limit exceeded"));
        }
        let mut attribute_bytes = 0usize;
        for item in &outline.items {
            attribute_bytes = item
                .scope
                .heap_bytes()
                .checked_add(128)
                .and_then(|amount| attribute_bytes.checked_add(amount))
                .filter(|n| *n <= 32 * 1024 * 1024)
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
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| error("original attribute candidate metadata limit"))?;
                if index.attribute_candidates.len() >= 100_000
                    || index
                        .attribute_candidates
                        .insert(
                            (item.scope.clone(), range.start, range.end),
                            item.declaration_span,
                        )
                        .is_some()
                {
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
                        return Err(error("duplicate source textual scope"));
                    }
                    // A transforming attribute belongs to the opening item.
                    // Include its frontier in the captured parent visibility.
                    let before = event
                        .at
                        .checked_add(1)
                        .ok_or_else(|| error("scope opening position overflow"))?;
                    let key = index.scopes.add_child(scope, before, *ordinal)?;
                    index.scope_keys.insert(child.clone(), key);
                }
                TextualEventKind::Producer { identity, kind } => {
                    if *kind == ProducerKind::ModuleSource {
                        index.module_inputs.insert(*identity, event.scope.clone());
                    }
                    index.scopes.block(scope, event.at, *identity)?;
                    if index.producers.insert(*identity, *kind).is_some() {
                        return Err(error("duplicate source producer identity"));
                    }
                }
                TextualEventKind::Definition {
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
                    index.declarations.register(
                        definition.clone(),
                        context,
                        None,
                        &index.facts,
                        &index.ancestry,
                    )?;
                    index.scopes.define(
                        scope,
                        event.at,
                        MacroName::parse(source_name, outline.profile.edition)?,
                        definition.clone(),
                    )?;
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
            index
                .scopes
                .macro_use_module(parent, child, u64::from(item.declaration_span.end))?;
        }
        Ok(index)
    }
    pub(crate) fn reserve_test_attribute(
        &mut self,
        call: &crate::syntax::outline::ItemInvocation,
    ) -> Result<(), Diagnostic> {
        if self.producers.contains_key(&call.item_input) {
            return Err(error("test attribute already reserved"));
        }
        if !self.item_attributes.contains_key(&(
            call.scope.clone(),
            call.range.start,
            call.range.end,
        )) || self.producers.len() >= 100_000
        {
            return Err(error("test attribute source attachment or count differs"));
        }
        let scope = *self
            .scope_keys
            .get(&call.scope)
            .ok_or_else(|| error("test attribute source scope absent"))?;
        self.scopes
            .block(scope, u64::from(call.range.start), call.item_input)?;
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
        self.scopes.publish(key, at, identity, vec![])?;
        self.producers.remove(&identity);
        Ok(())
    }
    pub(crate) fn complete_generated_item_producer(
        &mut self,
        scope: &SourceScope,
        at: u64,
        identity: Digest,
        catalog: &super::generated_macros::GeneratedMacroCatalog,
    ) -> Result<(), Diagnostic> {
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
        snapshot.scopes = self.scopes.prefix_view(key, at, producer)?;
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
            .filter(|n| *n <= 1_000_000)
            .ok_or_else(|| error("outward macro definition count limit"))?;
        for entry in catalog.outward() {
            if self.definitions.contains_key(&entry.definition.identity) {
                return Err(error("outward macro identity was already registered"));
            }
        }
        std::sync::Arc::make_mut(&mut self.facts).extend(catalog.facts())?;
        self.ancestry.extend(catalog.ancestry())?;
        let bindings = catalog.root_bindings()?;
        for entry in catalog.outward() {
            self.declarations.register(
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
            self.scopes.publish_prefix(key, at, identity, bindings)?;
        } else {
            self.scopes.publish(key, at, identity, bindings)?;
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
            .checked_add(attribute.module_suffix().len())
            .filter(|n| *n <= 256)
            .ok_or_else(|| error("source attribute module depth exceeded"))?;
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
            .checked_add(call.module_suffix().len())
            .filter(|n| *n <= 256)
            .ok_or_else(|| error("declaration module depth limit"))?;
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
            .checked_add(call.module_suffix().len())
            .filter(|n| *n <= 256)
            .ok_or_else(|| error("source invocation module depth exceeded"))?;
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
    pub(crate) fn pending_macro_use(&self, input: Digest) -> bool {
        self.macro_use_inputs.contains(&input)
            && self.producers.get(&input) == Some(&ProducerKind::ModuleSource)
    }
    pub(crate) fn complete_captured_module(
        &mut self,
        input: Digest,
        child: &Self,
    ) -> Result<bool, Diagnostic> {
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
        self.scopes.publish(scope, 0, input, Vec::new())?;
        self.producers.remove(&input);
        Ok(true)
    }
    pub(crate) fn complete_captured_macro_use(
        &mut self,
        input: Digest,
        child: &Self,
    ) -> Result<bool, Diagnostic> {
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
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("captured macro-use binding metadata limit"))?;
        let identities = bindings
            .iter()
            .map(|(_, definition)| definition.identity)
            .collect::<BTreeSet<_>>();
        self.definitions
            .len()
            .checked_add(identities.len())
            .filter(|n| *n <= 1_000_000)
            .ok_or_else(|| error("captured macro-use definition count limit"))?;
        let target = *self
            .scope_keys
            .get(scope)
            .ok_or_else(|| error("macro-use placeholder scope absent"))?;
        std::sync::Arc::make_mut(&mut self.facts).extend(&child.facts)?;
        self.ancestry.extend(&child.ancestry)?;
        self.declarations.copy_selected(
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
        self.scopes.publish(target, 0, input, bindings)?;
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
        snapshot.scopes.omit_invocation_position(key, at)?;
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
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| error("source macro snapshot metadata limit"))?;
        Ok(bytes)
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
    index: SourceMacroIndex,
    runtime_outers: Vec<OwnedRuntimeOuter>,
    runtime_bytes: usize,
}
struct OwnedRuntimeOuter {
    scopes: TextualScopes,
    declarations: MacroDeclarations,
    scope: ScopeKey,
    before: u64,
}
impl OwnedRuntimeOuter {
    fn borrowed(&self) -> super::macro_scopes::OuterMacroScope<'_> {
        super::macro_scopes::OuterMacroScope {
            scopes: &self.scopes,
            declarations: &self.declarations,
            scope: self.scope,
            before: self.before,
        }
    }
}
impl PublishedMacroIndex {
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
            index,
            runtime_outers: Vec::new(),
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
            index,
            runtime_outers: Vec::new(),
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
        if outers.len() > 256 {
            return Err(error("runtime source outer depth limit"));
        }
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
        snapshot.ancestry = ancestry.clone();
        let mut result = Self::from_completed_index(view, snapshot)?;
        result
            .runtime_outers
            .try_reserve_exact(outers.len())
            .map_err(|_| error("runtime source outer allocation"))?;
        result
            .runtime_outers
            .extend(outers.iter().map(|outer| OwnedRuntimeOuter {
                scopes: outer.scopes.clone(),
                declarations: outer.declarations.clone(),
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
    pub(super) parent: &'a PublishedMacroIndex,
    pub(super) body: crate::source::manifest::BodyKey,
    pub(super) facts: crate::syntax::entered_macros::EnteredMacroFacts,
    pub(super) namespace: Option<super::body_namespace::Namespace>,
    scopes: TextualScopes,
    pub(super) keys: BTreeMap<u32, ScopeKey>,
    declarations: MacroDeclarations,
    producers: BTreeMap<Digest, ProducerKind>,
    retired_no_item: BTreeSet<Digest>,
    outer: ScopeKey,
    before: u64,
    body_outers: Vec<super::macro_scopes::OuterMacroScope<'a>>,
    ancestor_producers: Vec<&'a BTreeMap<Digest, ProducerKind>>,
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
                    let definition = MacroDefinition {
                        identity: *identity,
                        defining_unit: self.source().outline.crate_key,
                        definition_edition: self.source().outline.parse.profile.edition,
                    };
                    declarations.register(
                        definition.clone(),
                        self.index.context,
                        None,
                        &self.index.facts,
                        &self.index.ancestry,
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
            parent: self,
            body: body.clone(),
            facts,
            namespace: None,
            scopes,
            keys,
            declarations,
            producers,
            retired_no_item: BTreeSet::new(),
            outer,
            before,
            body_outers: vec![],
            ancestor_producers: vec![],
        })
    }
}
impl EnteredMacroIndex<'_> {
    pub(super) fn local_hygiene(&self) -> (Key, std::sync::Arc<HygieneFacts>) {
        (
            self.parent.index.context,
            std::sync::Arc::clone(&self.parent.index.facts),
        )
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
        if self.body_outers.len() >= 255 {
            return Err(error("nested declaration scope-chain depth exceeded"));
        }
        let mut child =
            self.parent
                .build_entered(facts, module_scope, self.outer, self.before, child_body)?;
        if capture_imports {
            child.namespace = Some(super::body_namespace::Namespace::capture(
                self,
                &child.facts,
                item.scope,
            )?);
        }
        child
            .body_outers
            .try_reserve(self.body_outers.len() + 1)
            .map_err(|_| error("nested body outer scope allocation failed"))?;
        child
            .body_outers
            .push(super::macro_scopes::OuterMacroScope {
                scopes: &self.scopes,
                scope: self.keys[&item.scope],
                before: u64::from(item.item.declaration_span.start),
                declarations: &self.declarations,
            });
        child.body_outers.extend(self.body_outers.iter().copied());
        child
            .ancestor_producers
            .try_reserve(self.ancestor_producers.len() + 1)
            .map_err(|_| error("nested body producer ancestry allocation failed"))?;
        child.ancestor_producers.push(&self.producers);
        child
            .ancestor_producers
            .extend(self.ancestor_producers.iter().copied());
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
                Some(scope) => self.resolve_namespace_at(scope, at, name, &[]),
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
        self.scopes.publish(scope, event.at, identity, vec![])?;
        // Retain only the exact validated empty declaration producer. Ordinary
        // namespace/local indexes must not reconstruct its pending frontier.
        self.retired_no_item.insert(identity);
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
        mut lookup: impl FnMut(Option<u32>, u64, &str) -> Result<HygienicLookup, Diagnostic>,
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
        let call = self.parent.view.macro_call_at(range)?;
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
        let textual = lookup(None, u64::from(range.start), name)?;
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
            |scope, at, name| lookup(Some(scope), at, name),
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
        self.resolve_at(scope.ordinal, at, name)
    }
    // Called only after public selection checks, with a retained invocation or import scope.
    fn resolve_namespace_at(
        &self,
        ordinal: u32,
        at: u64,
        name: &str,
        extra: &[super::macro_scopes::OuterMacroScope<'_>],
    ) -> Result<HygienicLookup, Diagnostic> {
        let name = MacroName::parse(name, self.parent.source().outline.parse.profile.edition)?;
        let context = self
            .namespace
            .as_ref()
            .and_then(|n| n.contexts.get(&ordinal));
        let start = context.map_or(0, |(frame, _)| frame + 1);
        let mut outers = self.body_outers[start..].to_vec();
        outers
            .try_reserve(extra.len() + 1)
            .map_err(|_| error("namespace outer allocation failed"))?;
        outers.push(super::macro_scopes::OuterMacroScope {
            scopes: &self.parent.index.scopes,
            scope: self.outer,
            before: self.before,
            declarations: &self.parent.index.declarations,
        });
        outers
            .try_reserve(self.parent.runtime_outers.len())
            .map_err(|_| error("runtime namespace outer allocation"))?;
        outers.extend(
            self.parent
                .runtime_outers
                .iter()
                .map(OwnedRuntimeOuter::borrowed),
        );
        outers.extend_from_slice(extra);
        let (scopes, declarations, key) = match context {
            Some((frame, key)) => {
                let captured = self
                    .body_outers
                    .get(*frame)
                    .ok_or_else(|| error("namespace frame missing"))?;
                (captured.scopes, captured.declarations, *key)
            }
            None => (
                &self.scopes,
                &self.declarations,
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
                context: self.parent.index.context,
                origin: None,
            },
            &outers,
            &self.parent.index.facts,
            &self.parent.index.ancestry,
        )
    }
    fn resolve_at(&self, ordinal: u32, at: u64, name: &str) -> Result<HygienicLookup, Diagnostic> {
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
        let name = MacroName::parse(name, self.parent.source().outline.parse.profile.edition)?;
        let mut outers = self.body_outers.clone();
        outers
            .try_reserve(1)
            .map_err(|_| error("body outer scope allocation failed"))?;
        outers.push(super::macro_scopes::OuterMacroScope {
            scopes: &self.parent.index.scopes,
            scope: self.outer,
            before: self.before,
            declarations: &self.parent.index.declarations,
        });
        outers
            .try_reserve(self.parent.runtime_outers.len())
            .map_err(|_| error("runtime body outer allocation"))?;
        outers.extend(
            self.parent
                .runtime_outers
                .iter()
                .map(OwnedRuntimeOuter::borrowed),
        );
        self.declarations.resolve_with_outers(
            &self.scopes,
            MacroReference {
                scope: self.keys[&scope.ordinal],
                before: at,
                name: &name,
                context: self.parent.index.context,
                origin: None,
            },
            &outers,
            &self.parent.index.facts,
            &self.parent.index.ancestry,
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
        if self.ancestors.len() >= 255 {
            return Err(error("external module source depth exceeded"));
        }
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
            .checked_add(call.module_suffix().len())
            .filter(|n| *n <= 256)
            .ok_or_else(|| error("linked invocation module depth exceeded"))?;
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
                Some(scope) => {
                    self.entered
                        .resolve_namespace_at(scope, at, name, &self.module.outers)
                }
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
        let scope = self
            .entered
            .facts
            .scopes
            .iter()
            .find(|scope| {
                scope.ordinal == ordinal
                    && u64::from(scope.range.start) <= at
                    && at < u64::from(scope.range.end)
            })
            .ok_or_else(|| error("retained linked declaration scope mismatch"))?;
        let name = MacroName::parse(name, self.module.source().outline.parse.profile.edition)?;
        let mut outers = Vec::new();
        outers
            .try_reserve(self.module.outers.len() + self.entered.body_outers.len() + 1)
            .map_err(|_| error("linked body scope chain allocation failed"))?;
        outers.extend(self.entered.body_outers.iter().copied());
        outers.push(super::macro_scopes::OuterMacroScope {
            scopes: &self.entered.parent.index.scopes,
            scope: self.entered.outer,
            before: self.entered.before,
            declarations: &self.entered.parent.index.declarations,
        });
        outers.extend(self.module.outers.iter().copied());
        self.entered.declarations.resolve_with_outers(
            &self.entered.scopes,
            MacroReference {
                scope: self.entered.keys[&scope.ordinal],
                before: at,
                name: &name,
                context: self.entered.parent.index.context,
                origin: None,
            },
            &outers,
            &self.entered.parent.index.facts,
            &self.entered.parent.index.ancestry,
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
