//! Actual annotation completions joined to this admitted body's declaration rows.
use super::*;
use crate::{
    core::digest::{StableEncode, StableEncoder},
    frontend::impl_index::CompletedOriginalAnnotation,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct EnteredAnnotationNames {
    pub(crate) source_scope: ScopeKey,
    pub(crate) completion: Arc<CompletedOriginalAnnotation>,
    source: Digest,
    parse: crate::syntax::parse::ParseKey,
    body: BodyHandle,
    owner_scope: ScopeKey,
    facts: Arc<crate::syntax::entered_macros::EnteredMacroFacts>,
    row: usize,
}
pub(crate) struct PreparedDeclarationNameCompletions {
    source: Digest,
    identity: Digest,
    entries: BTreeMap<Digest, Arc<EnteredAnnotationNames>>,
    retired: BTreeSet<Digest>,
    locations: BTreeMap<(Digest, u32, u32), Vec<Digest>>,
}
impl PreparedDeclarationNameCompletions {
    pub(crate) fn empty(context: &PreparedBodyContext) -> Arc<Self> {
        Self::finish(context, BTreeMap::new(), BTreeSet::new())
    }
    fn finish(context: &PreparedBodyContext, entries: BTreeMap<Digest, Arc<EnteredAnnotationNames>>,
        retired: BTreeSet<Digest>) -> Arc<Self> {
        let mut identity = StableEncoder::new("lrr/v1/entered-declaration-name-completions");
        identity.digest(context.source().identity());
        let mut locations = BTreeMap::<_, Vec<_>>::new();
        for (definition, entry) in &entries {
            identity.digest(*definition); identity.record(&entry.source_scope);
            identity.digest(entry.completion.name_fingerprint());
            let range = entry.facts.items[entry.row].item.declaration_span;
            locations.entry((entry.source, range.start, range.end)).or_default().push(*definition);
        }
        for id in &retired { identity.digest(*id); }
        Arc::new(Self { source: context.source().identity(), identity: identity.finish(), entries, retired, locations })
    }
    pub(crate) fn prepare(
        context: Arc<PreparedBodyContext>,
        completions: &BTreeMap<Digest, Arc<CompletedOriginalAnnotation>>,
        initial: impl Fn(&CompletedOriginalAnnotation) -> Result<Arc<crate::expand::source_declarative::assembly::InitialLocalDeclaration>, Diagnostic>,
    ) -> Result<Arc<Self>, Diagnostic> {
        let mut entries = BTreeMap::new();
        let mut retired = BTreeSet::new();
        if let Some(parent) = context.source().local_parent() {
            let inherited = parent.declaration_name_completions();
            entries.extend(inherited.entries.iter().map(|(id, entry)| (*id, Arc::clone(entry))));
            retired.extend(inherited.retired.iter().copied());
        }
        let Some(original) = context.source().original_source() else {
            return Ok(Self::finish(&context, entries, retired));
        };
        let retained = match context.source.source_receipt.as_deref() {
            Some(InputValue::RetainedSyntax(unit)) if unit.generated_unit().is_none()
                && unit.generated_associated().is_none() => Some(unit),
            None => None,
            _ => return Ok(Self::finish(&context, entries, retired)),
        };
        let original_owner = original.outline().items.iter().any(|item|
            item.body.as_ref().is_some_and(|body| body.def_key == context.source().body().def_key));
        for (ordinal, row) in context.facts().items.iter().enumerate() {
            let Some(producer) = row.attribute_input else { continue };
            for completion in completions.values() {
                if completion.name_effects().is_none() { continue; }
                if original_owner && completion.original_target(original)?.local_plan()
                    .is_none_or(|plan| plan.body().def_key != context.source().body().def_key) { continue; }
                let matched = if let Some(unit) = retained {
                    if unit.key() != &context.source().parsed().key()
                        || unit.prepared().source() != original.selection() {
                        return Err(fault("entered annotation retained source differs"));
                    }
                    initial(completion)?.matches(unit.prepared(), row)?
                } else if row.item.def_key.fingerprint() == completion.item() {
                    completion.require_local_declaration(context.source().parsed(), context.source().body(),
                        context.source().scope(), row)?;
                    true
                } else { false };
                if !matched { continue; }
                let target = completion.original_target(original)?;
                let id = row.item.def_key.fingerprint();
                if entries.insert(id, Arc::new(EnteredAnnotationNames {
                    source_scope: target.item().scope.clone(), completion: Arc::clone(completion),
                    source: context.source().identity(), parse: context.source().parsed().key(),
                    body: context.source().body().clone(), owner_scope: context.source().scope().clone(),
                    facts: Arc::clone(context.facts()), row: ordinal,
                })).is_some() { return Err(fault("entered annotation declaration completion repeats")); }
                retired.insert(producer);
                retired.extend(completion.retired_names());
            }
        }
        Ok(Self::finish(&context, entries, retired))
    }
    pub(crate) fn require_context(&self, context: &PreparedBodyContext) -> Result<(), Diagnostic> {
        if context.source().identity() != self.source { return Err(fault("annotation completion belongs to another entered source")); }
        Ok(())
    }
    pub(crate) fn identity(&self) -> Digest { self.identity }
    pub(crate) fn require_local_declaration(&self, parsed: &ParsedFile, body: &BodyHandle,
        scope: &ScopeKey, row: &crate::syntax::body_items::BodyItem) -> Result<(), Diagnostic> {
        let entry = self.entries.get(&row.item.def_key.fingerprint())
            .ok_or_else(|| fault("entered annotation completion row absent"))?;
        let expected = entry.facts.items.get(entry.row)
            .ok_or_else(|| fault("entered annotation canonical row absent"))?;
        if entry.parse != parsed.key() || &entry.body != body || &entry.owner_scope != scope
            || expected.item.def_key != row.item.def_key || expected.item.scope != row.item.scope
            || expected.item.declaration_span != row.item.declaration_span || expected.item.header != row.item.header
            || expected.attribute_input != row.attribute_input {
            return Err(fault("entered annotation completion owning declaration differs"));
        }
        Ok(())
    }
    pub(crate) fn retires(&self, producer: Digest) -> bool { self.retired.contains(&producer) }
    pub(crate) fn entry(&self, definition: Digest) -> Option<&EnteredAnnotationNames> {
        self.entries.get(&definition).map(|entry| &**entry)
    }
    pub(crate) fn declared_names(&self) -> Vec<Arc<crate::frontend::generated_headers::declared_names::DeclaredItemName>> {
        self.entries.values().flat_map(|entry| entry.completion.declared_names_at(&entry.source_scope)).collect()
    }
}

/// One actual completed declaration at an indexed ordinary node. This supplies
/// annotation preparation, not a separate declaration evaluator or recipe.
pub(crate) struct EnteredDeclarationAdmission {
    entry: Arc<EnteredAnnotationNames>,
    node: crate::ir::pc::NodeId,
    index: Digest,
}
impl PreparedDeclarationNameCompletions {
    pub(crate) fn admit_node(&self, context: &PreparedBodyContext,
        index: &BodyIndex, key: &crate::ir::pc::PcKey,
    ) -> Result<Option<EnteredDeclarationAdmission>, Diagnostic> {
        self.require_context(context)?;
        if key.body != *index.body() || context.source().index().fingerprint() != index.fingerprint() {
            return Err(fault("declaration admission indexed source differs"));
        }
        let Some(node) = index.nodes().get(key.node as usize) else { return Err(fault("declaration admission node absent")); };
        if node.role != crate::syntax::body_index::NodeRole::OpaqueItem { return Ok(None); }
        let body = context.source().body();
        let range = crate::syntax::spans::SourceRange::checked(
            body.range.start.checked_add(node.range.start).ok_or_else(|| fault("declaration node start overflow"))?,
            body.range.start.checked_add(node.range.end).ok_or_else(|| fault("declaration node end overflow"))?)?;
        match self.locations.get(&(context.source().identity(), range.start, range.end)).map(Vec::as_slice) {
            None => Ok(None),
            Some([id]) => {
                let entry = self.entries.get(id).ok_or_else(|| fault("declaration location index differs"))?;
                Ok(Some(EnteredDeclarationAdmission { entry: Arc::clone(entry), node: node.id, index: index.fingerprint() }))
            }
            Some(_) => Err(fault("declaration location has conflicting completion owners")),
        }
    }
}
impl EnteredDeclarationAdmission {
    pub(crate) fn require(&self, parsed: &ParsedFile, body: &BodyHandle, index: &BodyIndex,
        key: &crate::ir::pc::PcKey) -> Result<(), Diagnostic> {
        if parsed.key() != self.entry.parse || body != &self.entry.body
            || index.body() != &body.key() || index.fingerprint() != self.index
            || key.body != body.key() || key.node != self.node.0 {
            return Err(fault("completed declaration admission source/node differs"));
        }
        Ok(())
    }
    pub(crate) fn require_published(&self, db: &mut Database) -> Result<(), Diagnostic> {
        let row = &self.entry.facts.items[self.entry.row];
        let definition = row.item.def_key.fingerprint();
        if !matches!(db.input(&InputKey::DefinitionHeader(definition)).as_deref(),
            Some(InputValue::DefinitionHeader(header)) if header == &row.item.header)
            || !matches!(db.input(&InputKey::DefinitionSource(definition)).as_deref(),
                Some(InputValue::DefinitionSource(parse)) if parse == &self.entry.parse)
            || !matches!(db.input(&InputKey::DeclarationContext(definition)).as_deref(),
                Some(InputValue::DeclarationContext(context)) if context.scope == row.item.scope
                    && context.range == row.item.declaration_span) {
            return Err(fault("completed declaration canonical metadata is not published"));
        }
        Ok(())
    }
}
