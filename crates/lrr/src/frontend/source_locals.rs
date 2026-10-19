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
pub use owned_plan::PreparedSourceLocalPlan;
#[path = "local_declaration_names.rs"]
mod declaration_names;
use declaration_names::{DeclarationNameIndex, DeclarationNameTables};
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
    binding_names: BTreeMap<(u32, u32), u32>,
    binding_source_identity: Digest,
    scopes: BTreeMap<u32, ScopeKey>,
    locals: LocalScopes,
    context: Key,
    declarations: DeclarationNameTables,
}
impl Deref for SourceLocalTables {
    type Target = DeclarationNameTables;
    fn deref(&self) -> &Self::Target { &self.declarations }
}
impl SourceLocalTables {
    pub(crate) fn binding_source_identity(&self) -> Digest {
        self.binding_source_identity
    }
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
        add(map(&self.binding_names)?)?;
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
            .ok_or_else(|| error("source local row count exceeded"))?;
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
                .ok_or_else(|| error("emitted declaration metadata exceeded"))?;
            for declaration in &publication.declarations {
                if let EmittedLocalDeclaration::Binding { name, .. } = declaration {
                    emitted_bytes = emitted_bytes
                        .checked_add(name.len())
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
        let declarations = DeclarationNameTables::prepare(
            namespace, edition, declaration_names, |identity| self.no_item_retired(identity),
        )?;
        self.facts
            .bindings
            .len()
            .checked_mul(16 * std::mem::size_of::<((u32, u32), u32)>() + 256)
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut binding_names = BTreeMap::new();
        let mut binding_identity = StableEncoder::new("lrr/v1/entered-binding-name-index");
        binding_identity.u64(self.facts.bindings.len() as u64); // existing bounded facts
        for (ordinal, binding) in self.facts.bindings.iter().enumerate() {
            let ordinal = u32::try_from(ordinal).map_err(|_| crate::runtime::arena::resource())?;
            if binding_names
                .insert((binding.name_range.start, binding.name_range.end), ordinal)
                .is_some()
            {
                return Err(error("entered binding name range has multiple recipes"));
            }
        }
        for ((start, end), ordinal) in &binding_names {
            binding_identity.u32(*start);
            binding_identity.u32(*end);
            binding_identity.u32(*ordinal);
            binding_identity.digest(self.facts.bindings[*ordinal as usize].identity);
        }
        let binding_source_identity = binding_identity.finish();
        Ok(SourceLocalIndex {
            entered: self,
            tables: Arc::new(SourceLocalTables {
                binding_names,
                binding_source_identity,
                declarations,
                scopes,
                locals,
                context,
            }),
        })
    }
}
impl SourceLocalIndex<'_> {
    fn declaration_index(&self) -> DeclarationNameIndex<'_> {
        DeclarationNameIndex::new(&self.declarations, self.namespace(),
            &self.entered.module_scope, self.entered.parent.source().outline.parse.profile.edition)
    }

    /// The entered function parameter introducing local `identity`, if any.
    /// A parameter's pattern lies in the signature, outside the indexed body.
    pub(in crate::frontend::source_locals) fn parameter_recipe(
        &self,
        identity: Digest,
    ) -> Option<&crate::syntax::local_bindings::LocalBindingRecipe> {
        self.entered.facts.bindings.iter().find(|binding| {
            binding.identity == identity
                && matches!(binding.form, crate::syntax::local_bindings::BindingForm::Parameter | crate::syntax::local_bindings::BindingForm::Receiver)
        })
    }
    pub(in crate::frontend::source_locals) fn binding_recipe(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<Option<&crate::syntax::local_bindings::LocalBindingRecipe>, Diagnostic> {
        self.binding_names
            .get(&(range.start, range.end))
            .map(|ordinal| {
                self.entered
                    .facts
                    .bindings
                    .get(*ordinal as usize)
                    .filter(|binding| binding.name_range == range)
                    .ok_or_else(|| error("entered binding name index differs from retained recipe"))
            })
            .transpose()
    }
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
            if !seen.insert(ordinal) {
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
        names: &impl super::names::reads::QualifiedNames,
        work: &mut usize,
        memo: &mut ImportMemo,
    ) -> Result<super::names::AnonymousImportOutcome, Diagnostic> {
        self.declaration_index().anonymous_import(scope, index, names, work, memo)
    }
    fn import_routes(
        &self,
        scope: u32,
        source: &crate::syntax::imports::SourceImport,
        names: &impl super::names::reads::QualifiedNames,
        work: &mut usize,
    ) -> Result<Vec<crate::syntax::imports::SourceImport>, Diagnostic> {
        self.declaration_index().import_routes(scope, source, names, work)
    }
    fn glob_lookup(
        &self,
        scope: u32,
        name: &MacroName,
        namespace: crate::source::manifest::Namespace,
        names: &impl super::names::reads::QualifiedNames,
        work: &mut usize,
    ) -> Result<super::names::NameLookup, Diagnostic> {
        self.declaration_index().glob_lookup(scope, name, namespace, names, work)
    }
    fn import_lookup(
        &self,
        scope: u32,
        name: &MacroName,
        namespace: crate::source::manifest::Namespace,
        names: &impl super::names::reads::QualifiedNames,
        work: &mut usize,
        memo: &mut ImportMemo,
    ) -> Result<super::names::NameLookup, Diagnostic> {
        self.declaration_index().import_lookup(scope, name, namespace, names, work, memo)
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
        self.lookup_qualified_module_kernel(site, path, absolute, names, None)
    }
    pub(crate) fn lookup_qualified_module_observed(
        &self,
        site: OrdinaryNameSite<'_>,
        path: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        names: &super::names::NameNamespaces,
    ) -> Result<
        (
            super::names::NameLookup,
            super::names::reads::NamespaceReads,
        ),
        Diagnostic,
    > {
        self.lookup_qualified_module_rooted_observed(site, path, absolute, names, None)
    }
    /// Same ordinary module resolver, with the actual emitting crate root of
    /// a source keyword. The raw context must prove that unit independently.
    pub(crate) fn lookup_qualified_module_rooted_observed(
        &self,
        site: OrdinaryNameSite<'_>,
        path: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        names: &super::names::NameNamespaces,
        defining_root: Option<Digest>,
    ) -> Result<(super::names::NameLookup, super::names::reads::NamespaceReads), Diagnostic> {
        let observed = super::names::reads::ObservedNames::new(names);
        let result = self.lookup_qualified_module_kernel(site, path, absolute, &observed, defining_root)?;
        Ok((result, observed.finish()))
    }
    fn lookup_qualified_module_kernel(
        &self,
        site: OrdinaryNameSite<'_>,
        path: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        names: &impl super::names::reads::QualifiedNames,
        defining_root: Option<Digest>,
    ) -> Result<super::names::NameLookup, Diagnostic> {
        use super::names::NameLookup;
        use crate::syntax::imports::{ImportKind, ImportSegment, SourceImport};
        if site.namespace == crate::source::manifest::Namespace::Macro {
            return Err(error("qualified macros require retained invocation lookup"));
        }
        let scope = self.require_lookup_position(site.source, site.body, site.at)?;
        if let Some(unit) = defining_root {
            if absolute || !matches!(path.first(), Some(ImportSegment::Crate))
                || Digest(self.entered.local_hygiene().1.dollar_crate(site.context)?.0) != unit
            {
                return Err(error("qualified defining-crate root differs from actual keyword context"));
            }
        }
        // Keywords route structurally; they are not identifier/local bindings.
        // Actual pinned context and the same entered position remain required.
        if matches!(path.first(), Some(ImportSegment::Name(_))) {
            self.lookup(site.source, site.body, site.at, site.name, site.context)?;
        } else {
            self.entered.local_hygiene().1.name_item(site.context)?;
        }
        names.require_source(site.source, self.entered.parent.view.outline())?;
        if path.is_empty() {
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
        let mut work = 0usize;
        let mut memo = ImportMemo::default();
        if !absolute && let Some(ImportSegment::Name(first)) = path.first() {
            let first = MacroName::parse(first, site.source.outline.parse.profile.edition)?;
            let mut current = Some(scope.ordinal);
            let mut seen = BTreeSet::new();
            while let Some(scope) = current {
                if !seen.insert(scope) {
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
            scope: defining_root.map_or_else(|| self.entered.module_scope.clone(), |crate_key| crate::syntax::outline::ScopeKey {
                crate_key, module_path: Vec::new(), owner: None,
            }),
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
            None,
        )? {
            PrefixResolution::Name(found) => Ok(Some(found)),
            PrefixResolution::ModuleTail => Ok(None),
            _ => Err(error(
                "ordinary graph adapter lacks required prefix service",
            )),
        }
    }
    pub(in crate::frontend::source_locals) fn lookup_prepared_value_prefix_observed(
        &self,
        site: &OrdinaryNameSite<'_>,
        evidence: &owned_plan::LocalTypeNames,
        names: &super::names::NameNamespaces,
    ) -> Result<(PrefixResolution, super::names::reads::NamespaceReads), Diagnostic> {
        // Value paths and call callees select in the Value namespace; a
        // record pattern's name is a Type-namespace name of the same prefix.
        if site.namespace == crate::source::manifest::Namespace::Macro {
            return Err(error("prepared Value prefix namespace differs"));
        }
        let observed = super::names::reads::ObservedNames::new(names);
        let found = self.lookup_prefix_inner(
            site,
            Some(&observed),
            Some(evidence),
            site.source.outline.parse.profile.edition,
            None,
        )?;
        Ok((found, observed.finish()))
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
        self.lookup_prefix_inner::<super::names::NameNamespaces>(
            site,
            None,
            Some(evidence),
            edition,
            None,
        )
    }
    fn lookup_prepared_field_type_prefix(
        &self,
        site: &OrdinaryNameSite<'_>,
        evidence: &owned_plan::LocalTypeNames,
        edition: u16,
        header: &owned_plan::field_types::HeaderTypeSite<'_>,
    ) -> Result<PrefixResolution, Diagnostic> {
        self.lookup_prefix_inner::<super::names::NameNamespaces>(
            site,
            None,
            Some(evidence),
            edition,
            Some(header),
        )
    }
    fn lookup_prefix_inner<N: super::names::reads::QualifiedNames>(
        &self,
        site: &OrdinaryNameSite<'_>,
        names: Option<&N>,
        evidence: Option<&owned_plan::LocalTypeNames>,
        edition: u16,
        header: Option<&owned_plan::field_types::HeaderTypeSite<'_>>,
    ) -> Result<PrefixResolution, Diagnostic> {
        use super::names::NameLookup;
        // Exact selection/position validation remains shared with local lookup.
        if let Some(header) = header {
            let owner = self
                .entered
                .body_context
                .as_ref()
                .ok_or_else(|| error("declaration Type body context absent"))?;
            if site.source.canonical()? != *self.entered.parent.source()
                || *site.body != self.entered.body
            {
                return Err(error("declaration Type owning source/body differs"));
            }
            // This exact header occurrence, not a caller position or boolean,
            // discharges the ordinary opaque-item entry boundary.
            header.require(owner.source().identity(), site, edition)?;
        } else {
            self.lookup_in_edition(
                site.source,
                site.body,
                site.at,
                site.name,
                site.context,
                edition,
            )?;
        }
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
            .find(|s| {
                if let Some(header) = header {
                    s.ordinal == header.scope()
                } else {
                    u64::from(s.range.start) <= site.at && site.at < u64::from(s.range.end)
                }
            })
            .ok_or_else(|| error("ordinary lookup position outside entered scopes"))?;
        let name = MacroName::local(site.name, edition)?;
        if let Some(header) = header {
            if let Some(parameter) =
                header.generic(&name, self.locals.facts_root().name_item(site.context)?)?
            {
                return Ok(PrefixResolution::Parameter(parameter));
            }
        }
        let (local, local_scope) = if header.is_some() {
            (
                LocalLookup::Absent,
                *self
                    .scopes
                    .get(&scope.ordinal)
                    .ok_or_else(|| error("declaration Type lexical scope absent"))?,
            )
        } else {
            self.locals.lookup_with_scope(
                self.scopes[&scope.ordinal],
                boundary(site.at)?,
                &name,
                site.context,
            )?
        };
        let found = self.declaration_index().lookup_prefix(
            scope.ordinal, site.namespace, &name,
            || self.locals.facts_root().name_item(site.context), names, evidence,
            |ordinal| {
                if self.scopes.get(&ordinal) != Some(&local_scope) { return None; }
                match &local {
                    LocalLookup::Binding(id) => Some(NameLookup::LocalBinding(*id)),
                    LocalLookup::NeedInput(id) => Some(NameLookup::NeedInput(*id)),
                    LocalLookup::Absent => None,
                }
            },
        )?;
        if !matches!(found, PrefixResolution::ModuleTail) { return Ok(found); }
        // Local declaration headers own their generic scope; they cannot
        // capture a containing function's generic by equal spelling.
        if let Some(evidence) = evidence.filter(|_| {
            header.is_none() && site.namespace == crate::source::manifest::Namespace::Type
        }) {
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
    fn require_lookup_position(
        &self,
        expected: &SourceSelection,
        body: &BodyKey,
        at: u64,
    ) -> Result<&crate::syntax::entered_macros::BodyMacroScope, Diagnostic> {
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
        Ok(scope)
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
        let scope = self.require_lookup_position(expected, body, at)?;
        self.locals.lookup(
            self.scopes[&scope.ordinal],
            boundary(at)?,
            &MacroName::local(name, edition)?,
            context,
        )
    }
}
