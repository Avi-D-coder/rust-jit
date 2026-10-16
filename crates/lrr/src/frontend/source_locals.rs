//! Immutable original local-name index over exact entered source and pinned hygiene.
use super::{
    local_scopes::{LocalLookup, LocalScopes},
    source_macros::EnteredMacroIndex,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::scopes::{MacroDefinition, MacroName, ScopeKey},
    source::{changes::SourceSelection, manifest::BodyKey},
    syntax::{
        entered_macros::BodyMacroEventKind, local_bindings::PatternNameKind, token_wire::Key,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Deref,
    sync::Arc,
};
#[path = "source_local_plan.rs"]
pub(crate) mod owned_plan;
pub(crate) use owned_plan::PreparedSourceLocalPlan;
#[path = "source_local_patterns.rs"]
mod pattern_equivalence;
// Per-call memoization is valid only under this immutable source/namespace borrow.
#[derive(Default)]
struct ImportMemo {
    entries:
        BTreeMap<(u32, crate::source::manifest::Namespace, MacroName), super::names::NameLookup>,
    bytes: usize,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PatternResolution {
    Binding,
    Constant,
    NeedInput(Digest),
}
/// Supplied ordered top-level declarations; discovery and expansion execution
/// belong to the coordinator, never to this source-name index.
#[derive(Debug, Clone)]
pub enum EmittedLocalDeclaration {
    Binding { name: String, context: Key },
    Macro { definition: MacroDefinition },
    Pending { invocation: Digest },
}
#[derive(Debug, Clone)]
pub struct SourceLocalPublication {
    pub source: SourceSelection,
    pub body: BodyKey,
    pub producer: Digest,
    pub expansion: Key,
    pub output: Digest,
    pub declarations: Vec<EmittedLocalDeclaration>,
}
impl SourceLocalPublication {
    pub fn binding_identity(&self, body: &BodyKey, ordinal: usize) -> Result<Digest, Diagnostic> {
        if *body != self.body {
            return Err(error("emitted binding body revision mismatch"));
        }
        if !matches!(
            self.declarations.get(ordinal),
            Some(EmittedLocalDeclaration::Binding { .. })
        ) {
            return Err(error("emitted binding ordinal missing"));
        }
        let mut e = StableEncoder::new("lrr/v2/emitted-local-binding");
        e.digest(body.definition);
        e.digest(self.producer);
        e.digest(Digest(self.expansion.0));
        e.digest(self.output);
        e.u32(u32::try_from(ordinal).map_err(|_| error("emitted binding ordinal overflow"))?);
        Ok(e.finish())
    }
}
enum PrefixResolution {
    Name(super::names::NameLookup),
    Parameter(crate::traits::term::ParamKey),
    ModuleTail,
    NeedImport(Digest),
    NeedDeclaration(Digest),
    NeedOwnerGeneric,
}
pub struct OrdinaryNameSite<'a> {
    pub source: &'a SourceSelection,
    pub body: &'a BodyKey,
    pub at: u64,
    pub name: &'a str,
    pub context: Key,
    pub namespace: crate::source::manifest::Namespace,
}
/// Immutable tables built by the single ordinary source-local preparation.
/// Fields stay private; owning this allocation grants no use-site Type authority.
pub struct SourceLocalTables {
    anonymous_imports: BTreeMap<u32, Vec<usize>>,
    scopes: BTreeMap<u32, ScopeKey>,
    locals: LocalScopes,
    context: Key,
    item_candidates: BTreeMap<
        (u32, crate::source::manifest::Namespace, MacroName),
        Vec<super::names::NameDefinition>,
    >,
    item_frontiers: BTreeMap<u32, BTreeSet<Digest>>,
    named_imports: BTreeMap<(u32, MacroName), Vec<usize>>,
    glob_imports: BTreeMap<u32, Vec<usize>>,
}
impl SourceLocalTables {
    pub(crate) fn hygiene_root(&self) -> &Arc<super::hygiene::HygieneFacts> {
        self.locals.facts_root()
    }
    /// Exclusive table/node/string storage. Shared pinned hygiene is separate.
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        fn map<K, V>(rows: &BTreeMap<K, V>) -> Result<usize, Diagnostic> {
            rows.len()
                .checked_mul(
                    std::mem::size_of::<(K, V)>()
                        .checked_mul(16)
                        .and_then(|n| n.checked_add(256))
                        .ok_or_else(crate::runtime::arena::resource)?,
                )
                .ok_or_else(crate::runtime::arena::resource)
        }
        let mut bytes = std::mem::size_of::<Self>();
        let mut add = |n: usize| -> Result<(), Diagnostic> {
            bytes = bytes
                .checked_add(n)
                .ok_or_else(crate::runtime::arena::resource)?;
            Ok(())
        };
        add(self
            .locals
            .metadata_bytes()?
            .checked_sub(std::mem::size_of::<LocalScopes>())
            .ok_or_else(crate::runtime::arena::resource)?)?;
        add(map(&self.scopes)?)?;
        add(map(&self.anonymous_imports)?)?;
        add(map(&self.glob_imports)?)?;
        add(map(&self.named_imports)?)?;
        add(map(&self.item_candidates)?)?;
        add(map(&self.item_frontiers)?)?;
        for rows in self
            .anonymous_imports
            .values()
            .chain(self.glob_imports.values())
            .chain(self.named_imports.values())
        {
            add(rows
                .capacity()
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or_else(crate::runtime::arena::resource)?)?;
        }
        for ((_, name), _) in &self.named_imports {
            add(name
                .as_str()
                .len()
                .checked_mul(2)
                .ok_or_else(crate::runtime::arena::resource)?)?;
        }
        for ((_, _, name), rows) in &self.item_candidates {
            add(name
                .as_str()
                .len()
                .checked_mul(2)
                .ok_or_else(crate::runtime::arena::resource)?)?;
            add(rows
                .capacity()
                .checked_mul(std::mem::size_of::<super::names::NameDefinition>())
                .ok_or_else(crate::runtime::arena::resource)?)?;
        }
        for rows in self.item_frontiers.values() {
            add(rows
                .len()
                .checked_mul(16 * std::mem::size_of::<Digest>() + 256)
                .ok_or_else(crate::runtime::arena::resource)?)?;
        }
        Ok(bytes)
    }
}
pub struct SourceLocalIndex<'a> {
    entered: &'a EnteredMacroIndex<'a>,
    tables: Arc<SourceLocalTables>,
}
impl Deref for SourceLocalIndex<'_> {
    type Target = SourceLocalTables;
    fn deref(&self) -> &SourceLocalTables {
        &self.tables
    }
}

fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
fn import_metadata(bytes: &mut usize, count: usize, text: usize) -> Result<(), Diagnostic> {
    *bytes = count
        .checked_mul(160)
        .and_then(|n| n.checked_add(text))
        .and_then(|n| bytes.checked_add(n))
        .filter(|n| *n <= 32 * 1024 * 1024)
        .ok_or_else(|| error("body named import metadata limit"))?;
    Ok(())
}
fn boundary(at: u64) -> Result<u64, Diagnostic> {
    let byte =
        u32::try_from(at).map_err(|_| error("local source position exceeds retained range"))?;
    Ok((u64::from(byte) << 32) | u64::from(u32::MAX))
}
enum Row<'a> {
    Binding(&'a crate::syntax::local_bindings::LocalBindingRecipe),
    Event(&'a crate::syntax::entered_macros::BodyMacroEvent),
    Emitted(
        &'a SourceLocalPublication,
        usize,
        &'a EmittedLocalDeclaration,
    ),
}
impl EnteredMacroIndex<'_> {
    pub fn local_index(
        &self,
        expected: &SourceSelection,
        body: &BodyKey,
        resolutions: &BTreeMap<Digest, PatternResolution>,
        expansions: &[(Key, MacroDefinition)],
    ) -> Result<SourceLocalIndex<'_>, Diagnostic> {
        self.local_index_with_publications(expected, body, resolutions, expansions, &[])
    }
    pub fn local_index_with_publications(
        &self,
        expected: &SourceSelection,
        body: &BodyKey,
        resolutions: &BTreeMap<Digest, PatternResolution>,
        expansions: &[(Key, MacroDefinition)],
        publications: &[SourceLocalPublication],
    ) -> Result<SourceLocalIndex<'_>, Diagnostic> {
        self.local_index_inner(
            expected,
            body,
            resolutions,
            expansions,
            publications,
            None,
            None,
        )
    }
    pub(crate) fn local_index_with_pattern_index(
        &self,
        expected: &SourceSelection,
        handle: &crate::syntax::outline::BodyHandle,
        resolutions: &BTreeMap<Digest, PatternResolution>,
        expansions: &[(Key, MacroDefinition)],
        index: &crate::syntax::body_index::BodyIndex,
    ) -> Result<SourceLocalIndex<'_>, Diagnostic> {
        self.local_index_with_pattern_names(expected, handle, resolutions, expansions, index, None)
    }
    fn local_index_with_pattern_names(
        &self,
        expected: &SourceSelection,
        handle: &crate::syntax::outline::BodyHandle,
        resolutions: &BTreeMap<Digest, PatternResolution>,
        expansions: &[(Key, MacroDefinition)],
        index: &crate::syntax::body_index::BodyIndex,
        names: Option<&owned_plan::LocalTypeNames>,
    ) -> Result<SourceLocalIndex<'_>, Diagnostic> {
        let admitted = self.body_context.as_ref().is_some_and(|context| {
            context.source().body() == handle
                && context.source().index().body() == index.body()
                && context.source().index().profile() == index.profile()
        });
        if handle.key() != self.body
            || index.body() != &self.body
            || index.profile() != &expected.outline.parse.profile
            || !(admitted
                || self
                    .parent
                    .view
                    .outline()
                    .items
                    .iter()
                    .any(|item| item.body.as_ref() == Some(handle)))
        {
            return Err(error("local pattern index owning body/profile differs"));
        }
        let equivalence = pattern_equivalence::prepare(&self.facts.bindings, handle, index)?;
        self.local_index_inner(
            expected,
            &handle.key(),
            resolutions,
            expansions,
            &[],
            Some(&equivalence),
            names,
        )
    }
    fn local_index_inner(
        &self,
        expected: &SourceSelection,
        body: &BodyKey,
        resolutions: &BTreeMap<Digest, PatternResolution>,
        expansions: &[(Key, MacroDefinition)],
        publications: &[SourceLocalPublication],
        equivalence: Option<&BTreeMap<Digest, Digest>>,
        declaration_names: Option<&owned_plan::LocalTypeNames>,
    ) -> Result<SourceLocalIndex<'_>, Diagnostic> {
        if expected.canonical()? != *self.parent.source() || *body != self.body {
            return Err(error("local index source/body revision mismatch"));
        }
        let mut count = self
            .facts
            .bindings
            .len()
            .checked_add(self.facts.events.len())
            .filter(|n| *n <= 1_000_000)
            .ok_or_else(|| error("source local row count exceeded"))?;
        if publications.len() > 100_000 {
            return Err(error("local publication count exceeded"));
        }
        if expansions.len() > 1_000_000 {
            return Err(error("local expansion metadata count exceeded"));
        }
        let mut emitted_bytes = publications
            .len()
            .checked_mul(std::mem::size_of::<SourceLocalPublication>())
            .ok_or_else(|| error("local publication metadata overflow"))?;
        emitted_bytes = emitted_bytes
            .checked_add(
                self.facts
                    .events
                    .len()
                    .checked_mul(std::mem::size_of::<(
                        Digest,
                        &crate::syntax::entered_macros::BodyMacroEvent,
                    )>())
                    .ok_or_else(|| error("local producer metadata overflow"))?,
            )
            .and_then(|n| n.checked_add(expansions.len().checked_mul(std::mem::size_of::<Key>())?))
            .and_then(|n| {
                n.checked_add(
                    publications
                        .len()
                        .checked_mul(std::mem::size_of::<(Digest, &SourceLocalPublication)>())?,
                )
            })
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("local publication index metadata exceeded"))?;
        let mut producers = BTreeMap::new();
        for event in &self.facts.events {
            if let BodyMacroEventKind::Producer { identity, .. } = event.kind
                && producers.insert(identity, event).is_some()
            {
                return Err(error("duplicate retained local producer"));
            }
        }
        let expansion_keys: BTreeSet<_> = expansions.iter().map(|(key, _)| *key).collect();
        let mut batches = BTreeMap::new();
        for publication in publications {
            let provenance_bytes = publication
                .source
                .outline
                .module_path
                .iter()
                .try_fold(
                    publication.source.path.as_str().len(),
                    |bytes, component| bytes.checked_add(component.len()),
                )
                .ok_or_else(|| error("publication source metadata overflow"))?;
            emitted_bytes = emitted_bytes
                .checked_add(provenance_bytes)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| error("publication source metadata exceeded"))?;
            if publication.source.canonical()? != *self.parent.source() || publication.body != *body
            {
                return Err(error("local publication source/body revision mismatch"));
            }
            emitted_bytes = emitted_bytes
                .checked_add(
                    publication
                        .declarations
                        .len()
                        .checked_mul(std::mem::size_of::<EmittedLocalDeclaration>())
                        .ok_or_else(|| error("emitted declaration metadata overflow"))?,
                )
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| error("emitted declaration metadata exceeded"))?;
            for declaration in &publication.declarations {
                if let EmittedLocalDeclaration::Binding { name, .. } = declaration {
                    emitted_bytes = emitted_bytes
                        .checked_add(name.len())
                        .filter(|n| *n <= 32 * 1024 * 1024)
                        .ok_or_else(|| error("emitted name metadata exceeded"))?;
                }
            }
            let event = producers
                .get(&publication.producer)
                .ok_or_else(|| error("publication producer is absent from retained source"))?;
            if event.at != u64::from(event.range.start)
                || !matches!(
                    event.kind,
                    BodyMacroEventKind::Producer {
                        kind: crate::syntax::outline::ProducerKind::ItemMacro,
                        ..
                    }
                )
            {
                return Err(error(
                    "publication requires generated declaration discovery",
                ));
            }
            if !expansion_keys.contains(&publication.expansion) {
                return Err(error("publication expansion metadata missing"));
            }
            if batches.insert(publication.producer, publication).is_some() {
                return Err(error("duplicate source local publication"));
            }
            count = count
                .checked_sub(1)
                .and_then(|n| n.checked_add(publication.declarations.len()))
                .filter(|n| *n <= 1_000_000)
                .ok_or_else(|| error("emitted source local row count exceeded"))?;
        }
        let metadata = count
            .checked_mul(std::mem::size_of::<(u32, u64, Digest, usize, Row<'_>)>())
            .and_then(|n| {
                n.checked_add(self.facts.bindings.len().checked_mul(
                    std::mem::size_of::<(Digest, PatternNameKind)>()
                        + std::mem::size_of::<(
                            u32,
                            u64,
                            &str,
                            crate::syntax::token_wire::NameContextKey,
                            Digest,
                        )>(),
                )?)
            })
            .and_then(|n| {
                n.checked_add(
                    self.facts
                        .scopes
                        .len()
                        .checked_mul(std::mem::size_of::<(u32, ScopeKey)>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    expansions
                        .len()
                        .checked_mul(std::mem::size_of::<(Key, MacroDefinition)>())?,
                )
            })
            .and_then(|n| n.checked_add(emitted_bytes))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("source local metadata limit exceeded"))?;
        let _ = metadata;
        let (context, facts) = self.local_hygiene();
        let mut locals = LocalScopes::new(std::sync::Arc::clone(&facts));
        let root = ScopeKey::root(body.definition);
        locals.add_root(root)?;
        let mut scopes = BTreeMap::from([(0, root)]);
        for scope in &self.facts.scopes[1..] {
            let parent = *scopes
                .get(
                    &scope
                        .parent
                        .ok_or_else(|| error("local scope parent missing"))?,
                )
                .ok_or_else(|| error("local scope parent undisclosed"))?;
            let key = locals.add_child(
                parent,
                boundary(u64::from(scope.range.start))?,
                scope.ordinal,
                false,
            )?;
            if scopes.insert(scope.ordinal, key).is_some() {
                return Err(error("duplicate local scope ordinal"));
            }
        }
        let known: BTreeMap<_, _> = self
            .facts
            .bindings
            .iter()
            .map(|b| (b.identity, b.classification))
            .collect();
        for (identity, resolution) in resolutions {
            let kind = known
                .get(identity)
                .ok_or_else(|| error("pattern resolution is absent from retained source"))?;
            if *kind == PatternNameKind::ExplicitBinding
                && *resolution != PatternResolution::Binding
            {
                return Err(error("explicit binding resolution conflicts with syntax"));
            }
        }
        let mut resolved_names = BTreeMap::new();
        let canonical = |identity: Digest| {
            equivalence
                .and_then(|map| map.get(&identity))
                .copied()
                .unwrap_or(identity)
        };
        let mut resolved_groups = BTreeMap::new();
        for (identity, resolution) in resolutions {
            if resolved_groups
                .insert(canonical(*identity), *resolution)
                .is_some_and(|old| old != *resolution)
            {
                return Err(error(
                    "equivalent pattern occurrences have conflicting resolutions",
                ));
            }
        }
        for binding in &self.facts.bindings {
            if (binding.classification == PatternNameKind::ExplicitBinding
                || resolved_groups.get(&canonical(binding.identity))
                    == Some(&PatternResolution::Binding))
                && resolved_names
                    .insert(
                        (
                            binding.scope,
                            binding.visible_from,
                            binding.name.as_str(),
                            facts.name_local(self.identifier_context(binding.name_range)?)?,
                        ),
                        canonical(binding.identity),
                    )
                    .is_some_and(|old| old != canonical(binding.identity))
            {
                return Err(error(
                    "duplicate pattern binding requires equivalence or diagnostic resolution",
                ));
            }
        }
        if expansions.len() > 1_000_000 {
            return Err(error("local expansion metadata count exceeded"));
        }
        for (expansion, definition) in expansions {
            locals.register_expansion(*expansion, definition.clone())?;
        }
        let mut rows = Vec::new();
        rows.try_reserve(count)
            .map_err(|_| error("source local row allocation failed"))?;
        for b in &self.facts.bindings {
            if canonical(b.identity) == b.identity {
                rows.push((b.scope, b.visible_from, b.identity, 0, Row::Binding(b)));
            }
        }
        for e in &self.facts.events {
            if matches!(e.kind, BodyMacroEventKind::Producer { identity, .. } if self.no_item_retired(identity))
            {
                continue;
            }
            let identity = match e.kind {
                BodyMacroEventKind::Definition { identity, .. }
                | BodyMacroEventKind::Producer { identity, .. } => identity,
            };
            let byte =
                e.at.checked_add(1)
                    .ok_or_else(|| error("local event visibility overflow"))?;
            if let Some(publication) = batches.get(&identity) {
                // Emitted statements become visible after the retained call,
                // preserving source argument lookup at its pre-expansion site.
                let byte = u64::from(e.range.end);
                for (ordinal, declaration) in publication.declarations.iter().enumerate() {
                    rows.push((
                        e.scope,
                        byte,
                        identity,
                        ordinal,
                        Row::Emitted(publication, ordinal, declaration),
                    ));
                }
            } else {
                rows.push((e.scope, byte, identity, 0, Row::Event(e)));
            }
        }
        rows.sort_by_key(|(scope, at, identity, ordinal, _)| (*scope, *at, *identity, *ordinal));
        let edition = self.parent.source().outline.parse.profile.edition;
        for (ordinal, (scope, byte, _, _, row)) in rows.into_iter().enumerate() {
            let position = boundary(byte)?
                .checked_sub(u64::from(u32::MAX))
                .and_then(|n| n.checked_add(u64::try_from(ordinal).ok()?.checked_add(1)?))
                .ok_or_else(|| error("local logical position overflow"))?;
            let scope = *scopes
                .get(&scope)
                .ok_or_else(|| error("local declaration scope missing"))?;
            match row {
                Row::Binding(b) => {
                    let resolution = match b.classification {
                        PatternNameKind::ExplicitBinding => PatternResolution::Binding,
                        PatternNameKind::BindingOrConstant => resolved_groups
                            .get(&canonical(b.identity))
                            .copied()
                            .unwrap_or(PatternResolution::NeedInput(b.identity)),
                    };
                    let name =
                        MacroName::local(&b.raw_name, self.identifier_edition(b.name_range)?)?;
                    match resolution {
                        PatternResolution::Binding => locals.bind(
                            scope,
                            position,
                            name,
                            self.identifier_context(b.name_range)?,
                            b.identity,
                        )?,
                        PatternResolution::Constant => {}
                        PatternResolution::NeedInput(input) => locals.defer_binding(
                            scope,
                            position,
                            name,
                            self.identifier_context(b.name_range)?,
                            input,
                        )?,
                    }
                }
                Row::Emitted(publication, ordinal, declaration) => match declaration {
                    EmittedLocalDeclaration::Binding { name, context } => locals.bind(
                        scope,
                        position,
                        MacroName::local(name, edition)?,
                        *context,
                        publication.binding_identity(body, ordinal)?,
                    )?,
                    EmittedLocalDeclaration::Macro { definition } => {
                        locals.define_macro(scope, position, definition.clone())?
                    }
                    EmittedLocalDeclaration::Pending { invocation } => {
                        locals.block(scope, position, *invocation)?
                    }
                },
                Row::Event(e) => match &e.kind {
                    BodyMacroEventKind::Definition { identity, .. } => {
                        let definition = if self.body_context.is_some() {
                            self.retained_definition(*identity)?.ok_or_else(|| {
                                error("actual local macro definition metadata absent")
                            })?
                        } else {
                            MacroDefinition {
                                identity: *identity,
                                defining_unit: self.parent.source().outline.crate_key,
                                definition_edition: edition,
                            }
                        };
                        locals.define_macro(scope, position, definition)?;
                    }
                    BodyMacroEventKind::Producer { identity, .. } => {
                        locals.block(scope, position, *identity)?
                    }
                },
            }
        }
        let namespace = self
            .namespace
            .as_ref()
            .map(|n| &n.facts)
            .unwrap_or(&self.facts);
        let mut item_candidates: BTreeMap<_, Vec<super::names::NameDefinition>> = BTreeMap::new();
        let mut item_frontiers: BTreeMap<_, BTreeSet<Digest>> = BTreeMap::new();
        let mut item_bytes = 0usize;
        for row in &namespace.items {
            for namespace in &row.namespaces {
                if *namespace == crate::source::manifest::Namespace::Macro {
                    continue;
                }
                let (name, definition_edition) = match declaration_names
                    .and_then(|names| names.item_spelling(row.item.def_key.fingerprint()))
                {
                    Some((name, actual_edition)) => (name.clone(), actual_edition),
                    None => (
                        MacroName::parse(&row.item.def_key.declared_name, edition)?,
                        edition,
                    ),
                };
                item_bytes = item_bytes
                    .checked_add(name.as_str().len())
                    .and_then(|n| n.checked_add(160))
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| error("body item namespace metadata limit"))?;
                item_candidates
                    .entry((row.scope, *namespace, name))
                    .or_default()
                    .push(super::names::NameDefinition {
                        namespace: *namespace,
                        identity: row.item.def_key.fingerprint(),
                        defining_unit: row.item.scope.crate_key,
                        definition_edition,
                    });
            }
            if let Some(producer) = row.attribute_input {
                item_bytes = item_bytes
                    .checked_add(128)
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| error("body attribute frontier metadata limit"))?;
                item_frontiers
                    .entry(row.scope)
                    .or_default()
                    .insert(producer);
            }
        }
        for event in &namespace.events {
            if let BodyMacroEventKind::Producer { identity, .. } = event.kind {
                if self.no_item_retired(identity) {
                    continue;
                }
                item_bytes = item_bytes
                    .checked_add(128)
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| error("body item frontier metadata limit"))?;
                item_frontiers
                    .entry(event.scope)
                    .or_default()
                    .insert(identity);
            }
        }
        let mut named_imports: BTreeMap<_, Vec<usize>> = BTreeMap::new();
        let mut glob_imports: BTreeMap<_, Vec<usize>> = BTreeMap::new();
        let mut anonymous_imports: BTreeMap<_, Vec<usize>> = BTreeMap::new();
        for (index, import) in namespace.imports.iter().enumerate() {
            item_bytes = item_bytes
                .checked_add(128)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| error("body import frontier metadata limit"))?;
            if let crate::syntax::imports::ImportKind::Named { alias } = &import.source.kind {
                let name = alias.as_deref().or_else(|| {
                    import.source.path.iter().rev().find_map(|part| {
                        if let crate::syntax::imports::ImportSegment::Name(name) = part {
                            Some(name.as_str())
                        } else {
                            None
                        }
                    })
                });
                if let Some(name) = name {
                    item_bytes = item_bytes
                        .checked_add(name.len())
                        .filter(|n| *n <= 32 * 1024 * 1024)
                        .ok_or_else(|| error("body named import metadata limit"))?;
                    named_imports
                        .entry((import.scope, MacroName::parse(name, edition)?))
                        .or_default()
                        .push(index);
                } else {
                    item_frontiers.entry(import.scope).or_default().insert(
                        super::names::NameNamespaces::import_identity(&import.source),
                    );
                }
            } else if import.source.kind == crate::syntax::imports::ImportKind::Glob {
                glob_imports.entry(import.scope).or_default().push(index);
            } else if import.source.kind == crate::syntax::imports::ImportKind::Anonymous {
                anonymous_imports
                    .entry(import.scope)
                    .or_default()
                    .push(index);
            }
            if let Some(producer) = import.attribute_input {
                item_frontiers
                    .entry(import.scope)
                    .or_default()
                    .insert(producer);
            }
        }
        Ok(SourceLocalIndex {
            entered: self,
            tables: Arc::new(SourceLocalTables {
                anonymous_imports,
                glob_imports,
                named_imports,
                item_candidates,
                item_frontiers,
                scopes,
                locals,
                context,
            }),
        })
    }
}
impl SourceLocalIndex<'_> {
    fn namespace(&self) -> &crate::syntax::entered_macros::EnteredMacroFacts {
        self.entered
            .namespace
            .as_ref()
            .map(|n| &n.facts)
            .unwrap_or(&self.entered.facts)
    }
    /// Original-source anonymous facets. This does not issue a discovery seal.
    pub fn disclosed_anonymous_imports(
        &self,
        site: OrdinaryNameSite<'_>,
        names: &super::names::NameNamespaces,
    ) -> Result<super::names::AnonymousImports, Diagnostic> {
        use super::names::{AnonymousImportOutcome, AnonymousImports};
        self.lookup(site.source, site.body, site.at, site.name, site.context)?;
        names.require_source(site.source, self.entered.parent.view.outline())?;
        let mut scope = Some(
            self.entered
                .facts
                .scopes
                .iter()
                .rev()
                .find(|s| u64::from(s.range.start) <= site.at && site.at < u64::from(s.range.end))
                .ok_or_else(|| error("anonymous lookup position outside entered scopes"))?
                .ordinal,
        );
        let mut seen = BTreeSet::new();
        let mut work = 0usize;
        let mut bytes = 0usize;
        let mut memo = ImportMemo::default();
        let mut records = Vec::new();
        while let Some(ordinal) = scope {
            if !seen.insert(ordinal) || seen.len() > 256 {
                return Err(error("anonymous body ancestry limit"));
            }
            if let Some(id) = self
                .item_frontiers
                .get(&ordinal)
                .and_then(|ids| ids.first())
            {
                return Ok(AnonymousImports::NeedInput(*id));
            }
            for index in self.anonymous_imports.get(&ordinal).into_iter().flatten() {
                match self.anonymous_import(ordinal, *index, names, &mut work, &mut memo)? {
                    AnonymousImportOutcome::Ready(record) => {
                        bytes = bytes
                            .checked_add(record.estimated_bytes()?)
                            .filter(|n| *n <= 32 * 1024 * 1024)
                            .ok_or_else(|| error("anonymous body metadata limit"))?;
                        records
                            .try_reserve(1)
                            .map_err(|_| error("anonymous body allocation failed"))?;
                        records.push(record);
                    }
                    AnonymousImportOutcome::Pending(id) => {
                        return Ok(AnonymousImports::NeedInput(id));
                    }
                    _ => {
                        return Ok(AnonymousImports::NeedInput(
                            super::names::NameNamespaces::import_identity(
                                &self.namespace().imports[*index].source,
                            ),
                        ));
                    }
                }
            }
            scope = self
                .namespace()
                .scopes
                .iter()
                .find(|s| s.ordinal == ordinal)
                .ok_or_else(|| error("anonymous body scope absent"))?
                .parent;
        }
        records.sort_by_key(|record| record.identity);
        Ok(AnonymousImports::Disclosed(records))
    }
    fn anonymous_import(
        &self,
        scope: u32,
        index: usize,
        names: &super::names::NameNamespaces,
        work: &mut usize,
        memo: &mut ImportMemo,
    ) -> Result<super::names::AnonymousImportOutcome, Diagnostic> {
        use super::names::AnonymousImportCandidates;
        use crate::syntax::imports::ImportSegment;
        let import = &self.namespace().imports[index].source;
        let mut candidates = AnonymousImportCandidates::default();
        let edition = self.entered.parent.source().outline.parse.profile.edition;
        for namespace in [
            crate::source::manifest::Namespace::Value,
            crate::source::manifest::Namespace::Type,
            crate::source::manifest::Namespace::Macro,
        ] {
            if !import.absolute
                && let [ImportSegment::Name(name)] = import.path.as_slice()
            {
                candidates.observe(self.import_lookup(
                    scope,
                    &MacroName::parse(name, edition)?,
                    namespace,
                    names,
                    work,
                    memo,
                )?)?;
            } else {
                for route in self.import_routes(scope, import, names, work)? {
                    candidates.observe(names.lookup_import_target(&route, namespace, edition)?)?;
                }
            }
        }
        Ok(candidates.finish(import))
    }
    fn import_routes(
        &self,
        scope: u32,
        source: &crate::syntax::imports::SourceImport,
        names: &super::names::NameNamespaces,
        work: &mut usize,
    ) -> Result<Vec<crate::syntax::imports::SourceImport>, Diagnostic> {
        use crate::syntax::imports::{ImportSegment, SourceImport};
        fn charge_route(bytes: &mut usize, source: &SourceImport) -> Result<(), Diagnostic> {
            import_metadata(bytes, 1, 0)?;
            for part in &source.path {
                import_metadata(
                    bytes,
                    1,
                    match part {
                        ImportSegment::Name(n) => n.len(),
                        _ => 0,
                    },
                )?;
            }
            for name in &source.scope.module_path {
                import_metadata(bytes, 1, name.len())?;
            }
            if let crate::syntax::imports::ImportKind::Named { alias: Some(alias) } = &source.kind {
                import_metadata(bytes, 1, alias.len())?;
            }
            if let crate::syntax::outline::Visibility::Restricted(tokens) = &source.visibility {
                for token in tokens {
                    import_metadata(bytes, 1, token.text.len())?;
                }
            }
            Ok(())
        }
        // Explicit continuations resolve glob bases before testing the requested
        // module prefix. The active glob set cuts cycles without native recursion.
        enum Task {
            Expand {
                slot: usize,
                scope: u32,
                source: SourceImport,
                active: BTreeSet<usize>,
            },
            Search {
                slot: usize,
                scope: u32,
                source: SourceImport,
                active: BTreeSet<usize>,
                ancestry: BTreeSet<u32>,
            },
            Weak {
                slot: usize,
                scope: u32,
                source: SourceImport,
                active: BTreeSet<usize>,
                ancestry: BTreeSet<u32>,
                children: Vec<usize>,
            },
            Union {
                slot: usize,
                children: Vec<usize>,
            },
        }
        let edition = self.entered.parent.source().outline.parse.profile.edition;
        let mut bytes = 0usize;
        charge_route(&mut bytes, source)?;
        let mut results: Vec<Vec<SourceImport>> = vec![Vec::new()];
        let mut tasks = vec![Task::Expand {
            slot: 0,
            scope,
            source: source.clone(),
            active: BTreeSet::new(),
        }];
        while let Some(task) = tasks.pop() {
            *work = work
                .checked_add(1)
                .filter(|n| *n <= 1_000_000)
                .ok_or_else(|| error("body glob route work limit"))?;
            import_metadata(&mut bytes, 1, 0)?;
            match task {
                Task::Expand {
                    slot,
                    scope,
                    source,
                    active,
                } => {
                    let routes = self.named_import_routes(scope, &source, work)?;
                    let mut children = Vec::new();
                    let mut pending = Vec::new();
                    for (scope, source) in routes {
                        charge_route(&mut bytes, &source)?;
                        let child = results.len();
                        import_metadata(&mut bytes, active.len() + 2, 0)?;
                        results.push(Vec::new());
                        children.push(child);
                        pending.push(Task::Search {
                            slot: child,
                            scope,
                            source,
                            active: active.clone(),
                            ancestry: BTreeSet::new(),
                        });
                    }
                    tasks.push(Task::Union { slot, children });
                    tasks.extend(pending);
                }
                Task::Union { slot, children } => {
                    for child in children {
                        let rows = std::mem::take(&mut results[child]);
                        import_metadata(&mut bytes, rows.len(), 0)?;
                        results[slot].extend(rows);
                    }
                }
                Task::Search {
                    slot,
                    scope,
                    source,
                    active,
                    mut ancestry,
                } => {
                    if !ancestry.insert(scope) || ancestry.len() > 256 {
                        return Err(error("body glob route ancestry limit"));
                    }
                    let first = if source.absolute {
                        None
                    } else {
                        source.path.first()
                    };
                    let Some(ImportSegment::Name(first)) = first else {
                        results[slot].push(source);
                        continue;
                    };
                    let name = MacroName::parse(first, edition)?;
                    if self.item_candidates.contains_key(&(
                        scope,
                        crate::source::manifest::Namespace::Type,
                        name,
                    )) {
                        results[slot].push(source);
                        continue;
                    }
                    let mut children = Vec::new();
                    let mut pending = Vec::new();
                    for index in self.glob_imports.get(&scope).into_iter().flatten() {
                        if active.contains(index) {
                            continue;
                        }
                        charge_route(&mut bytes, &self.namespace().imports[*index].source)?;
                        let mut active = active.clone();
                        active.insert(*index);
                        if active.len() > 256 {
                            return Err(error("body glob route depth limit"));
                        }
                        import_metadata(&mut bytes, active.len() + 2, 0)?;
                        let child = results.len();
                        results.push(Vec::new());
                        children.push(child);
                        pending.push(Task::Expand {
                            slot: child,
                            scope,
                            source: self.namespace().imports[*index].source.clone(),
                            active,
                        });
                    }
                    import_metadata(&mut bytes, ancestry.len() + active.len(), 0)?;
                    tasks.push(Task::Weak {
                        slot,
                        scope,
                        source,
                        active,
                        ancestry,
                        children,
                    });
                    tasks.extend(pending);
                }
                Task::Weak {
                    slot,
                    scope,
                    source,
                    active,
                    ancestry,
                    children,
                } => {
                    let Some(ImportSegment::Name(first)) = source.path.first() else {
                        return Err(error("body glob route continuation lost prefix"));
                    };
                    for child in children {
                        for mut candidate in std::mem::take(&mut results[child]) {
                            candidate.path.push(ImportSegment::Name(first.clone()));
                            if let super::macro_names::ModuleLookup::Defined { .. } =
                                names.lookup_import_module(&candidate, edition)?
                            {
                                charge_route(&mut bytes, &source)?;
                                let mut next = source.clone();
                                next.path = candidate.path;
                                next.path.extend_from_slice(&source.path[1..]);
                                next.scope = candidate.scope;
                                next.absolute = candidate.absolute;
                                if next.path.len() > 256 {
                                    return Err(error("body glob module path depth limit"));
                                }
                                let text = next
                                    .path
                                    .iter()
                                    .try_fold(0usize, |n, part| {
                                        n.checked_add(match part {
                                            ImportSegment::Name(n) => n.len(),
                                            _ => 0,
                                        })
                                    })
                                    .ok_or_else(|| error("body glob route path size overflow"))?;
                                import_metadata(&mut bytes, next.path.len(), text)?;
                                results[slot].push(next);
                            }
                        }
                    }
                    if results[slot].is_empty() {
                        if let Some(parent) = self
                            .namespace()
                            .scopes
                            .iter()
                            .find(|s| s.ordinal == scope)
                            .ok_or_else(|| error("body glob route scope absent"))?
                            .parent
                        {
                            tasks.push(Task::Search {
                                slot,
                                scope: parent,
                                source,
                                active,
                                ancestry,
                            });
                        } else {
                            results[slot].push(source);
                        }
                    }
                }
            }
        }
        let mut seen = BTreeSet::new();
        Ok(std::mem::take(&mut results[0])
            .into_iter()
            .filter(|route| seen.insert(route.fingerprint()))
            .collect())
    }
    fn named_import_routes(
        &self,
        scope: u32,
        source: &crate::syntax::imports::SourceImport,
        work: &mut usize,
    ) -> Result<Vec<(u32, crate::syntax::imports::SourceImport)>, Diagnostic> {
        use crate::syntax::imports::ImportSegment;
        let mut stack = vec![(scope, source.clone())];
        let mut seen = BTreeSet::new();
        let mut routes = Vec::new();
        let mut bytes = 0usize;
        while let Some((scope, source)) = stack.pop() {
            *work = work
                .checked_add(1)
                .filter(|n| *n <= 1_000_000)
                .ok_or_else(|| error("body module alias work limit"))?;
            let text = source
                .path
                .iter()
                .try_fold(0usize, |n, part| {
                    n.checked_add(match part {
                        ImportSegment::Name(n) => n.len(),
                        _ => 0,
                    })
                })
                .ok_or_else(|| error("body module alias path size overflow"))?;
            import_metadata(&mut bytes, source.path.len(), text)?;
            if source.path.len() > 256 {
                return Err(error("body module alias path depth limit"));
            }
            if !seen.insert((scope, source.fingerprint())) {
                continue;
            }
            let first = if source.absolute {
                None
            } else {
                source.path.first().and_then(|p| {
                    if let ImportSegment::Name(n) = p {
                        Some(n)
                    } else {
                        None
                    }
                })
            };
            let mut binding = None;
            if let Some(first) = first {
                let name = MacroName::parse(
                    first,
                    self.entered.parent.source().outline.parse.profile.edition,
                )?;
                let mut current = Some(scope);
                let mut ancestry = BTreeSet::new();
                while let Some(scope) = current {
                    if !ancestry.insert(scope) || ancestry.len() > 256 {
                        return Err(error("body module alias ancestry limit"));
                    }
                    if let Some(imports) = self.named_imports.get(&(scope, name.clone())) {
                        binding = Some((scope, imports));
                        break;
                    }
                    if self.item_candidates.contains_key(&(
                        scope,
                        crate::source::manifest::Namespace::Type,
                        name.clone(),
                    )) {
                        break;
                    }
                    current = self
                        .namespace()
                        .scopes
                        .iter()
                        .find(|s| s.ordinal == scope)
                        .ok_or_else(|| error("body module alias scope absent"))?
                        .parent;
                }
            }
            if let Some((scope, indices)) = binding {
                for index in indices {
                    let original = &self.namespace().imports[*index].source;
                    let mut next = source.clone();
                    next.path = original.path.clone();
                    next.path.extend_from_slice(&source.path[1..]);
                    next.absolute = original.absolute;
                    next.scope = original.scope.clone();
                    let text = next
                        .path
                        .iter()
                        .try_fold(0usize, |n, part| {
                            n.checked_add(match part {
                                ImportSegment::Name(n) => n.len(),
                                _ => 0,
                            })
                        })
                        .ok_or_else(|| error("body module alias path size overflow"))?;
                    import_metadata(&mut bytes, next.path.len(), text)?;
                    if next.path.len() > 256 {
                        return Err(error("body module alias path depth limit"));
                    }
                    stack.push((scope, next));
                }
            } else {
                routes.push((scope, source));
            }
        }
        Ok(routes)
    }
    fn glob_lookup(
        &self,
        scope: u32,
        name: &MacroName,
        namespace: crate::source::manifest::Namespace,
        names: &super::names::NameNamespaces,
        work: &mut usize,
    ) -> Result<super::names::NameLookup, Diagnostic> {
        use super::names::NameLookup;
        let mut definitions = BTreeSet::new();
        let mut metadata = 0usize;
        for index in self.glob_imports.get(&scope).into_iter().flatten() {
            *work = work
                .checked_add(1)
                .filter(|n| *n <= 1_000_000)
                .ok_or_else(|| error("body glob work limit"))?;
            let import = &self.namespace().imports[*index].source;
            for route in self.import_routes(scope, import, names, work)? {
                match names.lookup_glob_member(
                    &route,
                    name.as_str(),
                    namespace,
                    self.entered.parent.source().outline.parse.profile.edition,
                )? {
                    NameLookup::Defined(d) => {
                        import_metadata(&mut metadata, 1, 0)?;
                        definitions.insert(d);
                    }
                    NameLookup::Ambiguous(ds) => {
                        import_metadata(&mut metadata, ds.len(), 0)?;
                        definitions.extend(ds);
                    }
                    NameLookup::NeedInput(id) => return Ok(NameLookup::NeedInput(id)),
                    NameLookup::Absent => {}
                    NameLookup::Inaccessible(paths) => return Ok(NameLookup::Inaccessible(paths)),
                    NameLookup::AmbiguousModules(paths) => {
                        return Ok(NameLookup::AmbiguousModules(paths));
                    }
                    _ => return Err(error("unsupported body glob member outcome")),
                }
            }
        }
        Ok(if definitions.len() == 1 {
            NameLookup::Defined(definitions.pop_first().expect("one candidate"))
        } else if definitions.len() > 1 {
            NameLookup::Ambiguous(definitions.into_iter().collect())
        } else {
            NameLookup::Absent
        })
    }
    fn import_lookup(
        &self,
        scope: u32,
        name: &MacroName,
        namespace: crate::source::manifest::Namespace,
        names: &super::names::NameNamespaces,
        work: &mut usize,
        memo: &mut ImportMemo,
    ) -> Result<super::names::NameLookup, Diagnostic> {
        use super::names::NameLookup;
        use crate::syntax::imports::ImportSegment;
        let mut linear = true;
        let mut scratch_bytes = 0usize;
        import_metadata(&mut scratch_bytes, 1, name.as_str().len())?;
        let mut stack = vec![(scope, name.clone())];
        let mut seen = BTreeSet::new();
        let mut definitions = BTreeSet::new();
        let mut demands = BTreeSet::new();
        let mut denied = Vec::new();
        let mut roots = BTreeSet::new();
        while let Some((mut scope, name)) = stack.pop() {
            if !seen.insert((scope, name.clone())) {
                linear = false;
                continue;
            }
            if let Some(found) = memo.entries.get(&(scope, namespace, name.clone())) {
                match found {
                    NameLookup::Defined(d) => {
                        definitions.insert(d.clone());
                    }
                    NameLookup::ModuleRoot(unit) => {
                        roots.insert(*unit);
                    }
                    NameLookup::Absent => {}
                    _ => return Err(error("invalid body import memo outcome")),
                }
                continue;
            }
            loop {
                import_metadata(&mut scratch_bytes, 1, name.as_str().len())?;
                *work = work
                    .checked_add(1)
                    .filter(|n| *n <= 1_000_000)
                    .ok_or_else(|| error("body named import work limit"))?;
                if let Some(id) = self.item_frontiers.get(&scope).and_then(|ids| ids.first()) {
                    demands.insert(*id);
                    break;
                }
                let items = self.item_candidates.get(&(scope, namespace, name.clone()));
                let imports = self.named_imports.get(&(scope, name.clone()));
                if items.map_or(0, Vec::len) + imports.map_or(0, Vec::len) > 1 {
                    linear = false;
                }
                if let Some(items) = items {
                    definitions.extend(items.iter().cloned());
                }
                if let Some(imports) = imports {
                    for index in imports {
                        *work = work
                            .checked_add(1)
                            .filter(|n| *n <= 1_000_000)
                            .ok_or_else(|| error("body named import work limit"))?;
                        let row = self
                            .namespace()
                            .imports
                            .get(*index)
                            .ok_or_else(|| error("body named import input absent"))?;
                        if !row.source.absolute
                            && row.source.path.len() > 1
                            && let Some(ImportSegment::Name(first)) = row.source.path.first()
                        {
                            let first = MacroName::parse(
                                first,
                                self.entered.parent.source().outline.parse.profile.edition,
                            )?;
                            if !self.named_imports.contains_key(&(scope, first.clone()))
                                && !self.item_candidates.contains_key(&(
                                    scope,
                                    crate::source::manifest::Namespace::Type,
                                    first.clone(),
                                ))
                            {
                                match self.glob_lookup(
                                    scope,
                                    &first,
                                    crate::source::manifest::Namespace::Type,
                                    names,
                                    work,
                                )? {
                                    NameLookup::Ambiguous(ds) => {
                                        return Ok(NameLookup::Ambiguous(ds));
                                    }
                                    NameLookup::NeedInput(id) => {
                                        return Ok(NameLookup::NeedInput(id));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        if !row.source.absolute
                            && let [ImportSegment::Name(target)] = row.source.path.as_slice()
                        {
                            import_metadata(&mut scratch_bytes, 1, target.len())?;
                            stack.push((
                                scope,
                                MacroName::parse(
                                    target,
                                    self.entered.parent.source().outline.parse.profile.edition,
                                )?,
                            ));
                            continue;
                        }
                        let routes = self.import_routes(scope, &row.source, names, work)?;
                        if routes.len() > 1 {
                            linear = false;
                        }
                        for route in routes {
                            match names.lookup_import_target(
                                &route,
                                namespace,
                                self.entered.parent.source().outline.parse.profile.edition,
                            )? {
                                NameLookup::Defined(d) => {
                                    definitions.insert(d);
                                }
                                NameLookup::Ambiguous(ds) => {
                                    import_metadata(&mut scratch_bytes, ds.len(), 0)?;
                                    definitions.extend(ds);
                                }
                                NameLookup::NeedInput(id) => {
                                    demands.insert(id);
                                }
                                NameLookup::Inaccessible(paths) => {
                                    let text = paths
                                        .iter()
                                        .flat_map(|p| p.components())
                                        .try_fold(0usize, |n, name| {
                                            n.checked_add(name.as_str().len())
                                        })
                                        .ok_or_else(|| {
                                            error("body import denied path size overflow")
                                        })?;
                                    import_metadata(&mut scratch_bytes, paths.len(), text)?;
                                    denied.extend(paths);
                                }
                                NameLookup::ModuleRoot(unit) => {
                                    roots.insert(unit);
                                }
                                NameLookup::Absent => {}
                                NameLookup::AmbiguousModules(paths) => {
                                    return Ok(NameLookup::AmbiguousModules(paths));
                                }
                                NameLookup::LocalBinding(_) => {
                                    return Err(error(
                                        "body import target cannot be a local binding",
                                    ));
                                }
                            }
                        }
                    }
                }
                if items.is_some() || imports.is_some() {
                    break;
                }
                match self.glob_lookup(scope, &name, namespace, names, work)? {
                    NameLookup::Defined(d) => {
                        definitions.insert(d);
                        break;
                    }
                    NameLookup::Ambiguous(ds) => {
                        import_metadata(&mut scratch_bytes, ds.len(), 0)?;
                        definitions.extend(ds);
                        break;
                    }
                    NameLookup::NeedInput(id) => {
                        demands.insert(id);
                        break;
                    }
                    NameLookup::Absent => {}
                    found => return Ok(found),
                }
                if let Some(parent) = self
                    .namespace()
                    .scopes
                    .iter()
                    .find(|s| s.ordinal == scope)
                    .ok_or_else(|| error("body import scope absent"))?
                    .parent
                {
                    scope = parent;
                } else {
                    match names.lookup_unqualified(
                        namespace,
                        name.as_str(),
                        &self.entered.module_scope,
                        self.entered.parent.source().outline.parse.profile.edition,
                        None,
                    )? {
                        NameLookup::Defined(d) => {
                            definitions.insert(d);
                        }
                        NameLookup::NeedInput(id) => {
                            demands.insert(id);
                        }
                        NameLookup::Ambiguous(ds) => {
                            import_metadata(&mut scratch_bytes, ds.len(), 0)?;
                            definitions.extend(ds);
                        }
                        NameLookup::Inaccessible(paths) => {
                            let text = paths
                                .iter()
                                .flat_map(|p| p.components())
                                .try_fold(0usize, |n, name| n.checked_add(name.as_str().len()))
                                .ok_or_else(|| error("body import denied path size overflow"))?;
                            import_metadata(&mut scratch_bytes, paths.len(), text)?;
                            denied.extend(paths);
                        }
                        NameLookup::ModuleRoot(unit) => {
                            roots.insert(unit);
                        }
                        NameLookup::Absent => {}
                        _ => return Err(error("unsupported body import target outcome")),
                    }
                    break;
                }
            }
        }
        let found = if let Some(id) = demands.first() {
            NameLookup::NeedInput(*id)
        } else if definitions.len() == 1 {
            NameLookup::Defined(definitions.pop_first().expect("one candidate"))
        } else if definitions.len() > 1 {
            NameLookup::Ambiguous(definitions.into_iter().collect())
        } else if let Some(unit) = roots.first() {
            NameLookup::ModuleRoot(*unit)
        } else if !denied.is_empty() {
            denied.sort();
            denied.dedup();
            NameLookup::Inaccessible(denied)
        } else {
            NameLookup::Absent
        };
        // A single-path traversal gives every visited alias the same outcome.
        // Branches and cycles cannot share this certificate.
        if linear
            && matches!(
                found,
                NameLookup::Defined(_) | NameLookup::Absent | NameLookup::ModuleRoot(_)
            )
        {
            for (scope, name) in seen {
                let key = (scope, namespace, name);
                if !memo.entries.contains_key(&key) {
                    import_metadata(&mut memo.bytes, 2, key.2.as_str().len())?;
                    memo.entries.insert(key, found.clone());
                }
            }
        }
        Ok(found)
    }
    /// Qualified module paths in an exact entered body. Associated-item paths
    /// belong to the later type/trait resolver; macros use retained invocations.
    pub fn lookup_qualified_module(
        &self,
        site: OrdinaryNameSite<'_>,
        path: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        names: &super::names::NameNamespaces,
    ) -> Result<super::names::NameLookup, Diagnostic> {
        use super::names::NameLookup;
        use crate::syntax::imports::{ImportKind, ImportSegment, SourceImport};
        if site.namespace == crate::source::manifest::Namespace::Macro {
            return Err(error("qualified macros require retained invocation lookup"));
        }
        self.lookup(site.source, site.body, site.at, site.name, site.context)?;
        names.require_source(site.source, self.entered.parent.view.outline())?;
        if path.is_empty() || path.len() > 256 {
            return Err(error("qualified body module path depth limit"));
        }
        let text = path
            .iter()
            .try_fold(0usize, |n, part| {
                n.checked_add(match part {
                    ImportSegment::Name(n) => n.len(),
                    _ => 0,
                })
            })
            .ok_or_else(|| error("qualified body path size overflow"))?;
        let mut metadata = 0usize;
        import_metadata(&mut metadata, path.len(), text)?;
        let scope = self
            .entered
            .facts
            .scopes
            .iter()
            .rev()
            .find(|s| u64::from(s.range.start) <= site.at && site.at < u64::from(s.range.end))
            .ok_or_else(|| error("qualified body path position outside entered scopes"))?;
        let mut work = 0usize;
        let mut memo = ImportMemo::default();
        if !absolute && let Some(ImportSegment::Name(first)) = path.first() {
            let first = MacroName::parse(first, site.source.outline.parse.profile.edition)?;
            let mut current = Some(scope.ordinal);
            let mut seen = BTreeSet::new();
            while let Some(scope) = current {
                if !seen.insert(scope) || seen.len() > 256 {
                    return Err(error("qualified body path ancestry limit"));
                }
                if let Some(producer) = self.item_frontiers.get(&scope).and_then(|ids| ids.first())
                {
                    return Ok(NameLookup::NeedInput(*producer));
                }
                if let Some(indices) = self.named_imports.get(&(scope, first.clone())) {
                    match self.import_lookup(
                        scope,
                        &first,
                        crate::source::manifest::Namespace::Type,
                        names,
                        &mut work,
                        &mut memo,
                    )? {
                        NameLookup::NeedInput(id) => return Ok(NameLookup::NeedInput(id)),
                        NameLookup::Absent => {
                            return Ok(NameLookup::NeedInput(
                                super::names::NameNamespaces::import_identity(
                                    &self.namespace().imports[indices[0]].source,
                                ),
                            ));
                        }
                        NameLookup::Inaccessible(paths) => {
                            return Ok(NameLookup::Inaccessible(paths));
                        }
                        NameLookup::Ambiguous(ds) => return Ok(NameLookup::Ambiguous(ds)),
                        _ => {}
                    }
                    break;
                }
                match self.glob_lookup(
                    scope,
                    &first,
                    crate::source::manifest::Namespace::Type,
                    names,
                    &mut work,
                )? {
                    NameLookup::NeedInput(id) => return Ok(NameLookup::NeedInput(id)),
                    NameLookup::Ambiguous(ds) => return Ok(NameLookup::Ambiguous(ds)),
                    NameLookup::Defined(_) => break,
                    _ => {}
                }
                current = self
                    .namespace()
                    .scopes
                    .iter()
                    .find(|s| s.ordinal == scope)
                    .ok_or_else(|| error("qualified body scope absent"))?
                    .parent;
            }
        }
        let byte =
            u32::try_from(site.at).map_err(|_| error("qualified body path position overflow"))?;
        let mut source = SourceImport {
            identity: site.body.fingerprint(),
            leaf_ordinal: 0,
            scope: self.entered.module_scope.clone(),
            range: crate::syntax::spans::SourceRange {
                start: byte,
                end: byte,
            },
            path: path.to_vec(),
            absolute,
            kind: ImportKind::Named { alias: None },
            visibility: crate::syntax::outline::Visibility::Private,
        };
        let mut e = StableEncoder::new("lrr/v2/ordinary-body-module-path");
        e.record(&source);
        e.u32(site.namespace.tag());
        e.digest(Digest(site.context.0));
        source.identity = e.finish();
        let mut definitions = BTreeSet::new();
        let mut roots = BTreeSet::new();
        let mut denied = Vec::new();
        for route in self.import_routes(scope.ordinal, &source, names, &mut work)? {
            match names.lookup_import_target(
                &route,
                site.namespace,
                site.source.outline.parse.profile.edition,
            )? {
                NameLookup::Defined(d) => {
                    definitions.insert(d);
                }
                NameLookup::Ambiguous(ds) => {
                    import_metadata(&mut metadata, ds.len(), 0)?;
                    definitions.extend(ds);
                }
                NameLookup::NeedInput(id) => return Ok(NameLookup::NeedInput(id)),
                NameLookup::Inaccessible(paths) => denied.extend(paths),
                NameLookup::ModuleRoot(unit) => {
                    roots.insert(unit);
                }
                NameLookup::Absent => {}
                NameLookup::AmbiguousModules(paths) => {
                    return Ok(NameLookup::AmbiguousModules(paths));
                }
                NameLookup::LocalBinding(_) => {
                    return Err(error("qualified module member cannot be a local binding"));
                }
            }
        }
        Ok(if definitions.len() == 1 {
            NameLookup::Defined(definitions.pop_first().expect("one candidate"))
        } else if definitions.len() > 1 {
            NameLookup::Ambiguous(definitions.into_iter().collect())
        } else if let Some(unit) = roots.first() {
            NameLookup::ModuleRoot(*unit)
        } else if !denied.is_empty() {
            denied.sort();
            denied.dedup();
            NameLookup::Inaccessible(denied)
        } else {
            NameLookup::Absent
        })
    }
    /// Source-ordered hygienic values shadow module and supplied prelude facts.
    /// Other namespaces retain their separate module view; no body is evaluated.
    pub fn lookup_ordinary(
        &self,
        site: OrdinaryNameSite<'_>,
        names: &super::names::NameNamespaces,
        prelude: Option<&crate::syntax::outline::ScopeKey>,
    ) -> Result<super::names::NameLookup, Diagnostic> {
        match self.lookup_ordinary_prefix(&site, names)? {
            Some(found) => Ok(found),
            None => names.lookup_unqualified(
                site.namespace,
                site.name,
                &self.entered.module_scope,
                site.source.outline.parse.profile.edition,
                prelude,
            ),
        }
    }
    /// The same local/import/frontier kernel precedes every module lookup.
    /// None is a module demand, never a proved absence or namespace authority.
    /// The owning occurrence query must retain the actual input/dependency proof.
    pub(crate) fn lookup_ordinary_prefix(
        &self,
        site: &OrdinaryNameSite<'_>,
        names: &super::names::NameNamespaces,
    ) -> Result<Option<super::names::NameLookup>, Diagnostic> {
        match self.lookup_prefix_inner(
            site,
            Some(names),
            None,
            site.source.outline.parse.profile.edition,
        )? {
            PrefixResolution::Name(found) => Ok(Some(found)),
            PrefixResolution::ModuleTail => Ok(None),
            _ => Err(error(
                "ordinary graph adapter lacks required prefix service",
            )),
        }
    }
    fn lookup_prepared_type_prefix(
        &self,
        site: &OrdinaryNameSite<'_>,
        evidence: &owned_plan::LocalTypeNames,
        edition: u16,
    ) -> Result<PrefixResolution, Diagnostic> {
        if site.namespace != crate::source::manifest::Namespace::Type {
            return Err(error("prepared Type prefix namespace differs"));
        }
        self.lookup_prefix_inner(site, None, Some(evidence), edition)
    }
    fn lookup_prefix_inner(
        &self,
        site: &OrdinaryNameSite<'_>,
        names: Option<&super::names::NameNamespaces>,
        evidence: Option<&owned_plan::LocalTypeNames>,
        edition: u16,
    ) -> Result<PrefixResolution, Diagnostic> {
        use super::names::NameLookup;
        // Exact selection/position validation remains shared with local lookup.
        self.lookup_in_edition(
            site.source,
            site.body,
            site.at,
            site.name,
            site.context,
            edition,
        )?;
        if let Some(names) = names {
            names.require_source(site.source, self.entered.parent.view.outline())?;
        }
        if let Some(evidence) = evidence {
            if self
                .entered
                .body_context
                .as_ref()
                .is_none_or(|context| context.source().identity() != evidence.owner())
            {
                return Err(error("local Type declaration evidence owner differs"));
            }
        }
        if site.namespace == crate::source::manifest::Namespace::Macro {
            use super::{macro_origins::MacroSelection, macro_scopes::HygienicLookup};
            let definition = |d: MacroDefinition| super::names::NameDefinition {
                namespace: site.namespace,
                identity: d.identity,
                defining_unit: d.defining_unit,
                definition_edition: d.definition_edition,
            };
            return Ok(PrefixResolution::Name(
                match self.entered.lookup_with_context(
                    site.source,
                    site.body,
                    site.at,
                    site.name,
                    site.context,
                    edition,
                )? {
                    HygienicLookup::NeedInput(id) => NameLookup::NeedInput(id),
                    HygienicLookup::Selection(MacroSelection::Defined(d)) => {
                        NameLookup::Defined(definition(d))
                    }
                    HygienicLookup::Selection(MacroSelection::Ambiguous(ds)) => {
                        NameLookup::Ambiguous(ds.into_iter().map(definition).collect())
                    }
                    HygienicLookup::Selection(MacroSelection::Absent) => NameLookup::Absent,
                },
            ));
        }

        let scope = self
            .entered
            .facts
            .scopes
            .iter()
            .rev()
            .find(|s| u64::from(s.range.start) <= site.at && site.at < u64::from(s.range.end))
            .ok_or_else(|| error("ordinary lookup position outside entered scopes"))?;
        let name = MacroName::local(site.name, edition)?;
        let (local, local_scope) = self.locals.lookup_with_scope(
            self.scopes[&scope.ordinal],
            boundary(site.at)?,
            &name,
            site.context,
        )?;
        let mut current = Some(scope.ordinal);
        let mut seen = BTreeSet::new();
        let mut import_work = 0usize;
        let mut memo = ImportMemo::default();
        while let Some(ordinal) = current {
            if !seen.insert(ordinal) || seen.len() > 256 {
                return Err(error("ordinary body item ancestry limit"));
            }
            if site.namespace == crate::source::manifest::Namespace::Value
                && self.scopes.get(&ordinal) == Some(&local_scope)
            {
                match &local {
                    LocalLookup::Binding(id) => {
                        return Ok(PrefixResolution::Name(NameLookup::LocalBinding(*id)));
                    }
                    LocalLookup::NeedInput(id) => {
                        return Ok(PrefixResolution::Name(NameLookup::NeedInput(*id)));
                    }
                    LocalLookup::Absent => {}
                }
            }
            if let Some(producer) = self
                .item_frontiers
                .get(&ordinal)
                .and_then(|ids| ids.first())
            {
                return Ok(PrefixResolution::Name(NameLookup::NeedInput(*producer)));
            }
            for index in self.anonymous_imports.get(&ordinal).into_iter().flatten() {
                let Some(names) = names else {
                    return Ok(PrefixResolution::NeedImport(
                        self.namespace().imports[*index].source.identity,
                    ));
                };
                use super::names::AnonymousImportOutcome;
                match self.anonymous_import(ordinal, *index, names, &mut import_work, &mut memo)? {
                    AnonymousImportOutcome::Ready(_) => {}
                    AnonymousImportOutcome::Pending(id) => {
                        return Ok(PrefixResolution::Name(NameLookup::NeedInput(id)));
                    }
                    _ => {
                        return Ok(PrefixResolution::Name(NameLookup::NeedInput(
                            super::names::NameNamespaces::import_identity(
                                &self.namespace().imports[*index].source,
                            ),
                        )));
                    }
                }
            }
            for ((scope, alias), indices) in &self.named_imports {
                if *scope != ordinal {
                    continue;
                }
                let Some(names) = names else {
                    return Ok(PrefixResolution::NeedImport(
                        self.namespace().imports[indices[0]].source.identity,
                    ));
                };
                let mut ready = false;
                for namespace in [
                    crate::source::manifest::Namespace::Value,
                    crate::source::manifest::Namespace::Type,
                    crate::source::manifest::Namespace::Macro,
                ] {
                    match self.import_lookup(
                        ordinal,
                        alias,
                        namespace,
                        names,
                        &mut import_work,
                        &mut memo,
                    )? {
                        NameLookup::Defined(_) | NameLookup::ModuleRoot(_) => ready = true,
                        NameLookup::NeedInput(id) => {
                            return Ok(PrefixResolution::Name(NameLookup::NeedInput(id)));
                        }
                        NameLookup::Ambiguous(ds)
                            if alias == &name && namespace == site.namespace =>
                        {
                            return Ok(PrefixResolution::Name(NameLookup::Ambiguous(ds)));
                        }
                        NameLookup::AmbiguousModules(paths)
                            if alias == &name && namespace == site.namespace =>
                        {
                            return Ok(PrefixResolution::Name(NameLookup::AmbiguousModules(paths)));
                        }
                        _ => {}
                    }
                }
                if !ready {
                    let source = &self.namespace().imports[indices[0]].source;
                    return Ok(PrefixResolution::Name(NameLookup::NeedInput(
                        super::names::NameNamespaces::import_identity(source),
                    )));
                }
            }
            if self.named_imports.contains_key(&(ordinal, name.clone())) {
                let Some(names) = names else {
                    return Ok(PrefixResolution::NeedImport(
                        self.namespace().imports[self.named_imports[&(ordinal, name.clone())][0]]
                            .source
                            .identity,
                    ));
                };
                let found = self.import_lookup(
                    ordinal,
                    &name,
                    site.namespace,
                    names,
                    &mut import_work,
                    &mut memo,
                )?;
                if found != NameLookup::Absent {
                    return Ok(PrefixResolution::Name(found));
                }
            }
            if let Some(items) = self
                .item_candidates
                .get(&(ordinal, site.namespace, name.clone()))
            {
                if let Some(evidence) = evidence {
                    match evidence
                        .items(items, self.locals.facts_root().name_item(site.context)?)?
                    {
                        owned_plan::ItemNames::NeedDeclaration(identity) => {
                            return Ok(PrefixResolution::NeedDeclaration(identity));
                        }
                        owned_plan::ItemNames::Selected(Some(found)) => {
                            return Ok(PrefixResolution::Name(found));
                        }
                        owned_plan::ItemNames::Selected(None) => {}
                    }
                } else {
                    return Ok(PrefixResolution::Name(if items.len() == 1 {
                        NameLookup::Defined(items[0].clone())
                    } else {
                        NameLookup::Ambiguous(items.clone())
                    }));
                }
            }
            if let Some(names) = names {
                let glob =
                    self.glob_lookup(ordinal, &name, site.namespace, names, &mut import_work)?;
                if glob != NameLookup::Absent {
                    return Ok(PrefixResolution::Name(glob));
                }
            } else if let Some(index) = self
                .glob_imports
                .get(&ordinal)
                .and_then(|rows| rows.first())
            {
                return Ok(PrefixResolution::NeedImport(
                    self.namespace().imports[*index].source.identity,
                ));
            }
            current = self
                .entered
                .namespace
                .as_ref()
                .map(|n| &n.facts)
                .unwrap_or(&self.entered.facts)
                .scopes
                .iter()
                .find(|s| s.ordinal == ordinal)
                .ok_or_else(|| error("ordinary body item scope absent"))?
                .parent;
        }
        if let Some(evidence) = evidence {
            if let Some(parameter) =
                evidence.generic(&name, self.locals.facts_root().name_item(site.context)?)?
            {
                return Ok(PrefixResolution::Parameter(parameter));
            }
            if evidence.needs_owner() {
                return Ok(PrefixResolution::NeedOwnerGeneric);
            }
        }
        Ok(PrefixResolution::ModuleTail)
    }
    pub(crate) fn context_at(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<Key, Diagnostic> {
        self.entered.identifier_context(range)
    }
    pub fn context(&self) -> Key {
        self.context
    }
    pub fn lookup(
        &self,
        expected: &SourceSelection,
        body: &BodyKey,
        at: u64,
        name: &str,
        context: Key,
    ) -> Result<LocalLookup, Diagnostic> {
        self.lookup_in_edition(
            expected,
            body,
            at,
            name,
            context,
            self.entered.parent.source().outline.parse.profile.edition,
        )
    }
    fn lookup_in_edition(
        &self,
        expected: &SourceSelection,
        body: &BodyKey,
        at: u64,
        name: &str,
        context: Key,
        edition: u16,
    ) -> Result<LocalLookup, Diagnostic> {
        if expected.canonical()? != *self.entered.parent.source() || *body != self.entered.body {
            return Err(error("local lookup source/body revision mismatch"));
        }
        if self
            .entered
            .facts
            .opaque
            .iter()
            .any(|r| u64::from(r.start) <= at && at < u64::from(r.end))
        {
            return Err(error("local lookup requires separate opaque syntax entry"));
        }
        let scope = self
            .entered
            .facts
            .scopes
            .iter()
            .rev()
            .find(|s| u64::from(s.range.start) <= at && at < u64::from(s.range.end))
            .ok_or_else(|| error("local lookup position outside entered scopes"))?;
        self.locals.lookup(
            self.scopes[&scope.ordinal],
            boundary(at)?,
            &MacroName::local(name, edition)?,
            context,
        )
    }
}
