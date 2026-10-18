//! Namespace-qualified owned lookup over the existing deterministic import engine.
//! This selects definitions only; it never expands, type-checks or executes a body.
#[path = "namespace_reads.rs"]
pub(crate) mod reads;
#[path = "names_retention.rs"]
pub(super) mod retention;
use super::{
    generated_headers::declared_names::DeclaredItemName,
    macro_names::{MacroNamespaces, MacroPath, NameBindingClass, PathLookup},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::scopes::{MacroDefinition, MacroName},
    source::manifest::Namespace,
    syntax::{outline::ScopeKey, visibility::VisibilityScope},
};
use std::sync::Arc;
pub type NamePath = MacroPath;
pub(crate) fn owned_name(name: &str, edition: u16) -> Result<MacroName, Diagnostic> {
    // Source names include the receiver keyword, as in ordinary local lookup.
    // Macro definition names still use MacroName::parse at their own boundary.
    MacroName::local(name, edition).or_else(|_| super::source_macro_routes::semantic(name, edition))
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct NameDefinition {
    pub namespace: Namespace,
    pub identity: Digest,
    pub defining_unit: Digest,
    pub definition_edition: u16,
}
impl StableEncode for NameDefinition {
    const DOMAIN: &'static str = "lrr/v2/namespace-definition";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(self.namespace.tag());
        e.digest(self.identity);
        e.digest(self.defining_unit);
        e.u16(self.definition_edition);
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NameLookup {
    Defined(NameDefinition),
    Ambiguous(Vec<NameDefinition>),
    AmbiguousModules(Vec<NamePath>),
    ModuleRoot(Digest),
    LocalBinding(Digest),
    Absent,
    Inaccessible(Vec<NamePath>),
    NeedInput(Digest),
}
impl StableEncode for NameLookup {
    const DOMAIN: &'static str = "lrr/v2/namespace-lookup";
    fn encode_fields(&self, e: &mut StableEncoder) {
        let path = |e: &mut StableEncoder, p: &NamePath| {
            e.digest(p.unit);
            e.sequence(p.components(), |e, n| e.string(n.as_str()));
        };
        match self {
            Self::Defined(d) => {
                e.u32(0);
                e.record(d);
            }
            Self::Ambiguous(ds) => {
                e.u32(1);
                e.sequence(ds, |e, d| e.record(d));
            }
            Self::AmbiguousModules(paths) => {
                e.u32(2);
                e.sequence(paths, path);
            }
            Self::ModuleRoot(id) => {
                e.u32(3);
                e.digest(*id);
            }
            Self::LocalBinding(id) => {
                e.u32(4);
                e.digest(*id);
            }
            Self::Absent => e.u32(5),
            Self::Inaccessible(paths) => {
                e.u32(6);
                e.sequence(paths, path);
            }
            Self::NeedInput(id) => {
                e.u32(7);
                e.digest(*id);
            }
        }
    }
}
impl NameLookup {
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let fail = || {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "namespace lookup size overflow".into(),
            )
        };
        let mut bytes = std::mem::size_of::<Self>();
        match self {
            Self::Ambiguous(ds) => {
                bytes = bytes
                    .checked_add(
                        ds.capacity()
                            .checked_mul(std::mem::size_of::<NameDefinition>())
                            .ok_or_else(fail)?,
                    )
                    .ok_or_else(fail)?
            }
            Self::AmbiguousModules(paths) | Self::Inaccessible(paths) => {
                bytes = bytes
                    .checked_add(
                        paths
                            .capacity()
                            .checked_mul(std::mem::size_of::<NamePath>())
                            .ok_or_else(fail)?,
                    )
                    .ok_or_else(fail)?;
                for path in paths {
                    bytes = bytes
                        .checked_add(
                            path.components()
                                .len()
                                .checked_mul(std::mem::size_of::<MacroName>())
                                .ok_or_else(fail)?,
                        )
                        .ok_or_else(fail)?;
                    for part in path.components() {
                        bytes = bytes.checked_add(part.as_str().len()).ok_or_else(fail)?;
                    }
                }
            }
            _ => {}
        }
        Ok(bytes)
    }
}
#[derive(Debug, Default, Clone)]
pub struct ImportResolution {
    pub resolved: Vec<Digest>,
    pub pending: Vec<Digest>,
    pub missing: Vec<Digest>,
    pub inaccessible: Vec<Digest>,
    pub ambiguous: Vec<Digest>,
}
/// Disclosed anonymous facets; this record is not a discovery or trait-universe seal.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AnonymousImport {
    pub identity: Digest,
    pub scope: ScopeKey,
    pub definitions: Vec<NameDefinition>,
    pub module_root: Option<Digest>,
}
impl StableEncode for AnonymousImport {
    const DOMAIN: &'static str = "lrr/v2/anonymous-import";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.identity);
        e.record(&self.scope);
        e.sequence(&self.definitions, |e, d| e.record(d));
        e.bool(self.module_root.is_some());
        if let Some(unit) = self.module_root {
            e.digest(unit);
        }
    }
}
impl AnonymousImport {
    pub(crate) fn estimated_bytes(&self) -> Result<usize, Diagnostic> {
        self.scope.module_path.iter().try_fold(
            256usize
                .checked_add(
                    self.definitions
                        .len()
                        .checked_mul(160)
                        .ok_or(Diagnostic::new(
                            "LRR-E-SOURCE-LIMIT",
                            "anonymous facet size overflow".into(),
                        ))?,
                )
                .ok_or(Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "anonymous import size overflow".into(),
                ))?,
            |n, name| {
                n.checked_add(name.len())
                    .and_then(|n| n.checked_add(std::mem::size_of::<String>()))
                    .ok_or(Diagnostic::new(
                        "LRR-E-SOURCE-LIMIT",
                        "anonymous scope size overflow".into(),
                    ))
            },
        )
    }
}
#[derive(Debug)]
pub enum AnonymousImports {
    Disclosed(Vec<AnonymousImport>),
    NeedInput(Digest),
}
pub(crate) enum AnonymousImportOutcome {
    Ready(AnonymousImport),
    Pending(Digest),
    Missing,
    Inaccessible,
    Ambiguous,
}
#[derive(Default)]
pub(crate) struct AnonymousImportCandidates {
    definitions: std::collections::BTreeSet<NameDefinition>,
    roots: std::collections::BTreeSet<Digest>,
    pending: std::collections::BTreeSet<Digest>,
    denied: bool,
    ambiguous: bool,
}
impl AnonymousImportCandidates {
    pub(crate) fn observe(&mut self, found: NameLookup) -> Result<(), Diagnostic> {
        match found {
            NameLookup::Defined(d) => {
                self.definitions.insert(d);
            }
            NameLookup::ModuleRoot(unit) => {
                self.roots.insert(unit);
            }
            NameLookup::NeedInput(id) => {
                self.pending.insert(id);
            }
            NameLookup::Inaccessible(_) => self.denied = true,
            NameLookup::Ambiguous(_) | NameLookup::AmbiguousModules(_) => self.ambiguous = true,
            NameLookup::Absent => {}
            NameLookup::LocalBinding(_) => {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-SCOPE",
                    "anonymous import cannot select a local binding".into(),
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn finish(
        self,
        import: &crate::syntax::imports::SourceImport,
    ) -> AnonymousImportOutcome {
        if let Some(id) = self.pending.first() {
            return AnonymousImportOutcome::Pending(*id);
        }
        if self.ambiguous || self.roots.len() > 1 {
            return AnonymousImportOutcome::Ambiguous;
        }
        if !self.definitions.is_empty() || !self.roots.is_empty() {
            return AnonymousImportOutcome::Ready(AnonymousImport {
                identity: NameNamespaces::import_identity(import),
                scope: import.scope.clone(),
                definitions: self.definitions.into_iter().collect(),
                module_root: self.roots.first().copied(),
            });
        }
        if self.denied {
            AnonymousImportOutcome::Inaccessible
        } else {
            AnonymousImportOutcome::Missing
        }
    }
}
type GlobItemEdge = (Namespace, NamePath, NamePath, VisibilityScope);
/// Digests are uniformly distributed, so their leading bytes already hash.
#[derive(Default, Clone, Copy)]
pub(crate) struct DigestHasher(u64);
impl std::hash::Hasher for DigestHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.0 = self.0.rotate_left(17) ^ u64::from_le_bytes(word);
        }
    }
}
pub(crate) type DigestSet =
    std::collections::HashSet<Digest, std::hash::BuildHasherDefault<DigestHasher>>;
type GlobModuleEdge = (
    super::module_paths::ModulePath,
    super::module_paths::ModulePath,
    VisibilityScope,
);
#[derive(Clone, Default)]
pub struct NameNamespaces {
    ordinary_source_bytes: usize,
    declared_names: std::collections::BTreeMap<(Namespace, Digest), Arc<DeclaredItemName>>,
    declared_name_bytes: usize,
    module_frontier_scopes: std::collections::BTreeMap<Digest, ScopeKey>,
    ordinary_definitions: std::collections::BTreeSet<Digest>,
    anonymous: std::collections::BTreeMap<Digest, AnonymousImport>,
    source_glob_items: std::collections::BTreeSet<GlobItemEdge>,
    source_glob_modules: std::collections::BTreeSet<GlobModuleEdge>,
    manual_globs: std::collections::BTreeSet<GlobItemEdge>,
    manual_glob_bytes: usize,
    generated_frontier_bytes: usize,
    extern_alias_bytes: usize,
    named_module_bytes: usize,
    named_module_routes: usize,
    source_proofs: std::collections::BTreeMap<crate::db::key::OutlineKey, Digest>,
    values: MacroNamespaces,
    types: MacroNamespaces,
    macros: MacroNamespaces,
    frontiers: std::collections::BTreeMap<ScopeKey, std::collections::BTreeSet<Digest>>,
    imports: std::collections::BTreeMap<Digest, (crate::syntax::imports::SourceImport, u16)>,
    /// Identities in `imports` whose kind is named or anonymous: a producer
    /// frontier that ignores imports skips them unless deferred.
    named_import_ids: DigestSet,
    links: std::collections::BTreeMap<(Namespace, NamePath), std::collections::BTreeSet<NamePath>>,
    visibility: std::collections::BTreeMap<(Namespace, Digest), VisibilityScope>,
    modules: std::collections::BTreeSet<Digest>,
    deferred_imports: std::collections::BTreeSet<Digest>,
    module_aliases: std::collections::BTreeMap<NamePath, super::module_paths::ModulePath>,
    named_module_links: std::collections::BTreeMap<
        NamePath,
        std::collections::BTreeSet<super::module_paths::ModulePath>,
    >,
    candidate_globs: std::collections::BTreeSet<Digest>,
    macro_import_facets: std::collections::BTreeMap<Digest, NameDefinition>,
    glob_module_links: std::collections::BTreeMap<NamePath, std::collections::BTreeSet<NamePath>>,
    /// Enum variant member identities admitted beneath their enum's module-like
    /// path. Variants are not outline items and own no source inputs; their
    /// declaration facts stay with the owning enum's schema.
    enum_variants: std::collections::BTreeMap<Digest, EnumVariantDefinition>,
    /// The module-like scopes of enums (`crate`, enum path), whose members are
    /// variants fixed by the enum's declaration, not by a source scope.
    enum_scopes: std::collections::BTreeSet<(Digest, Vec<String>)>,
    /// Each crate's standard prelude module (`std::prelude::rust_20xx`, or
    /// core's for `#![no_std]`), selected by the compiler-injected import of
    /// its root source. Absent for `#![no_core]`/crate `#![no_implicit_prelude]`.
    standard_preludes: std::collections::BTreeMap<Digest, ScopeKey>,
    /// Advanced by every graph mutator. `resolve_imports` is a deterministic
    /// function of the graph, so a fixed point computed at this generation
    /// is still the fixed point while no mutation has happened since.
    import_generation: u64,
    import_fixed_point: Option<(u64, ImportResolution)>,
    /// Key of the macro-route graph facts this graph last received.
    route_install_key: Option<Digest>,
}
/// Facts shared while certifying many scopes of one unchanged graph (see
/// `NameNamespaces::certified_scope_in`): the member-name universe per
/// namespace and each caller crate's standard-prelude answers. Prelude items
/// are public re-exports, so their answers do not vary between the modules
/// of one crate; whether a module sees the prelude is still checked per scope.
#[derive(Default)]
pub(crate) struct CertificationBatch {
    universe: std::collections::BTreeMap<u32, std::collections::BTreeSet<MacroName>>,
    prelude: std::collections::BTreeMap<(Digest, u32, u16), Vec<(MacroName, NameLookup)>>,
}
impl CertificationBatch {
    fn universe(
        &mut self,
        graph: &NameNamespaces,
        namespace: Namespace,
    ) -> &std::collections::BTreeSet<MacroName> {
        self.universe
            .entry(namespace.tag())
            .or_insert_with(|| graph.table(namespace).member_names())
    }
    fn prelude(
        &mut self,
        graph: &NameNamespaces,
        caller: &ScopeKey,
        namespace: Namespace,
        edition: u16,
    ) -> Result<Option<&Vec<(MacroName, NameLookup)>>, Diagnostic> {
        if namespace == Namespace::Macro {
            return Ok(None);
        }
        let Some(prelude) = graph.standard_preludes.get(&caller.crate_key) else {
            return Ok(None);
        };
        if !graph.table(namespace).implicit_enabled(caller) {
            return Ok(None);
        }
        let key = (caller.crate_key, namespace.tag(), edition);
        if !self.prelude.contains_key(&key) {
            let mut answers = Vec::new();
            for name in graph
                .table(namespace)
                .scoped_member_names(prelude, edition)
                .unwrap_or_default()
            {
                let answer = graph.lookup_standard_prelude(
                    namespace,
                    name.as_str(),
                    caller,
                    edition,
                    &mut reads::NamespaceReads::disabled(),
                )?;
                if answer != NameLookup::Absent {
                    answers.push((name, answer));
                }
            }
            self.prelude.insert(key.clone(), answers);
        }
        Ok(self.prelude.get(&key))
    }
}
/// The owning enum and semantic variant name of an admitted variant identity.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EnumVariantDefinition {
    pub enumeration: Digest,
    pub name: String,
}
impl NameNamespaces {
    /// Select an enum variant member by the identity a namespace lookup returned.
    pub(crate) fn enum_variant(&self, identity: Digest) -> Option<&EnumVariantDefinition> {
        self.enum_variants.get(&identity)
    }
    /// Whether `scope` is an enum's module-like namespace of variants.
    pub(crate) fn is_enum_scope(&self, scope: &ScopeKey) -> bool {
        scope.owner.is_none()
            && self
                .enum_scopes
                .contains(&(scope.crate_key, scope.module_path.clone()))
    }
    /// rustc resolves an enum as a module-like namespace of its variants: the
    /// enum's own path becomes a module prefix whose members are the variants,
    /// defined in the Type namespace and, for unit/tuple constructors, in the
    /// Value namespace, all with the enum's visibility. Ordinary paths, named
    /// imports and globs then reach variants through the shared graph walk.
    fn admit_enum_variants(
        &mut self,
        item: &crate::syntax::outline::ItemOutline,
        edition: u16,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        let mut path = item
            .scope
            .module_path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        path.push(&item.def_key.declared_name);
        let visibility =
            VisibilityScope::for_declaration(&item.header.visibility, &item.scope, edition)?;
        self.declare_module(item.scope.crate_key, &path, edition, visibility.clone())?;
        self.enum_scopes.insert((
            item.scope.crate_key,
            path.iter().map(|part| (*part).to_owned()).collect(),
        ));
        let enumeration = item.def_key.fingerprint();
        for variant in &item.variants {
            let name = crate::syntax::ast_adapter::semantic_name(&variant.name).ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "enum variant name is not a valid identifier".into(),
                )
            })?;
            let identity = crate::syntax::outline::variant_identity(enumeration, &name);
            let mut member = path.clone();
            member.push(&variant.name);
            let member = Self::path(item.scope.crate_key, &member, edition)?;
            let namespaces: &[Namespace] = if variant.has_value_constructor() {
                &[Namespace::Type, Namespace::Value]
            } else {
                &[Namespace::Type]
            };
            for namespace in namespaces {
                self.define(
                    member.clone(),
                    NameDefinition {
                        namespace: *namespace,
                        identity,
                        defining_unit: item.scope.crate_key,
                        definition_edition: edition,
                    },
                    visibility.clone(),
                )?;
            }
            if self
                .enum_variants
                .insert(identity, EnumVariantDefinition { enumeration, name })
                .is_some()
            {
                return Err(Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "duplicate enum variant definition identity".into(),
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn declared_value_identities(
        &self,
        unit: Digest,
    ) -> Result<std::collections::BTreeSet<Digest>, Diagnostic> {
        let mut ids = std::collections::BTreeSet::new();
        for (path, bindings) in &self.values.bindings {
            if path.unit != unit {
                continue;
            }
            for definition in &bindings.definitions {
                // Enum variants are members of their enum, not source items.
                if definition.defining_unit == unit
                    && !self.enum_variants.contains_key(&definition.identity)
                {
                    ids.insert(definition.identity);
                }
            }
        }
        Ok(ids)
    }
    fn source_key(outline: &crate::syntax::outline::Outline) -> crate::db::key::OutlineKey {
        crate::db::key::OutlineKey {
            parse: crate::syntax::parse::ParseKey {
                content: outline.content,
                profile: outline.profile.clone(),
            },
            crate_key: outline.root_scope.crate_key,
            module_path: outline.root_scope.module_path.clone(),
            cfg: outline.cfg,
        }
    }
    pub(crate) fn source_proof(outline: &crate::syntax::outline::Outline) -> Digest {
        let mut e = StableEncoder::new("lrr/v2/ordinary-source-proof");
        e.digest(outline.content);
        e.record(&outline.profile);
        e.digest(outline.cfg);
        e.record(&outline.root_scope);
        e.record(outline);
        e.sequence(&outline.imports, |e, import| e.record(import));
        e.finish()
    }
    /// `require_source` for a published view, using its memoized proof.
    pub(crate) fn require_view(
        &self,
        view: &crate::source::changes::PreparedSourceView,
    ) -> Result<(), Diagnostic> {
        let source = view.selection();
        if source.canonical()?.outline != Self::source_key(view.outline())
            || self.source_proofs.get(&source.outline) != Some(&view.ordinary_source_proof())
        {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-SCOPE",
                "ordinary namespace/source revision mismatch".into(),
            ));
        }
        Ok(())
    }
    /// Source pairing only: this is not a scope seal or generated-source certificate.
    pub fn require_source(
        &self,
        source: &crate::source::changes::SourceSelection,
        outline: &crate::syntax::outline::Outline,
    ) -> Result<(), Diagnostic> {
        if source.canonical()?.outline != Self::source_key(outline)
            || self.source_proofs.get(&source.outline) != Some(&Self::source_proof(outline))
        {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-SCOPE",
                "ordinary namespace/source revision mismatch".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn item_visibility(
        item: &crate::syntax::outline::ItemOutline,
        namespace: Namespace,
        edition: u16,
    ) -> Result<VisibilityScope, Diagnostic> {
        let mut visibility =
            VisibilityScope::for_declaration(&item.header.visibility, &item.scope, edition)?;
        if namespace == Namespace::Value
            && item.def_key.item_kind == crate::source::manifest::ItemKind::Struct
        {
            for field in &item.constructor_fields {
                let range = VisibilityScope::for_declaration(field, &item.scope, edition)?;
                visibility = super::glob_lookup::intersection(&visibility, &range).ok_or(
                    Diagnostic::new(
                        "LRR-E-MACRO-SCOPE",
                        "incompatible constructor field visibility ranges".into(),
                    ),
                )?;
            }
        }
        Ok(visibility)
    }
    pub fn import_identity(import: &crate::syntax::imports::SourceImport) -> Digest {
        let mut e = StableEncoder::new("lrr/v2/ordinary-import-leaf");
        e.digest(import.identity);
        e.u32(import.leaf_ordinal);
        e.finish()
    }

    /// Original names cannot survive an unresolved replacing transformation.
    pub(crate) fn hide_pending_attribute_declarations(
        &mut self,
        identities: &std::collections::BTreeSet<Digest>,
    ) {
        self.import_generation = self.import_generation.wrapping_add(1);
        for table in [&mut self.values, &mut self.types, &mut self.macros] {
            for bindings in table.bindings.values_mut() {
                bindings
                    .definitions
                    .retain(|definition| !identities.contains(&definition.identity));
                bindings
                    .definition_access
                    .retain(|definition, _| !identities.contains(&definition.identity));
            }
        }
    }
    /// Restore only the original item certified by the transformation worklist.
    pub(crate) fn restore_source_attribute_declaration(
        &mut self,
        outline: &crate::syntax::outline::Outline,
        identity: Digest,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        if self.source_proofs.get(&Self::source_key(outline)) != Some(&Self::source_proof(outline))
            || !self.ordinary_definitions.contains(&identity)
        {
            return Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "settled source item does not belong to the captured namespace".into(),
            ));
        }
        let item = outline
            .items
            .iter()
            .find(|i| i.def_key.fingerprint() == identity)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "settled source item absent".into(),
                )
            })?;
        let members = outline.scopes.get(&item.scope).ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "settled source scope absent".into(),
            )
        })?;
        for ((namespace, name), ids) in &members.candidates {
            if !ids.contains(&identity) {
                continue;
            }
            let mut path = item
                .scope
                .module_path
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            path.push(name);
            self.define(
                Self::path(item.scope.crate_key, &path, outline.profile.edition)?,
                NameDefinition {
                    namespace: *namespace,
                    identity,
                    defining_unit: item.scope.crate_key,
                    definition_edition: outline.profile.edition,
                },
                Self::item_visibility(item, *namespace, outline.profile.edition)?,
            )?;
        }
        Ok(())
    }
    /// Active declaration facts only. Imports and unknown producers remain explicit frontiers.
    pub fn from_outlines(
        outlines: &[&crate::syntax::outline::Outline],
    ) -> Result<Self, Diagnostic> {
        let mut graph = Self::default();
        graph.admit_source_outlines(outlines)?;
        Ok(graph)
    }
    pub(crate) fn admit_source_outline(
        &mut self,
        outline: &crate::syntax::outline::Outline,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        if outline.root_scope.module_path.is_empty() {
            return Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "derived source must have a retained child scope".into(),
            ));
        }
        self.admit_source_outlines(&[outline])
    }
    fn admit_source_outlines(
        &mut self,
        outlines: &[&crate::syntax::outline::Outline],
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        use crate::source::manifest::ItemKind;
        let fault = |message: &str| Diagnostic::new("LRR-E-SOURCE-LIMIT", message.into());
        let graph = self;
        let mut definitions = std::collections::BTreeMap::new();
        let mut bytes = graph.ordinary_source_bytes;
        let mut roots = graph
            .source_proofs
            .keys()
            .filter(|k| k.module_path.is_empty())
            .map(|k| k.crate_key)
            .collect::<std::collections::BTreeSet<_>>();
        let mut imports = graph.imports.len();
        for outline in outlines {
            if !outline.root_scope.module_path.is_empty() {
                continue;
            }
            graph.declare_module(
                outline.root_scope.crate_key,
                &[],
                outline.profile.edition,
                VisibilityScope::Public,
            )?;
            roots.insert(outline.root_scope.crate_key);
        }
        for outline in outlines {
            if outline.root_scope.owner.is_some() || !roots.contains(&outline.root_scope.crate_key)
            {
                return Err(fault("namespace unit root outline is absent"));
            }
            let externs = outline
                .extern_crates
                .iter()
                .map(|row| row.identity)
                .collect::<std::collections::BTreeSet<_>>();
            for item in &outline.items {
                bytes = bytes
                    .checked_add(96)
                    .and_then(|n| n.checked_add(std::mem::size_of_val(item)))
                    .and_then(|n| n.checked_add(item.header.estimated_bytes()))
                    .and_then(|n| {
                        item.constructor_field_bytes()
                            .ok()
                            .and_then(|size| n.checked_add(size))
                    })
                    .ok_or_else(|| fault("namespace outline metadata limit"))?;
                if !graph
                    .ordinary_definitions
                    .insert(item.def_key.fingerprint())
                    || definitions
                        .insert(item.def_key.fingerprint(), item)
                        .is_some()
                {
                    return Err(fault("duplicate source definition identity"));
                }
                if item.def_key.item_kind == ItemKind::Module && item.scope.owner.is_none() {
                    graph.modules.insert(item.def_key.fingerprint());
                    let mut path = item
                        .scope
                        .module_path
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>();
                    path.push(&item.def_key.declared_name);
                    let visibility = VisibilityScope::for_declaration(
                        &item.header.visibility,
                        &item.scope,
                        outline.profile.edition,
                    )?;
                    if !externs.contains(&item.def_key.fingerprint()) {
                        graph.declare_module(
                            item.scope.crate_key,
                            &path,
                            outline.profile.edition,
                            visibility,
                        )?;
                    }
                }
                if item.def_key.item_kind == ItemKind::Enum && item.scope.owner.is_none() {
                    graph.admit_enum_variants(item, outline.profile.edition)?;
                }
            }
        }
        for outline in outlines {
            for (scope, members) in &outline.scopes {
                if scope.owner.is_some() {
                    continue;
                }
                let scope_bytes = scope
                    .module_path
                    .iter()
                    .try_fold(std::mem::size_of_val(scope), |n, name| {
                        n.checked_add(name.capacity())
                    })
                    .ok_or_else(|| fault("namespace scope size overflow"))?;
                let frontier_bytes = members
                    .blockers
                    .len()
                    .checked_mul(96)
                    .and_then(|n| n.checked_add(scope_bytes))
                    .ok_or_else(|| fault("namespace frontier size overflow"))?;
                bytes = bytes
                    .checked_add(frontier_bytes)
                    .ok_or_else(|| fault("namespace frontier metadata limit"))?;
                for blocker in &members.blockers {
                    graph
                        .frontiers
                        .entry(scope.clone())
                        .or_default()
                        .insert(blocker.syntax);
                }
                for ((namespace, name), ids) in &members.candidates {
                    let candidate_bytes = scope_bytes
                        .checked_add(name.capacity())
                        .and_then(|n| n.checked_add(128))
                        .and_then(|n| n.checked_mul(ids.len()))
                        .ok_or_else(|| fault("namespace candidate size overflow"))?;
                    bytes = bytes
                        .checked_add(candidate_bytes)
                        .ok_or_else(|| fault("namespace candidate metadata limit"))?;
                    for id in ids {
                        let item = definitions
                            .get(id)
                            .ok_or_else(|| fault("namespace candidate header is absent"))?;
                        if item.scope != *scope {
                            return Err(fault("namespace candidate belongs to another scope"));
                        }
                        let mut path = scope
                            .module_path
                            .iter()
                            .map(String::as_str)
                            .collect::<Vec<_>>();
                        path.push(name);
                        graph.define(
                            Self::path(scope.crate_key, &path, outline.profile.edition)?,
                            NameDefinition {
                                namespace: *namespace,
                                identity: *id,
                                defining_unit: scope.crate_key,
                                definition_edition: outline.profile.edition,
                            },
                            Self::item_visibility(item, *namespace, outline.profile.edition)?,
                        )?;
                    }
                }
            }
            for import in &outline.imports {
                imports = imports
                    .checked_add(1)
                    .ok_or_else(|| fault("namespace import count limit"))?;
                bytes = import
                    .scope
                    .module_path
                    .iter()
                    .try_fold(bytes, |n, name| n.checked_add(name.capacity()))
                    .and_then(|n| n.checked_add(128))
                    .ok_or_else(|| fault("namespace import frontier metadata limit"))?;
                if import.scope.owner.is_none() {
                    if graph
                        .imports
                        .insert(
                            Self::import_identity(import),
                            (import.clone(), outline.profile.edition),
                        )
                        .is_some()
                    {
                        return Err(fault("duplicate source import identity"));
                    }
                    if matches!(
                        import.kind,
                        crate::syntax::imports::ImportKind::Named { .. }
                            | crate::syntax::imports::ImportKind::Anonymous
                    ) {
                        graph.named_import_ids.insert(Self::import_identity(import));
                    }
                    graph
                        .frontiers
                        .entry(import.scope.clone())
                        .or_default()
                        .insert(Self::import_identity(import));
                }
            }
        }
        for outline in outlines {
            let key = Self::source_key(outline);
            bytes = key
                .module_path
                .iter()
                .try_fold(bytes, |n, name| {
                    n.checked_add(name.capacity())
                        .and_then(|n| n.checked_add(std::mem::size_of::<String>()))
                })
                .and_then(|n| {
                    n.checked_add(
                        std::mem::size_of::<crate::db::key::OutlineKey>()
                            + std::mem::size_of::<Digest>(),
                    )
                })
                .ok_or_else(|| fault("namespace source proof metadata limit"))?;
            if graph
                .source_proofs
                .insert(key, Self::source_proof(outline))
                .is_some()
            {
                return Err(fault("duplicate namespace source input"));
            }
        }
        graph.ordinary_source_bytes = bytes;
        Ok(())
    }
    pub(crate) fn compiler_dependency(
        &mut self,
        consumer: Digest,
        alias: MacroName,
        provider: Digest,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
            self.table_mut(namespace)
                .dependency(consumer, alias.clone(), provider)?;
        }
        Ok(())
    }
    pub(crate) fn install_compiler_injection(
        &mut self,
        catalog: &super::compile_input_catalog::CompileInputCatalog,
        source: &crate::source::changes::PreparedSourceView,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
            catalog.install_injected_import(self.table_mut(namespace), source)?;
        }
        Ok(())
    }
    /// rustc's standard prelude for a crate whose root source receives the
    /// compiler-injected `std`/`core` import: the owning edition's
    /// `prelude::rust_20xx` module of that injected crate. Unqualified Type
    /// and Value lookups fall back to it after module items, imports and the
    /// extern prelude, outside `#[no_implicit_prelude]` modules.
    pub(crate) fn install_standard_prelude(
        &mut self,
        catalog: &super::compile_input_catalog::CompileInputCatalog,
        source: &crate::source::changes::PreparedSourceView,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        let Some(import) = catalog.injected_import(source)? else {
            return Ok(());
        };
        let (consumer, alias, provider) = import.parts();
        // The injected crate must be the actual captured dependency.
        if !self
            .types
            .dependency_bindings()
            .any(|((unit, name), target)| *unit == consumer && name == alias && *target == provider)
        {
            return Ok(());
        }
        let version = match source.outline().profile.edition {
            2015 => "rust_2015",
            2018 => "rust_2018",
            2021 => "rust_2021",
            2024 => "rust_2024",
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-TOOLCHAIN-CAPABILITY",
                    "unknown standard prelude edition".into(),
                ));
            }
        };
        let scope = ScopeKey {
            crate_key: provider,
            module_path: vec!["prelude".into(), version.into()],
            owner: None,
        };
        // Only an actually captured prelude module supplies names; without
        // the standard facade sources there is no prelude to consult.
        if self
            .declared_module_access(&scope, source.outline().profile.edition)
            .is_err()
        {
            return Ok(());
        }
        if self
            .standard_preludes
            .get(&consumer)
            .is_some_and(|old| *old != scope)
        {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-SCOPE",
                "one crate has conflicting standard preludes".into(),
            ));
        }
        self.standard_preludes.insert(consumer, scope);
        Ok(())
    }
    pub(crate) fn disable_compiler_prelude(&mut self, scope: ScopeKey) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
            self.table_mut(namespace).disable_implicit(scope.clone())?;
        }
        Ok(())
    }
    /// Cargo-selected dependency rename, supplied by the project unit graph.
    pub fn dependency(
        &mut self,
        consumer: Digest,
        alias: &str,
        provider: Digest,
        edition: u16,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        let name = MacroName::parse(alias, edition)?;
        let path = NamePath::new(consumer, vec![name.clone()])?;
        let target = super::module_paths::ModulePath {
            unit: provider,
            names: vec![],
        };
        let visibility = VisibilityScope::Within {
            unit: consumer,
            module_path: vec![],
        };
        let install = !self
            .types
            .module_paths
            .has_strong(&super::module_paths::ModulePath {
                unit: consumer,
                names: vec![name.clone()],
            });
        for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
            self.table_mut(namespace)
                .dependency(consumer, name.clone(), provider)?;
            // A disclosed local module shadows the extern prelude name.
            if !self
                .table(namespace)
                .module_paths
                .has_strong(&super::module_paths::ModulePath {
                    unit: consumer,
                    names: vec![name.clone()],
                })
            {
                self.table_mut(namespace).module_paths.alias(
                    super::module_paths::ModulePath {
                        unit: consumer,
                        names: vec![name.clone()],
                    },
                    target.clone(),
                    visibility.clone(),
                )?;
            }
        }
        if install {
            self.module_aliases.entry(path).or_insert(target);
        }
        Ok(())
    }
    /// Called only after the captured macro route graph validates the declaration set.
    pub(crate) fn extern_alias(
        &mut self,
        row: &super::source_macro_routes::ExternAlias,
        edition: u16,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        let Some(alias_name) = &row.name else {
            return Ok(());
        };
        let mut names = super::source_macro_routes::module_names(&row.scope.module_path, edition)?;
        if row.scope.owner.is_some() {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-SCOPE",
                "extern alias lexical path is invalid".into(),
            ));
        }
        names.try_reserve(1).map_err(|_| {
            Diagnostic::new(
                "LRR-E-SOURCE-LIMIT",
                "extern alias path allocation failed".into(),
            )
        })?;
        names.push(alias_name.clone());
        let path = NamePath::new(row.scope.crate_key, names.clone())?;
        let target = super::module_paths::ModulePath {
            unit: row.provider,
            names: vec![],
        };
        if self
            .module_aliases
            .get(&path)
            .is_some_and(|old| old != &target)
        {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-SCOPE",
                "extern alias target conflicts with an existing module alias".into(),
            ));
        }
        if !self.module_aliases.contains_key(&path) {
            let bytes = names
                .iter()
                .try_fold(256usize, |n, name| {
                    n.checked_add(name.as_str().len())?
                        .checked_add(std::mem::size_of::<MacroName>())
                })
                .ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-SOURCE-LIMIT",
                        "extern alias metadata overflow".into(),
                    )
                })?;
            self.extern_alias_bytes = self
                .extern_alias_bytes
                .checked_add(bytes)
                .ok_or_else(|| {
                    Diagnostic::new("LRR-E-SOURCE-LIMIT", "extern alias metadata budget".into())
                })?;
        }
        let prelude = names.len() == 1
            && self
                .types
                .dependency_bindings()
                .any(|((unit, name), provider)| {
                    *unit == row.scope.crate_key && name == alias_name && *provider == row.provider
                });
        let visibility = VisibilityScope::for_declaration(&row.visibility, &row.scope, edition)?;
        for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
            self.table_mut(namespace)
                .module_paths
                .explicit_extern_alias(
                    super::module_paths::ModulePath {
                        unit: row.scope.crate_key,
                        names: names.clone(),
                    },
                    target.clone(),
                    visibility.clone(),
                    prelude,
                )?;
        }
        self.module_aliases.insert(path, target);
        Ok(())
    }
    pub fn declare_module(
        &mut self,
        unit: Digest,
        components: &[&str],
        edition: u16,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        let names = components
            .iter()
            .map(|s| owned_name(s, edition))
            .collect::<Result<Vec<_>, _>>()?;
        super::module_paths::ModulePath::validate_components(&names)?;
        let path = super::module_paths::ModulePath { unit, names };
        for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
            self.table_mut(namespace)
                .module_paths
                .declare(path.clone(), visibility.clone())?;
        }
        Ok(())
    }
    fn table(&self, namespace: Namespace) -> &MacroNamespaces {
        match namespace {
            Namespace::Value => &self.values,
            Namespace::Type => &self.types,
            Namespace::Macro => &self.macros,
        }
    }
    fn table_mut(&mut self, namespace: Namespace) -> &mut MacroNamespaces {
        self.import_generation = self.import_generation.wrapping_add(1);
        match namespace {
            Namespace::Value => &mut self.values,
            Namespace::Type => &mut self.types,
            Namespace::Macro => &mut self.macros,
        }
    }
    pub fn define(
        &mut self,
        path: NamePath,
        definition: NameDefinition,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        self.define_in_class(path, definition, visibility, NameBindingClass::Root)
    }
    fn define_in_class(
        &mut self,
        path: NamePath,
        definition: NameDefinition,
        visibility: VisibilityScope,
        class: NameBindingClass,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        let namespace = definition.namespace;
        let identity = definition.identity;
        self.table_mut(definition.namespace)
            .define_visible_in_class(
                path,
                MacroDefinition {
                    identity: definition.identity,
                    defining_unit: definition.defining_unit,
                    definition_edition: definition.definition_edition,
                },
                visibility.clone(),
                None,
                class,
            )?;
        self.visibility.insert((namespace, identity), visibility);
        Ok(())
    }
    /// Source facts only. Current use/scope publication is a separate owner.
    pub(crate) fn declared_item_name(
        &self,
        namespace: Namespace,
        identity: Digest,
    ) -> Option<&Arc<DeclaredItemName>> {
        self.declared_names.get(&(namespace, identity))
    }
    /// Cold publication inventory only; installed lookup uses its relevant
    /// class/name bucket rather than traversing the complete graph.
    pub(crate) fn declared_item_names(&self) -> impl Iterator<Item = &Arc<DeclaredItemName>> {
        self.declared_names.values()
    }
    pub(crate) fn define_declared_item(
        &mut self,
        declaration: Arc<DeclaredItemName>,
        module: bool,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        // ModulePaths currently certifies root-class prefixes. Retaining a
        // declaration in another class does not certify descendants there.
        if module && declaration.class() != NameBindingClass::Root {
            return Err(Diagnostic::new(
                "LRR-E-NAME-CONTEXT",
                "declared module requires its actual contextual module-prefix receipt".into(),
            ));
        }
        let key = (
            declaration.definition().namespace,
            declaration.definition().identity,
        );
        if let Some(old) = self.declared_names.get(&key) {
            return if old.fingerprint() == declaration.fingerprint() {
                Ok(())
            } else {
                Err(Diagnostic::new(
                    "LRR-E-EXPANSION-CONFLICT",
                    "declared name occurrence differs".into(),
                ))
            };
        }
        let bytes = self
            .declared_name_bytes
            .checked_add(declaration.metadata_bytes()?)
            .and_then(|n| n.checked_add(96))
            .ok_or_else(crate::runtime::arena::resource)?;
        let scope = declaration.scope();
        let mut parts = scope
            .module_path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        parts.push(declaration.name());
        let path = Self::path(
            scope.crate_key,
            &parts,
            declaration.definition().definition_edition,
        )?;
        if module && declaration.class() == NameBindingClass::Root {
            self.define_generated_module(
                path,
                declaration.definition().clone(),
                declaration.visibility().clone(),
            )?;
        } else {
            self.define_in_class(
                path,
                declaration.definition().clone(),
                declaration.visibility().clone(),
                declaration.class(),
            )?;
        }
        self.declared_names.insert(key, declaration);
        self.declared_name_bytes = bytes;
        Ok(())
    }
    /// Register an admitted inline declaration container with its canonical
    /// generated identity; module aliases retain that identity through imports.
    pub(crate) fn define_generated_module(
        &mut self,
        path: NamePath,
        definition: NameDefinition,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        if definition.namespace != Namespace::Type || definition.defining_unit != path.unit {
            return Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "generated module namespace or unit differs".into(),
            ));
        }
        let mut components = Vec::new();
        components
            .try_reserve_exact(path.components().len())
            .map_err(|_| {
                Diagnostic::new(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated module path allocation".into(),
                )
            })?;
        components.extend(path.components().iter().map(MacroName::as_str));
        self.declare_module(
            path.unit,
            &components,
            definition.definition_edition,
            visibility.clone(),
        )?;
        let identity = definition.identity;
        self.define(path, definition, visibility)?;
        self.modules.insert(identity);
        Ok(())
    }
    pub fn named_import(
        &mut self,
        namespace: Namespace,
        path: NamePath,
        target: NamePath,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        self.table_mut(namespace)
            .reexport_visible(path.clone(), target.clone(), visibility)?;
        self.links
            .entry((namespace, path))
            .or_default()
            .insert(target);
        Ok(())
    }
    /// Exact resolved macro facet; value/type facets still run through ordinary imports.
    pub(crate) fn macro_import_facet(
        &mut self,
        import: &crate::syntax::imports::SourceImport,
        definition: NameDefinition,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        let id = Self::import_identity(import);
        if definition.namespace != Namespace::Macro
            || self.imports.get(&id).map(|r| &r.0) != Some(import)
        {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-SCOPE",
                "macro import facet has foreign identity".into(),
            ));
        }
        if self
            .macro_import_facets
            .get(&id)
            .is_some_and(|old| old != &definition)
        {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-SCOPE",
                "macro import facet changed in one discovery".into(),
            ));
        }
        self.macro_import_facets.insert(id, definition);
        Ok(())
    }
    fn retain_named_module_route(
        &mut self,
        destination: NamePath,
        target: super::module_paths::ModulePath,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        if self
            .named_module_links
            .get(&destination)
            .is_some_and(|old| old.contains(&target))
        {
            return Ok(());
        }
        let fail = || {
            Diagnostic::new(
                "LRR-E-SOURCE-LIMIT",
                "ordinary named module route budget".into(),
            )
        };
        let routes = self
            .named_module_routes
            .checked_add(1)
            .ok_or_else(fail)?;
        let added = [destination.components(), &target.names]
            .into_iter()
            .flatten()
            .try_fold(256usize, |n, name| {
                n.checked_add(std::mem::size_of::<MacroName>())?
                    .checked_add(name.as_str().len())
            })
            .ok_or_else(fail)?;
        let bytes = self
            .named_module_bytes
            .checked_add(added)
            .ok_or_else(fail)?;
        self.named_module_links
            .entry(destination)
            .or_default()
            .insert(target);
        self.named_module_routes = routes;
        self.named_module_bytes = bytes;
        Ok(())
    }
    /// Resolve named item and disclosed module imports. Other forms retain a frontier.
    pub fn resolve_named_imports(&mut self) -> Result<ImportResolution, Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        let _span = crate::core::timing::Span::enter(|| "names resolve_named_imports".into());
        use crate::syntax::imports::ImportKind;
        self.anonymous.clear();
        let mut anonymous_bytes = 0usize;
        let rows = self
            .imports
            .iter()
            .map(|(id, (import, edition))| (*id, import.clone(), *edition))
            .collect::<Vec<_>>();
        let mut routed = std::collections::BTreeMap::new();
        let mut root_imports = std::collections::BTreeSet::new();
        for (identity, import, edition) in &rows {
            let ImportKind::Named { alias } = &import.kind else {
                continue;
            };
            if let Some(root) = self.types.import_module_route(
                &import.path,
                import.absolute,
                &import.scope,
                *edition,
            )? && root.names.is_empty()
            {
                let Some(alias) = alias else {
                    return Err(Diagnostic::new(
                        "LRR-E-MACRO-SCOPE",
                        "root import requires an alias".into(),
                    ));
                };
                let mut names = import
                    .scope
                    .module_path
                    .iter()
                    .map(|n| owned_name(n, *edition))
                    .collect::<Result<Vec<_>, _>>()?;
                names.push(MacroName::parse(alias, *edition)?);
                let destination = NamePath::new(import.scope.crate_key, names.clone())?;
                if matches!(
                    self.types
                        .module_paths
                        .resolve_module(&root, Some(&import.scope))?,
                    super::module_paths::ModuleResolution::Known { .. }
                ) {
                    let visibility = VisibilityScope::for_declaration(
                        &import.visibility,
                        &import.scope,
                        *edition,
                    )?;
                    for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
                        self.table_mut(namespace).module_paths.imported_alias(
                            super::module_paths::ModulePath {
                                unit: destination.unit,
                                names: names.clone(),
                            },
                            root.clone(),
                            visibility.clone(),
                        )?;
                    }
                    self.retain_named_module_route(destination, root)?;
                    root_imports.insert(*identity);
                }
                continue;
            }
            // `use a::b::{self}` names module `a::b`: it binds the type
            // namespace and module alias only, never a value or macro.
            let self_leaf = import.path.len() > 1
                && import.path.last() == Some(&crate::syntax::imports::ImportSegment::SelfModule);
            let target = if self_leaf {
                let Some(module) = self.types.import_module_route(
                    &import.path,
                    import.absolute,
                    &import.scope,
                    *edition,
                )?
                else {
                    continue;
                };
                NamePath::new(module.unit, module.names)?
            } else {
                let Some(target) = self.values.import_segments_path(
                    &import.path,
                    import.absolute,
                    &import.scope,
                    *edition,
                )?
                else {
                    continue;
                };
                target
            };
            let name = match alias {
                Some(name) => MacroName::parse(name, *edition)?,
                None => target.components().last().cloned().ok_or(Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "empty import target".into(),
                ))?,
            };
            let mut names = import
                .scope
                .module_path
                .iter()
                .map(|n| owned_name(n, *edition))
                .collect::<Result<Vec<_>, _>>()?;
            names.push(name);
            let destination = NamePath::new(import.scope.crate_key, names)?;
            let visibility =
                VisibilityScope::for_declaration(&import.visibility, &import.scope, *edition)?;
            let namespaces: &[Namespace] = if self_leaf {
                &[Namespace::Type]
            } else {
                &[Namespace::Value, Namespace::Type, Namespace::Macro]
            };
            for namespace in namespaces {
                self.named_import(
                    *namespace,
                    destination.clone(),
                    target.clone(),
                    visibility.clone(),
                )?;
            }
            routed.insert(*identity, (target, destination, visibility));
        }
        // Install only disclosed module targets. Keep the original target path so
        // the existing registry checks intermediate aliases and re-export widths.
        self.deferred_imports.clear();
        let mut installed = std::collections::BTreeSet::new();
        let mut module_denied = std::collections::BTreeSet::new();
        let mut root_aliases = std::collections::BTreeSet::new();
        let mut steps = 0usize;
        loop {
            let before = installed.len();
            for (identity, import, _) in &rows {
                if installed.contains(identity) {
                    continue;
                }
                let Some((target, destination, visibility)) = routed.get(identity) else {
                    continue;
                };
                steps = steps
                    .checked_add(1)
                    .ok_or(Diagnostic::new(
                        "LRR-E-SOURCE-LIMIT",
                        "ordinary module alias work limit".into(),
                    ))?;
                if self
                    .producer_frontier(Namespace::Type, target, true)?
                    .is_some()
                {
                    continue;
                }
                let target_module = super::module_paths::ModulePath {
                    unit: target.unit,
                    names: target.components().to_vec(),
                };
                let module = self
                    .types
                    .module_paths
                    .resolve_module(&target_module, Some(&import.scope))?;
                let permitted = match module {
                    super::module_paths::ModuleResolution::Known {
                        visibility: access,
                        path: selected,
                    } => {
                        if access.permits_reexport_as(visibility) {
                            if selected.names.is_empty() {
                                root_aliases.insert(*identity);
                            }
                            true
                        } else {
                            module_denied.insert(*identity);
                            false
                        }
                    }
                    super::module_paths::ModuleResolution::Denied(_) => {
                        module_denied.insert(*identity);
                        false
                    }
                    _ => false,
                };
                if permitted {
                    let destination_module = super::module_paths::ModulePath {
                        unit: destination.unit,
                        names: destination.components().to_vec(),
                    };
                    for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
                        self.table_mut(namespace).module_paths.imported_alias(
                            destination_module.clone(),
                            target_module.clone(),
                            visibility.clone(),
                        )?;
                    }
                    self.retain_named_module_route(destination.clone(), target_module)?;
                    installed.insert(*identity);
                }
            }
            if installed.len() == before {
                break;
            }
        }
        for (identity, import, _) in &rows {
            if !installed.contains(identity)
                && !module_denied.contains(identity)
                && let Some((target, _, _)) = routed.get(identity)
                && self.ambiguous_prefix(target, &import.scope)?.is_none()
                && let PathLookup::Defined(definition) =
                    self.types.lookup_from(target, &import.scope)?
                && self.modules.contains(&definition.identity)
            {
                self.deferred_imports.insert(*identity);
            }
        }
        let mut result = ImportResolution::default();
        for (identity, import, edition) in rows {
            if import.kind == ImportKind::Anonymous {
                match self.resolve_anonymous_import(&import, edition)? {
                    AnonymousImportOutcome::Ready(record) => {
                        anonymous_bytes = anonymous_bytes
                            .checked_add(record.estimated_bytes()?)
                            .ok_or(Diagnostic::new(
                                "LRR-E-SOURCE-LIMIT",
                                "anonymous import metadata limit".into(),
                            ))?;
                        self.anonymous.insert(identity, record);
                        result.resolved.push(identity);
                        if let Some(frontier) = self.frontiers.get_mut(&import.scope) {
                            frontier.remove(&identity);
                        }
                    }
                    AnonymousImportOutcome::Pending(_) => result.pending.push(identity),
                    AnonymousImportOutcome::Missing => result.missing.push(identity),
                    AnonymousImportOutcome::Inaccessible => result.inaccessible.push(identity),
                    AnonymousImportOutcome::Ambiguous => result.ambiguous.push(identity),
                }
                continue;
            }
            if root_imports.contains(&identity) {
                result.resolved.push(identity);
                if let Some(frontier) = self.frontiers.get_mut(&import.scope) {
                    frontier.remove(&identity);
                }
                continue;
            }
            let Some((target, destination, visibility)) = routed.get(&identity) else {
                result.pending.push(identity);
                continue;
            };
            if self.ambiguous_prefix(target, &import.scope)?.is_some() {
                result.ambiguous.push(identity);
                continue;
            }
            let mut ready = root_aliases.contains(&identity);
            let mut pending = false;
            let mut denied = module_denied.contains(&identity);
            let mut ambiguous = false;
            let mut settled_destination_ambiguity = false;
            for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
                if namespace == Namespace::Macro && self.macro_import_facets.contains_key(&identity)
                {
                    ready = true;
                    continue;
                }
                if self.producer_frontier(namespace, target, true)?.is_some() {
                    pending = true;
                    continue;
                }

                match self.table(namespace).lookup_from(target, &import.scope)? {
                    PathLookup::Defined(definition) => {
                        if self.modules.contains(&definition.identity)
                            && !installed.contains(&identity)
                        {
                            if module_denied.contains(&identity) {
                                denied = true;
                            } else {
                                pending = true;
                            }
                        } else if self
                            .visibility
                            .get(&(namespace, definition.identity))
                            .is_some_and(|v| !v.permits_reexport_as(visibility))
                        {
                            denied = true;
                        } else {
                            match self
                                .table(namespace)
                                .lookup_from(destination, &import.scope)?
                            {
                                PathLookup::Defined(_) => ready = true,
                                PathLookup::Inaccessible(_) => denied = true,
                                PathLookup::Ambiguous(_) => {
                                    ambiguous = true;
                                    settled_destination_ambiguity = true;
                                }
                                PathLookup::NeedInput(_) => pending = true,
                                PathLookup::Absent => pending = true,
                            }
                        }
                    }
                    PathLookup::Ambiguous(_) => ambiguous = true,
                    PathLookup::Inaccessible(_) => denied = true,
                    PathLookup::NeedInput(_) => pending = true,
                    PathLookup::Absent => {}
                }
            }

            if pending {
                result.pending.push(identity);
            } else if ambiguous {
                result.ambiguous.push(identity);
                // Only a completed collision of individually selected current
                // source imports discharges the destination producer. Unsettled
                // target ambiguity and unknown producers retain their frontier.
                if settled_destination_ambiguity
                    && let Some(frontier) = self.frontiers.get_mut(&import.scope)
                {
                    frontier.remove(&identity);
                }
            } else if denied && !ready {
                result.inaccessible.push(identity);
            } else if !ready {
                result.missing.push(identity);
            } else {
                result.resolved.push(identity);
                if let Some(frontier) = self.frontiers.get_mut(&import.scope) {
                    frontier.remove(&identity);
                }
            }
        }
        Ok(result)
    }
    /// Ordinary import scheduling over the same owned named/glob engine.
    /// Unrouted glob modules and open source scopes retain their producer frontier.
    pub fn disclosed_anonymous_imports<'a>(
        &'a self,
        scope: &'a ScopeKey,
    ) -> impl Iterator<Item = &'a AnonymousImport> {
        self.anonymous
            .values()
            .filter(move |record| &record.scope == scope)
    }
    fn resolve_anonymous_import(
        &self,
        import: &crate::syntax::imports::SourceImport,
        edition: u16,
    ) -> Result<AnonymousImportOutcome, Diagnostic> {
        let mut candidates = AnonymousImportCandidates::default();
        if let Some(root) =
            self.types
                .import_module_route(&import.path, import.absolute, &import.scope, edition)?
            && root.names.is_empty()
        {
            match self
                .types
                .module_paths
                .resolve_module(&root, Some(&import.scope))?
            {
                super::module_paths::ModuleResolution::Known { .. } => {
                    let source_scope = ScopeKey {
                        crate_key: root.unit,
                        module_path: vec![],
                        owner: None,
                    };
                    if let Some(id) = self
                        .frontiers
                        .get(&source_scope)
                        .and_then(|ids| ids.iter().find(|id| !self.imports.contains_key(id)))
                    {
                        candidates.observe(NameLookup::NeedInput(*id))?;
                    } else {
                        candidates.observe(NameLookup::ModuleRoot(root.unit))?;
                    }
                }
                _ => return Ok(AnonymousImportOutcome::Missing),
            }
        } else if let Some(target) = self.values.import_segments_path(
            &import.path,
            import.absolute,
            &import.scope,
            edition,
        )? {
            if self.ambiguous_prefix(&target, &import.scope)?.is_some() {
                return Ok(AnonymousImportOutcome::Ambiguous);
            }
            for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
                let found = if let Some(id) = self.producer_frontier(namespace, &target, true)? {
                    NameLookup::NeedInput(id)
                } else {
                    match self.table(namespace).lookup_from(&target, &import.scope)? {
                        PathLookup::Defined(d) => NameLookup::Defined(NameDefinition {
                            namespace,
                            identity: d.identity,
                            defining_unit: d.defining_unit,
                            definition_edition: d.definition_edition,
                        }),
                        PathLookup::Absent => NameLookup::Absent,
                        PathLookup::NeedInput(id) => NameLookup::NeedInput(id),
                        PathLookup::Inaccessible(paths) => NameLookup::Inaccessible(paths),
                        PathLookup::Ambiguous(_) => return Ok(AnonymousImportOutcome::Ambiguous),
                    }
                };
                candidates.observe(found)?;
            }
        } else {
            return Ok(AnonymousImportOutcome::Pending(Self::import_identity(
                import,
            )));
        }
        Ok(candidates.finish(import))
    }
    fn import_state(&self) -> Digest {
        fn path(e: &mut StableEncoder, path: &NamePath) {
            e.digest(path.unit);
            e.sequence(path.components(), |e, n| e.string(n.as_str()));
        }
        let mut e = StableEncoder::new("lrr/v4/ordinary-import-convergence");
        for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
            e.u32(namespace.tag());
            self.table(namespace).encode_compiler_imports(&mut e);
        }
        e.map(&self.module_frontier_scopes, |e, id, child| {
            e.digest(*id);
            e.record(child);
        });
        e.sequence(
            &self.source_glob_items.iter().collect::<Vec<_>>(),
            |e, (n, from, to, access)| {
                e.u32(n.tag());
                path(e, from);
                path(e, to);
                e.record(access);
            },
        );
        e.sequence(
            &self.source_glob_modules.iter().collect::<Vec<_>>(),
            |e, (from, to, access)| {
                for p in [from, to] {
                    e.digest(p.unit);
                    e.sequence(&p.names, |e, n| e.string(n.as_str()));
                }
                e.record(access);
            },
        );
        e.sequence(
            &self.module_aliases.iter().collect::<Vec<_>>(),
            |e, (from, to)| {
                path(e, from);
                e.digest(to.unit);
                e.sequence(&to.names, |e, n| e.string(n.as_str()));
            },
        );
        e.sequence(
            &self.named_module_links.iter().collect::<Vec<_>>(),
            |e, (from, targets)| {
                path(e, from);
                e.sequence(&targets.iter().collect::<Vec<_>>(), |e, to| {
                    e.digest(to.unit);
                    e.sequence(&to.names, |e, n| e.string(n.as_str()));
                });
            },
        );
        e.sequence(
            &self.frontiers.iter().collect::<Vec<_>>(),
            |e, (scope, ids)| {
                e.record(*scope);
                e.sequence(&ids.iter().collect::<Vec<_>>(), |e, id| e.digest(**id));
            },
        );
        e.sequence(
            &self.deferred_imports.iter().collect::<Vec<_>>(),
            |e, id| e.digest(**id),
        );
        e.sequence(&self.anonymous.values().collect::<Vec<_>>(), |e, record| {
            e.record(*record)
        });
        e.sequence(
            &self.macro_import_facets.iter().collect::<Vec<_>>(),
            |e, (id, d)| {
                e.digest(**id);
                e.record(*d);
            },
        );
        e.finish()
    }
    /// Advances with every graph mutation: equal generations of one graph
    /// denote the same graph state.
    pub(crate) fn mutation_generation(&self) -> u64 {
        self.import_generation
    }
    pub(crate) fn route_install_key(&self) -> Option<Digest> {
        self.route_install_key
    }
    /// Records the key of graph facts the caller just installed; not a graph
    /// mutation itself (it never changes a lookup).
    pub(crate) fn set_route_install_key(&mut self, key: Digest) {
        self.route_install_key = Some(key);
    }
    pub fn resolve_imports(&mut self) -> Result<ImportResolution, Diagnostic> {
        if let Some((generation, result)) = &self.import_fixed_point
            && *generation == self.import_generation
        {
            return Ok(result.clone());
        }
        let result = self.resolve_imports_fixed_point()?;
        self.import_fixed_point = Some((self.import_generation, result.clone()));
        Ok(result)
    }
    fn resolve_imports_fixed_point(&mut self) -> Result<ImportResolution, Diagnostic> {
        let _span = crate::core::timing::Span::enter(|| "names resolve_imports".into());
        let mut work = 0usize;
        loop {
            let before = self.import_state();
            work = work
                .checked_add(self.imports.len().max(1))
                .ok_or(Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "ordinary import convergence work overflow".into(),
                ))?;
            // Re-establish demands before classifying the rebuilt graph. A prior
            // provisional success cannot discharge a now-ambiguous glob source.
            for (id, (import, _)) in &self.imports {
                self.frontiers
                    .entry(import.scope.clone())
                    .or_default()
                    .insert(*id);
            }
            let result = self.resolve_imports_round(&mut work)?;
            if self.import_state() == before {
                return Ok(result);
            }
        }
    }
    fn resolve_imports_round(&mut self, work: &mut usize) -> Result<ImportResolution, Diagnostic> {
        use super::module_paths::ModuleResolution;
        use crate::syntax::imports::ImportKind;
        // Publish symbolic named edges and disclosed module aliases before
        // taking the finite member-name universe. Unknown macro frontiers still
        // block this phase; glob outcomes are verified below before discharge.
        self.candidate_globs = self
            .imports
            .iter()
            .filter(|(_, (i, _))| i.kind == ImportKind::Glob)
            .map(|(id, _)| *id)
            .collect();
        let no_globs = self.candidate_globs.is_empty()
            && self.source_glob_items.is_empty()
            && self.source_glob_modules.is_empty()
            && self.glob_module_links.is_empty();
        let named = self.resolve_named_imports();
        self.candidate_globs.clear();
        let named = named?;
        // Without any glob import or previously published glob edge, the glob
        // phase below changes nothing and the two later named passes would
        // repeat this one on identical state.
        if no_globs {
            let mut result = named;
            for ids in [
                &mut result.resolved,
                &mut result.pending,
                &mut result.missing,
                &mut result.inaccessible,
                &mut result.ambiguous,
            ] {
                ids.sort();
            }
            return Ok(result);
        }
        let rows = self
            .imports
            .iter()
            .filter(|(_, (i, _))| i.kind == ImportKind::Glob)
            .map(|(id, (i, edition))| (*id, i.clone(), *edition))
            .collect::<Vec<_>>();
        // The global member-name universe is the conservative candidate set;
        // most globs instead enumerate their source module's own members.
        let mut universe = None::<std::collections::BTreeSet<MacroName>>;
        let mut routed = std::collections::BTreeMap::new();
        let mut globs = ImportResolution::default();
        let mut next_items = std::collections::BTreeSet::new();
        let mut next_modules = std::collections::BTreeSet::new();
        let mut edges = 0usize;
        let mut bytes = 0usize;
        for (identity, import, edition) in &rows {
            let Some(reference) = self.types.import_module_route(
                &import.path,
                import.absolute,
                &import.scope,
                *edition,
            )?
            else {
                globs.pending.push(*identity);
                continue;
            };
            let selected = match self
                .types
                .module_paths
                .resolve_module(&reference, Some(&import.scope))?
            {
                ModuleResolution::Known { path, .. } => path,
                ModuleResolution::Absent => {
                    let open = (0..=reference.names.len()).any(|length| {
                        let prefix = ScopeKey {
                            crate_key: reference.unit,
                            module_path: reference.names[..length]
                                .iter()
                                .map(|n| n.as_str().into())
                                .collect(),
                            owner: None,
                        };
                        self.frontiers
                            .get(&prefix)
                            .is_some_and(|ids| ids.iter().any(|id| id != identity))
                    });
                    if open {
                        globs.pending.push(*identity);
                    } else {
                        globs.missing.push(*identity);
                    }
                    continue;
                }
                ModuleResolution::Denied(_) => {
                    globs.inaccessible.push(*identity);
                    continue;
                }
                ModuleResolution::Ambiguous(_) => {
                    globs.ambiguous.push(*identity);
                    continue;
                }
            };
            let visibility =
                VisibilityScope::for_declaration(&import.visibility, &import.scope, *edition)?;
            let scope = ScopeKey {
                crate_key: selected.unit,
                module_path: selected.names.iter().map(|n| n.as_str().into()).collect(),
                owner: None,
            };
            let scoped = self.glob_source_member_names(&scope, *edition);
            let names = match &scoped {
                Some(names) => names,
                None => universe.get_or_insert_with(|| {
                    [Namespace::Value, Namespace::Type, Namespace::Macro]
                        .into_iter()
                        .flat_map(|n| self.table(n).member_names())
                        .collect()
                }),
            };
            routed.insert(*identity, scope);
            for name in names {
                let mut target = selected.names.clone();
                target.push(name.clone());
                let target = NamePath::new(selected.unit, target)?;
                let mut destination = import
                    .scope
                    .module_path
                    .iter()
                    .map(|n| owned_name(n, *edition))
                    .collect::<Result<Vec<_>, _>>()?;
                destination.push(name.clone());
                let destination = NamePath::new(import.scope.crate_key, destination)?;
                let source_module = super::module_paths::ModulePath {
                    unit: target.unit,
                    names: target.components().to_vec(),
                };
                for (module, access) in self
                    .types
                    .module_paths
                    .member_candidates(&source_module, &import.scope)?
                {
                    let Some(range) = super::glob_lookup::intersection(&visibility, &access) else {
                        continue;
                    };
                    let destination_module = super::module_paths::ModulePath {
                        unit: destination.unit,
                        names: destination.components().to_vec(),
                    };
                    next_modules.insert((destination_module, module, range));
                }
                for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
                    edges =
                        edges
                            .checked_add(1)
                            .ok_or(Diagnostic::new(
                                "LRR-E-SOURCE-LIMIT",
                                "ordinary glob edge limit".into(),
                            ))?;
                    bytes = bytes.checked_add(256).ok_or(Diagnostic::new(
                        "LRR-E-SOURCE-LIMIT",
                        "ordinary glob metadata overflow".into(),
                    ))?;
                    bytes = [target.components(), destination.components()]
                        .into_iter()
                        .flatten()
                        .try_fold(bytes, |n, name| {
                            n.checked_add(name.as_str().len())
                                .and_then(|n| n.checked_add(std::mem::size_of::<MacroName>()))
                        })
                        .ok_or(Diagnostic::new(
                            "LRR-E-SOURCE-LIMIT",
                            "ordinary glob metadata limit".into(),
                        ))?;
                    next_items.insert((
                        namespace,
                        destination.clone(),
                        target.clone(),
                        visibility.clone(),
                    ));
                }
            }
        }
        *work = work
            .checked_add(edges)
            .ok_or(Diagnostic::new(
                "LRR-E-SOURCE-LIMIT",
                "ordinary glob convergence work limit".into(),
            ))?;
        for (namespace, destination, target, visibility) in self
            .source_glob_items
            .difference(&next_items)
            .cloned()
            .collect::<Vec<_>>()
        {
            if !self.manual_globs.contains(&(
                namespace,
                destination.clone(),
                target.clone(),
                visibility.clone(),
            )) {
                self.table_mut(namespace)
                    .remove_glob_member(&destination, &target, &visibility)?;
            }
            if !self.table(namespace).has_import_edge(&destination, &target)
                && let Some(targets) = self.links.get_mut(&(namespace, destination))
            {
                targets.remove(&target);
            }
        }
        for (namespace, destination, target, visibility) in &next_items {
            self.table_mut(*namespace).glob_member(
                destination.clone(),
                target.clone(),
                visibility.clone(),
            )?;
            self.links
                .entry((*namespace, destination.clone()))
                .or_default()
                .insert(target.clone());
        }
        for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
            self.table_mut(namespace).module_paths.clear_globs()?;
        }
        self.glob_module_links.clear();
        for (destination, target, visibility) in &next_modules {
            for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
                self.table_mut(namespace).module_paths.glob(
                    destination.clone(),
                    target.clone(),
                    visibility.clone(),
                )?;
            }
            self.glob_module_links
                .entry(NamePath::new(destination.unit, destination.names.clone())?)
                .or_default()
                .insert(NamePath::new(target.unit, target.names.clone())?);
        }
        self.source_glob_items = next_items;
        self.source_glob_modules = next_modules;
        self.candidate_globs = routed.keys().copied().collect();
        let named = self.resolve_named_imports();
        self.candidate_globs.clear();
        named?;
        // Close disclosed glob cycles only when every reachable source scope is
        // free of unknown producers and unresolved non-glob imports. A cycle
        // itself adds no new spellings beyond the finite source name universe.
        for (identity, target_scope) in &routed {
            let mut stack = vec![target_scope.clone()];
            let mut seen = std::collections::BTreeSet::new();
            let mut open = false;
            while let Some(scope) = stack.pop() {
                if !seen.insert(scope.clone()) {
                    continue;
                }
                for length in 0..=scope.module_path.len() {
                    let prefix = ScopeKey {
                        crate_key: scope.crate_key,
                        module_path: scope.module_path[..length].to_vec(),
                        owner: None,
                    };
                    if let Some(frontiers) = self.frontiers.get(&prefix) {
                        for producer in frontiers {
                            if let Some(target) = routed.get(producer) {
                                stack.push(target.clone());
                            } else {
                                open = true;
                            }
                        }
                    }
                }
            }
            if open {
                globs.pending.push(*identity);
            } else {
                globs.resolved.push(*identity);
            }
        }
        for identity in &globs.resolved {
            let scope = &self
                .imports
                .get(identity)
                .expect("routed source glob")
                .0
                .scope;
            if let Some(frontier) = self.frontiers.get_mut(scope) {
                frontier.remove(identity);
            }
        }
        let mut result = self.resolve_named_imports()?;
        // Named scheduling retains unsupported forms, including globs. Replace
        // their entries with the separate glob scheduler's typed outcomes.
        result
            .pending
            .retain(|id| !rows.iter().any(|(glob, _, _)| glob == id));
        result.resolved.extend(globs.resolved);
        result.pending.extend(globs.pending);
        result.missing.extend(globs.missing);
        result.inaccessible.extend(globs.inaccessible);
        result.ambiguous.extend(globs.ambiguous);
        for ids in [
            &mut result.resolved,
            &mut result.pending,
            &mut result.missing,
            &mut result.inaccessible,
            &mut result.ambiguous,
        ] {
            ids.sort();
        }
        Ok(result)
    }
    /// Declaration access is a candidate fact; this does not certify membership or absence.
    pub(crate) fn declared_module_access(
        &self,
        scope: &ScopeKey,
        edition: u16,
    ) -> Result<VisibilityScope, Diagnostic> {
        let names = super::source_macro_routes::module_names(&scope.module_path, edition)?;
        match self
            .types
            .lookup_module_from(scope.crate_key, &names, scope)?
        {
            super::macro_names::ModuleLookup::Defined { visibility, .. } => Ok(visibility),
            _ => Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "captured module declaration access absent or ambiguous".into(),
            )),
        }
    }
    pub(crate) fn disclose_module_source_scope(
        &mut self,
        producer: Digest,
        child: ScopeKey,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        if child.owner.is_some()
            || child.module_path.is_empty()
            || !self.frontiers.values().any(|ids| ids.contains(&producer))
            || self
                .module_frontier_scopes
                .get(&producer)
                .is_some_and(|old| old != &child)
        {
            return Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "disclosed module frontier binding mismatch".into(),
            ));
        }
        if !self.module_frontier_scopes.contains_key(&producer) {
            self.ordinary_source_bytes = self
                .ordinary_source_bytes
                .checked_add(child.heap_bytes())
                .and_then(|n| n.checked_add(128))
                .ok_or(Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "disclosed module frontier metadata limit".into(),
                ))?;
        }
        self.module_frontier_scopes.insert(producer, child);
        Ok(())
    }
    pub(crate) fn disclosed_import_facet(
        &self,
        identity: Digest,
        namespace: Namespace,
    ) -> Result<bool, Diagnostic> {
        use crate::syntax::imports::ImportKind;
        let Some((import, edition)) = self.imports.get(&identity) else {
            return Ok(false);
        };
        let ImportKind::Named { alias } = &import.kind else {
            return Ok(false);
        };
        let Some(target) = self.table(namespace).import_segments_path(
            &import.path,
            import.absolute,
            &import.scope,
            *edition,
        )?
        else {
            return Ok(false);
        };
        if self.producer_frontier(namespace, &target, true)?.is_some() {
            return Ok(false);
        }
        let PathLookup::Defined(definition) =
            self.table(namespace).lookup_from(&target, &import.scope)?
        else {
            return Ok(false);
        };
        let visibility =
            VisibilityScope::for_declaration(&import.visibility, &import.scope, *edition)?;
        if self.modules.contains(&definition.identity)
            || self
                .visibility
                .get(&(namespace, definition.identity))
                .is_none_or(|access| !access.permits_reexport_as(&visibility))
        {
            return Ok(false);
        }
        let name = match alias {
            Some(alias) => owned_name(alias, *edition)?,
            None => target.components().last().cloned().ok_or(Diagnostic::new(
                "LRR-E-MACRO-SCOPE",
                "disclosed import facet member absent".into(),
            ))?,
        };
        let mut parts = import
            .scope
            .module_path
            .iter()
            .map(|name| owned_name(name, *edition))
            .collect::<Result<Vec<_>, _>>()?;
        parts.push(name);
        let destination = NamePath::new(import.scope.crate_key, parts)?;
        Ok(
            matches!(self.table(namespace).lookup_from(&destination, &import.scope)?, PathLookup::Defined(found) if found == definition),
        )
    }
    fn producer_frontier(
        &self,
        namespace: Namespace,
        path: &NamePath,
        ignore_imports: bool,
    ) -> Result<Option<Digest>, Diagnostic> {
        self.producer_frontier_observed(
            namespace,
            path,
            ignore_imports,
            &mut reads::NamespaceReads::disabled(),
        )
    }
    fn producer_frontier_observed(
        &self,
        namespace: Namespace,
        path: &NamePath,
        ignore_imports: bool,
        reads: &mut reads::NamespaceReads,
    ) -> Result<Option<Digest>, Diagnostic> {
        let mut stack = vec![path.clone()];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(path) = stack.pop() {
            if !seen.insert(path.clone()) {
                continue;
            }
            // A disclosed child can add crate-root macro exports, but cannot
            // change a validated cross-unit module alias's target exports.
            // Whether the path's parent module is a cross-unit alias. It only
            // matters for a module-source frontier, so it is resolved (and
            // its read recorded) on the first such frontier, not per path.
            let mut external_alias: Option<bool> = None;
            let mut scope = ScopeKey {
                crate_key: path.unit,
                module_path: vec![],
                owner: None,
            };
            for name in path.components() {
                reads.scope(namespace, &scope)?;
                if let Some(ids) = self.frontiers.get(&scope)
                    && external_alias.is_none()
                    && ids
                        .iter()
                        .any(|id| self.module_frontier_scopes.contains_key(id))
                {
                    let prefix = super::module_paths::ModulePath {
                        unit: path.unit,
                        names: path.components()[..path.components().len().saturating_sub(1)]
                            .to_vec(),
                    };
                    external_alias = Some(matches!(
                        self.types.module_paths.resolve_module_observed(&prefix, None, reads)?,
                        super::module_paths::ModuleResolution::Known { path: ref actual, .. } if actual.unit != path.unit
                    ));
                }
                let external_alias = external_alias.unwrap_or(false);
                if let Some(producer) = self.frontiers.get(&scope).and_then(|ids| {
                    ids.iter().find(|id| {
                        // Same answer as below, without the import-table probe.
                        if ignore_imports
                            && self.named_import_ids.contains(*id)
                            && !self.deferred_imports.contains(*id)
                        {
                            return false;
                        }
                        if (namespace != Namespace::Macro || external_alias)
                            && let Some(child) = self.module_frontier_scopes.get(id)
                        {
                            let inside = path.unit == child.crate_key
                                && path.components().len() >= child.module_path.len()
                                && path
                                    .components()
                                    .iter()
                                    .zip(&child.module_path)
                                    .all(|(name, part)| name.as_str() == part);
                            if !inside {
                                return false;
                            }
                        }
                        !ignore_imports
                            || self.deferred_imports.contains(id)
                            || (!self.candidate_globs.contains(id)
                                && !self.imports.get(id).is_some_and(|(import, _)| {
                                    matches!(
                                        import.kind,
                                        crate::syntax::imports::ImportKind::Named { .. }
                                            | crate::syntax::imports::ImportKind::Anonymous
                                    )
                                }))
                    })
                }) {
                    return Ok(Some(*producer));
                }
                scope.module_path.push(name.as_str().into());
            }
            // Prefix lookups bound work by path depth, independent of alias count.
            for length in 1..=path.components().len() {
                let alias = NamePath::new(path.unit, path.components()[..length].to_vec())?;
                reads.name(Namespace::Type, &alias)?;
                let strong = self
                    .types
                    .module_paths
                    .has_strong(&super::module_paths::ModulePath {
                        unit: alias.unit,
                        names: alias.components().to_vec(),
                    });
                if !strong && let Some(targets) = self.glob_module_links.get(&alias) {
                    for target in targets {
                        let mut names = target.components().to_vec();
                        names.extend_from_slice(&path.components()[length..]);
                        stack.push(NamePath::new(target.unit, names)?);
                    }
                }
                for target in self.module_aliases.get(&alias).into_iter().chain(
                    self.named_module_links
                        .get(&alias)
                        .into_iter()
                        .flat_map(|targets| targets.iter()),
                ) {
                    let mut names = target.names.clone();
                    names.extend_from_slice(&path.components()[length..]);
                    if names.is_empty() {
                        let root = ScopeKey {
                            crate_key: target.unit,
                            module_path: vec![],
                            owner: None,
                        };
                        reads.scope(namespace, &root)?;
                        if let Some(id) = self.frontiers.get(&root).and_then(|ids| {
                            ids.iter()
                                .find(|id| !ignore_imports || !self.imports.contains_key(id))
                        }) {
                            return Ok(Some(*id));
                        }
                    } else {
                        stack.push(NamePath::new(target.unit, names)?);
                    }
                }
            }
            reads.name(namespace, &path)?;
            if let Some(targets) = self.links.get(&(namespace, path)) {
                stack.extend(targets.iter().cloned());
            }
        }
        Ok(None)
    }
    fn ambiguous_prefix(
        &self,
        path: &NamePath,
        caller: &ScopeKey,
    ) -> Result<Option<Vec<NamePath>>, Diagnostic> {
        self.ambiguous_prefix_observed(path, caller, &mut reads::NamespaceReads::disabled())
    }
    fn ambiguous_prefix_observed(
        &self,
        path: &NamePath,
        caller: &ScopeKey,
        reads: &mut reads::NamespaceReads,
    ) -> Result<Option<Vec<NamePath>>, Diagnostic> {
        let modules = &path.components()[..path.components().len() - 1];
        Ok(
            match self
                .types
                .lookup_module_from_observed(path.unit, modules, caller, reads)?
            {
                super::macro_names::ModuleLookup::Ambiguous(paths) => Some(
                    paths
                        .into_iter()
                        .map(|(unit, names)| NamePath::new(unit, names))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                _ => None,
            },
        )
    }
    /// A demanded glob member; full source import scheduling joins the source integration.
    pub fn glob_member(
        &mut self,
        namespace: Namespace,
        path: NamePath,
        target: NamePath,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        let edge = (namespace, path.clone(), target.clone(), visibility.clone());
        let mut planned_bytes = self.manual_glob_bytes;
        if !self.manual_globs.contains(&edge) {
            let mut text = [path.components(), target.components()]
                .into_iter()
                .flatten()
                .try_fold(0usize, |n, name| {
                    n.checked_add(name.as_str().len())
                        .and_then(|n| n.checked_add(std::mem::size_of::<MacroName>()))
                })
                .ok_or(Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "manual glob metadata overflow".into(),
                ))?;
            if let VisibilityScope::Within { module_path, .. } = &visibility {
                text = module_path
                    .iter()
                    .try_fold(text, |n, name| {
                        n.checked_add(name.len())
                            .and_then(|n| n.checked_add(std::mem::size_of::<String>()))
                    })
                    .ok_or(Diagnostic::new(
                        "LRR-E-SOURCE-LIMIT",
                        "manual glob visibility metadata overflow".into(),
                    ))?;
            }
            planned_bytes = self
                .manual_glob_bytes
                .checked_add(text)
                .and_then(|n| n.checked_add(256))
                .ok_or(Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "manual glob metadata limit".into(),
                ))?;
        }
        self.table_mut(namespace)
            .glob_member(path.clone(), target.clone(), visibility)?;
        self.manual_glob_bytes = planned_bytes;
        self.manual_globs.insert(edge);
        self.links
            .entry((namespace, path))
            .or_default()
            .insert(target);
        Ok(())
    }
    pub(crate) fn admit_generated_import(
        &mut self,
        import: crate::syntax::imports::SourceImport,
        edition: u16,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        let fail = || {
            Diagnostic::new(
                "LRR-E-DISCOVERY-LIMIT",
                "generated import index limit".into(),
            )
        };
        let id = Self::import_identity(&import);
        if let Some((old, old_edition)) = self.imports.get(&id) {
            return if old == &import && *old_edition == edition {
                Ok(())
            } else {
                Err(Diagnostic::new(
                    "LRR-E-EXPANSION-INPUT",
                    "generated import identity has conflicting payload".into(),
                ))
            };
        }
        if import.scope.owner.is_some() {
            return Err(fail());
        }
        let mut bytes = import
            .scope
            .heap_bytes()
            .checked_add(std::mem::size_of_val(&import))
            .ok_or_else(fail)?;
        for part in &import.path {
            if let crate::syntax::imports::ImportSegment::Name(name) = part {
                bytes = bytes
                    .checked_add(name.capacity())
                    .and_then(|n| n.checked_add(64))
                    .ok_or_else(fail)?;
            }
        }
        if let crate::syntax::imports::ImportKind::Named { alias: Some(name) } = &import.kind {
            bytes = bytes.checked_add(name.capacity()).ok_or_else(fail)?;
        }
        bytes = bytes
            .checked_add(crate::syntax::imports::visibility_bytes(&import.visibility))
            .ok_or_else(fail)?;
        let extra = if self
            .frontiers
            .get(&import.scope)
            .is_some_and(|set| set.contains(&id))
        {
            0
        } else {
            import
                .scope
                .heap_bytes()
                .checked_add(192)
                .ok_or_else(fail)?
        };
        let total = self
            .generated_frontier_bytes
            .checked_add(bytes)
            .and_then(|n| n.checked_add(extra))
            .ok_or_else(fail)?;
        self.reserve_generated_frontier(import.scope.clone(), id)?;
        self.generated_frontier_bytes = total;
        if matches!(
            import.kind,
            crate::syntax::imports::ImportKind::Named { .. }
                | crate::syntax::imports::ImportKind::Anonymous
        ) {
            self.named_import_ids.insert(id);
        } else {
            self.named_import_ids.remove(&id);
        }
        self.imports.insert(id, (import, edition));
        Ok(())
    }
    pub(crate) fn reserve_generated_frontier(
        &mut self,
        scope: ScopeKey,
        producer: Digest,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        if self
            .frontiers
            .get(&scope)
            .is_some_and(|set| set.contains(&producer))
        {
            return Ok(());
        }
        let fail = || {
            Diagnostic::new(
                "LRR-E-DISCOVERY-LIMIT",
                "generated namespace frontier limit".into(),
            )
        };
        let bytes = self
            .generated_frontier_bytes
            .checked_add(scope.heap_bytes())
            .and_then(|n| n.checked_add(192))
            .ok_or_else(fail)?;
        if scope.owner.is_some() {
            return Err(fail());
        }
        self.frontiers.entry(scope).or_default().insert(producer);
        self.generated_frontier_bytes = bytes;
        Ok(())
    }
    pub(crate) fn release_generated_frontier(&mut self, scope: &ScopeKey, producer: Digest) {
        self.import_generation = self.import_generation.wrapping_add(1);
        if let Some(frontiers) = self.frontiers.get_mut(scope) {
            frontiers.remove(&producer);
        }
        // Charged cumulative integration work remains bounded across removals.
    }
    pub fn pending(
        &mut self,
        namespace: Namespace,
        path: NamePath,
        producer: Digest,
    ) -> Result<(), Diagnostic> {
        self.import_generation = self.import_generation.wrapping_add(1);
        self.table_mut(namespace).pending(path, producer)
    }
    pub fn lookup_import_module(
        &self,
        import: &crate::syntax::imports::SourceImport,
        edition: u16,
    ) -> Result<super::macro_names::ModuleLookup, Diagnostic> {
        self.lookup_import_module_observed(import, edition, &mut reads::NamespaceReads::disabled())
    }
    pub(crate) fn lookup_import_module_observed(
        &self,
        import: &crate::syntax::imports::SourceImport,
        edition: u16,
        reads: &mut reads::NamespaceReads,
    ) -> Result<super::macro_names::ModuleLookup, Diagnostic> {
        let Some(path) = self.types.import_module_route_observed(
            &import.path,
            import.absolute,
            &import.scope,
            edition,
            reads,
        )?
        else {
            return Ok(super::macro_names::ModuleLookup::Absent);
        };
        self.types
            .lookup_module_from_observed(path.unit, &path.names, &import.scope, reads)
    }
    pub fn lookup_glob_member(
        &self,
        import: &crate::syntax::imports::SourceImport,
        name: &str,
        namespace: Namespace,
        edition: u16,
    ) -> Result<NameLookup, Diagnostic> {
        self.lookup_glob_member_observed(
            import,
            name,
            namespace,
            edition,
            &mut reads::NamespaceReads::disabled(),
        )
    }
    pub(crate) fn lookup_glob_member_observed(
        &self,
        import: &crate::syntax::imports::SourceImport,
        name: &str,
        namespace: Namespace,
        edition: u16,
        reads: &mut reads::NamespaceReads,
    ) -> Result<NameLookup, Diagnostic> {
        reads.scope(Namespace::Type, &import.scope)?;
        use super::module_paths::ModuleResolution;
        let Some(reference) = self.types.import_module_route_observed(
            &import.path,
            import.absolute,
            &import.scope,
            edition,
            reads,
        )?
        else {
            return Ok(NameLookup::NeedInput(Self::import_identity(import)));
        };
        match self.types.module_paths.resolve_module_observed(
            &reference,
            Some(&import.scope),
            reads,
        )? {
            ModuleResolution::Known { .. } => {}
            ModuleResolution::Absent => {
                return Ok(NameLookup::NeedInput(Self::import_identity(import)));
            }
            ModuleResolution::Denied(path) => {
                return Ok(NameLookup::Inaccessible(vec![NamePath::new(
                    path.unit, path.names,
                )?]));
            }
            ModuleResolution::Ambiguous(paths) => {
                return Ok(NameLookup::AmbiguousModules(
                    paths
                        .into_iter()
                        .map(|p| NamePath::new(p.unit, p.names))
                        .collect::<Result<Vec<_>, _>>()?,
                ));
            }
        }
        let mut member = import.clone();
        member
            .path
            .push(crate::syntax::imports::ImportSegment::Name(name.into()));
        Ok(
            match self.lookup_import_target_observed(&member, namespace, edition, reads)? {
                NameLookup::Inaccessible(_) => NameLookup::Absent,
                found => found,
            },
        )
    }
    pub fn lookup_import_target(
        &self,
        import: &crate::syntax::imports::SourceImport,
        namespace: Namespace,
        edition: u16,
    ) -> Result<NameLookup, Diagnostic> {
        self.lookup_import_target_observed(
            import,
            namespace,
            edition,
            &mut reads::NamespaceReads::disabled(),
        )
    }
    pub(crate) fn lookup_import_target_observed(
        &self,
        import: &crate::syntax::imports::SourceImport,
        namespace: Namespace,
        edition: u16,
        reads: &mut reads::NamespaceReads,
    ) -> Result<NameLookup, Diagnostic> {
        reads.scope(Namespace::Type, &import.scope)?;
        if let Some(root) = self.types.import_module_route_observed(
            &import.path,
            import.absolute,
            &import.scope,
            edition,
            reads,
        )? && root.names.is_empty()
        {
            return Ok(if namespace != Namespace::Type {
                NameLookup::Absent
            } else if let super::module_paths::ModuleResolution::Known { path, .. } = self
                .types
                .module_paths
                .resolve_module_observed(&root, Some(&import.scope), reads)?
            {
                NameLookup::ModuleRoot(path.unit)
            } else {
                NameLookup::NeedInput(Self::import_identity(import))
            });
        }
        // A `self` leaf imports module `a::b` itself: only the type namespace
        // observes it, through the same lookup as `use a::b;`.
        let self_leaf = import.path.len() > 1
            && import.path.last() == Some(&crate::syntax::imports::ImportSegment::SelfModule);
        if self_leaf && namespace != Namespace::Type {
            return Ok(NameLookup::Absent);
        }
        let parts = if self_leaf {
            &import.path[..import.path.len() - 1]
        } else {
            &import.path[..]
        };
        let Some(path) = self.table(namespace).import_segments_path_observed(
            parts,
            import.absolute,
            &import.scope,
            edition,
            reads,
        )?
        else {
            return Ok(NameLookup::NeedInput(Self::import_identity(import)));
        };
        self.lookup_in_class_observed(
            namespace,
            &path,
            &import.scope,
            NameBindingClass::Root,
            reads,
        )
    }
    /// Resolve source module qualifiers through the existing unit/module route index.
    pub fn lookup_source_path(
        &self,
        namespace: Namespace,
        segments: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        caller: &ScopeKey,
        edition: u16,
    ) -> Result<NameLookup, Diagnostic> {
        let Some(path) = self
            .table(namespace)
            .source_segments_path(segments, absolute, caller, edition)?
        else {
            return Ok(NameLookup::Absent);
        };
        let found = self.lookup(namespace, &path, caller)?;
        if found != NameLookup::Absent || absolute || namespace == Namespace::Macro {
            return Ok(found);
        }
        // rustc resolves a relative path's first segment lexically, ending at
        // the standard prelude (`Vec`, `Vec::new`, `Option::Some`): only when
        // the first segment is no module member and no extern-prelude crate.
        let Some(crate::syntax::imports::ImportSegment::Name(first)) = segments.first() else {
            return Ok(found);
        };
        let Some(prelude) = self.standard_preludes.get(&caller.crate_key) else {
            return Ok(found);
        };
        if !self.types.implicit_enabled(caller)
            || self.lookup_module_member(Namespace::Type, first, caller, edition)?
                != NameLookup::Absent
            || self
                .types
                .dependency_bindings()
                .any(|((unit, name), _)| *unit == caller.crate_key && name.as_str() == first)
        {
            return Ok(found);
        }
        let mut parts = prelude
            .module_path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        for segment in segments {
            match segment {
                crate::syntax::imports::ImportSegment::Name(name) => parts.push(name.as_str()),
                _ => return Ok(found),
            }
        }
        self.lookup(namespace, &Self::path(prelude.crate_key, &parts, edition)?, caller)
    }
    /// Optimization eligibility is derived from the actual retained graph.
    /// It never certifies a name, completeness or an import by itself.
    fn scoped_certification_candidates(
        &self,
        scope: &ScopeKey,
        namespace: Namespace,
        edition: u16,
    ) -> Option<std::collections::BTreeSet<MacroName>> {
        self.scoped_certification_candidates_in(
            &mut CertificationBatch::default(),
            scope,
            namespace,
            edition,
        )
    }
    fn scoped_certification_candidates_in(
        &self,
        batch: &mut CertificationBatch,
        scope: &ScopeKey,
        namespace: Namespace,
        edition: u16,
    ) -> Option<std::collections::BTreeSet<MacroName>> {
        if scope.owner.is_some() || !matches!(edition, 2015 | 2018 | 2021 | 2024) {
            return None;
        }
        let parts = super::source_macro_routes::module_names(&scope.module_path, edition).ok()?;
        // Preserve module-root/alias/ambiguous outcomes for spellings that need
        // no item row. Ancestor rewrites use the old universe; an immediate
        // child alias or module link adds its own spelling when the universe
        // has it (its lookup is the only one that route can change);
        // unrelated module aliases do not disable this scope.
        let ancestor = |path: &NamePath| {
            path.unit == scope.crate_key && parts.starts_with(path.components())
        };
        let child = |path: &&NamePath| {
            path.unit == scope.crate_key
                && path.components().len() == parts.len() + 1
                && path.components().starts_with(&parts)
        };
        if self.module_aliases.keys().any(&ancestor)
            || self.named_module_links.keys().any(&ancestor)
            || self.glob_module_links.keys().any(ancestor)
        {
            return None;
        }
        let children = self
            .module_aliases
            .keys()
            .filter(child)
            .chain(self.named_module_links.keys().filter(child))
            .chain(self.glob_module_links.keys().filter(child))
            .filter_map(|path| path.components().last().cloned())
            .collect::<std::collections::BTreeSet<_>>();
        // This is the same source prefix reachability used by
        // producer_frontier. A source module frontier only affects a Value/Type
        // path inside that child; Macro may also acquire crate-root exports.
        // Unknown imports/expansions always require the original full walk.
        let semantic_parts: Vec<_> = parts.iter().map(|name| name.as_str().to_owned()).collect();
        for length in 0..=semantic_parts.len() {
            let prefix = ScopeKey {
                crate_key: scope.crate_key,
                module_path: semantic_parts[..length].to_vec(),
                owner: None,
            };
            for producer in self.frontiers.get(&prefix).into_iter().flatten() {
                if namespace != Namespace::Macro
                    && let Some(child) = self.module_frontier_scopes.get(producer)
                {
                    let inside = child.crate_key == scope.crate_key
                        && semantic_parts.starts_with(&child.module_path);
                    let immediate = child.crate_key == scope.crate_key
                        && child.module_path.len() == semantic_parts.len() + 1
                        && child.module_path.starts_with(&semantic_parts);
                    if !inside && !immediate {
                        continue;
                    }
                }
                return None;
            }
        }
        let mut names = self.table(namespace).scoped_member_names(scope, edition)?;
        if !children.is_empty() {
            // Exactly the spellings the global universe would have offered.
            let universe = batch.universe(self, namespace);
            names.extend(children.into_iter().filter(|name| universe.contains(name)));
        }
        Some(names)
    }
    /// The spellings a glob from `scope` can import: the members bound in
    /// that module in any namespace (definitions, imports, glob re-exports
    /// with a same-spelling target row). A name that only a later round or
    /// producer adds re-enters through the import fixed point, which repeats
    /// until the edge set is stable. None keeps the global universe.
    fn glob_source_member_names(
        &self,
        scope: &ScopeKey,
        edition: u16,
    ) -> Option<std::collections::BTreeSet<MacroName>> {
        let mut names = std::collections::BTreeSet::new();
        for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
            names.extend(self.table(namespace).scoped_member_names(scope, edition)?);
        }
        Some(names)
    }
    /// Module member first, then an explicitly supplied prelude scope. Locals
    /// and body-local item ribs are handled by their exact entered source index.
    pub(crate) fn certified_scope(
        &self,
        scope: &ScopeKey,
        namespace: Namespace,
        edition: u16,
    ) -> Result<crate::db::seals::CertifiedScope, Diagnostic> {
        self.certified_scope_in(&mut CertificationBatch::default(), scope, namespace, edition)
    }
    /// `certified_scope` for one of many scopes certified against this same
    /// unchanged graph: the batch shares the name universe and each crate's
    /// standard-prelude answers instead of recomputing them per scope.
    pub(crate) fn certified_scope_in(
        &self,
        batch: &mut CertificationBatch,
        scope: &ScopeKey,
        namespace: Namespace,
        edition: u16,
    ) -> Result<crate::db::seals::CertifiedScope, Diagnostic> {
        let _span = crate::core::timing::Span::enter(|| "names certified_scope".into());
        let candidates = match self.scoped_certification_candidates_in(batch, scope, namespace, edition)
        {
            Some(candidates) => candidates,
            None => batch.universe(self, namespace).clone(),
        };
        self.certified_scope_from_candidates_in(batch, scope, namespace, edition, candidates)
    }
    fn certified_scope_from_candidates(
        &self,
        scope: &ScopeKey,
        namespace: Namespace,
        edition: u16,
        candidates: std::collections::BTreeSet<MacroName>,
    ) -> Result<crate::db::seals::CertifiedScope, Diagnostic> {
        self.certified_scope_from_candidates_in(
            &mut CertificationBatch::default(),
            scope,
            namespace,
            edition,
            candidates,
        )
    }
    fn certified_scope_from_candidates_in(
        &self,
        batch: &mut CertificationBatch,
        scope: &ScopeKey,
        namespace: Namespace,
        edition: u16,
        candidates: std::collections::BTreeSet<MacroName>,
    ) -> Result<crate::db::seals::CertifiedScope, Diagnostic> {
        let mut names = std::collections::BTreeMap::new();
        let mut bytes = 0usize;
        // Module members first; then the names the crate's standard prelude
        // supplies, exactly as ordinary unqualified lookup falls back to it.
        // Every candidate set holds all spellings with a module answer, so a
        // prelude spelling outside it is a module absence.
        let mut lookups = std::collections::BTreeMap::new();
        let mut prelude_names = std::collections::BTreeSet::new();
        for name in candidates {
            let lookup = self.lookup_module_member(namespace, name.as_str(), scope, edition)?;
            lookups.insert(name, lookup);
        }
        if let Some(prelude) = batch.prelude(self, scope, namespace, edition)? {
            for (name, answer) in prelude {
                match lookups.get_mut(name) {
                    Some(lookup) if *lookup == NameLookup::Absent => {
                        *lookup = answer.clone();
                        prelude_names.insert(name.as_str().to_owned());
                    }
                    Some(_) => {}
                    None => {
                        lookups.insert(name.clone(), answer.clone());
                        prelude_names.insert(name.as_str().to_owned());
                    }
                }
            }
        }
        for (name, lookup) in lookups {
            if matches!(lookup, NameLookup::NeedInput(_)) {
                return Err(Diagnostic::new(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "namespace still has unresolved graph frontiers".into(),
                ));
            }
            if lookup == NameLookup::Absent {
                continue;
            }
            bytes = bytes
                .checked_add(name.as_str().len())
                .and_then(|n| n.checked_add(96))
                .and_then(|n| lookup.metadata_bytes().ok().and_then(|m| n.checked_add(m)))
                .ok_or(Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "certified namespace metadata limit".into(),
                ))?;
            names.insert(name.as_str().to_owned(), lookup);
        }
        // Even an empty name universe must check the scope's unresolved frontier index.
        if self
            .frontiers
            .get(scope)
            .is_some_and(|frontiers| !frontiers.is_empty())
        {
            return Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "namespace has unresolved frontiers".into(),
            ));
        }
        Ok(crate::db::seals::CertifiedScope {
            scope: scope.clone(),
            namespace,
            edition,
            names,
            prelude_names,
        })
    }
    pub fn lookup_unqualified(
        &self,
        namespace: Namespace,
        name: &str,
        caller: &ScopeKey,
        edition: u16,
        prelude: Option<&ScopeKey>,
    ) -> Result<NameLookup, Diagnostic> {
        self.lookup_unqualified_observed(
            namespace,
            name,
            caller,
            edition,
            prelude,
            &mut reads::NamespaceReads::disabled(),
        )
    }
    pub(crate) fn lookup_unqualified_observed(
        &self,
        namespace: Namespace,
        name: &str,
        caller: &ScopeKey,
        edition: u16,
        prelude: Option<&ScopeKey>,
        reads: &mut reads::NamespaceReads,
    ) -> Result<NameLookup, Diagnostic> {
        self.lookup_unqualified_in_class_observed(
            namespace,
            name,
            caller,
            edition,
            prelude,
            NameBindingClass::Root,
            reads,
        )
    }
    /// Pure class-keyed lookup facts. The caller's actual source occurrence and
    /// current namespace selection must be proved before using the result.
    pub(crate) fn lookup_unqualified_in_class(
        &self,
        namespace: Namespace,
        name: &str,
        caller: &ScopeKey,
        edition: u16,
        prelude: Option<&ScopeKey>,
        class: NameBindingClass,
    ) -> Result<NameLookup, Diagnostic> {
        self.lookup_unqualified_in_class_observed(
            namespace,
            name,
            caller,
            edition,
            prelude,
            class,
            &mut reads::NamespaceReads::disabled(),
        )
    }
    pub(crate) fn lookup_unqualified_in_class_observed(
        &self,
        namespace: Namespace,
        name: &str,
        caller: &ScopeKey,
        edition: u16,
        prelude: Option<&ScopeKey>,
        class: NameBindingClass,
        reads: &mut reads::NamespaceReads,
    ) -> Result<NameLookup, Diagnostic> {
        let mut parts = caller
            .module_path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        parts.push(name);
        let path = Self::path(caller.crate_key, &parts, edition)?;
        let found = self.lookup_in_class_observed(namespace, &path, caller, class, reads)?;
        if found != NameLookup::Absent {
            return Ok(found);
        }
        let Some(prelude) = prelude else {
            return self.lookup_standard_prelude(namespace, name, caller, edition, reads);
        };
        if class != NameBindingClass::Root {
            return Err(Diagnostic::new(
                "LRR-E-NAME-CONTEXT",
                "contextual prelude requires actual name adjustment".into(),
            ));
        }
        if prelude.owner.is_some() {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-SCOPE",
                "prelude requires a module scope".into(),
            ));
        }
        let mut parts = prelude
            .module_path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        parts.push(name);
        self.lookup_in_class_observed(
            namespace,
            &Self::path(prelude.crate_key, &parts, edition)?,
            caller,
            class,
            reads,
        )
    }
    /// `use` spelling of a retained ordinary import, for diagnostics only.
    pub(crate) fn import_spelling(&self, identity: Digest) -> Option<String> {
        use crate::syntax::imports::{ImportKind, ImportSegment};
        let (import, _) = self.imports.get(&identity)?;
        let mut text = import
            .scope
            .module_path
            .iter()
            .map(|name| format!("{name}::"))
            .collect::<String>();
        text.push_str("{use ");
        if import.absolute {
            text.push_str("::");
        }
        let segments = import
            .path
            .iter()
            .map(|segment| match segment {
                ImportSegment::Name(name) => name.as_str(),
                ImportSegment::Crate => "crate",
                ImportSegment::SelfModule => "self",
                ImportSegment::Super => "super",
            })
            .collect::<Vec<_>>();
        text.push_str(&segments.join("::"));
        match &import.kind {
            ImportKind::Glob => text.push_str("::*"),
            ImportKind::Named { alias: Some(alias) } => {
                text.push_str(" as ");
                text.push_str(alias);
            }
            _ => {}
        }
        text.push('}');
        Some(text)
    }
    /// Current ordinary namespace outcomes for a stalled import. This is only
    /// failure evidence: it neither advances discovery nor discharges a producer.
    pub(crate) fn import_resolution_summary(
        &self,
        identity: Digest,
    ) -> Option<String> {
        let (import, edition) = self.imports.get(&identity)?;
        // An evidence lookup must not replace the primary incomplete-discovery
        // diagnostic or its blocked ledger if that lookup itself fails.
        let outcome = |namespace| match self.lookup_import_target(import, namespace, *edition) {
            Ok(value) => format!("{value:?}"),
            Err(error) => format!("error {}: {}", error.code, error.message),
        };
        Some(format!(
            "value={}, type={}, macro={}",
            outcome(Namespace::Value),
            outcome(Namespace::Type),
            outcome(Namespace::Macro)
        ))
    }
    /// Unqualified lookup of a member of the caller's own module only.
    fn lookup_module_member(
        &self,
        namespace: Namespace,
        name: &str,
        caller: &ScopeKey,
        edition: u16,
    ) -> Result<NameLookup, Diagnostic> {
        let mut parts = caller
            .module_path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        parts.push(name);
        self.lookup_in_class_observed(
            namespace,
            &Self::path(caller.crate_key, &parts, edition)?,
            caller,
            NameBindingClass::Root,
            &mut reads::NamespaceReads::disabled(),
        )
    }
    /// The caller crate's standard prelude, last in unqualified Type/Value
    /// resolution (rustc `Scope::StdLibPrelude`). Any module item, import,
    /// glob or extern-prelude name found earlier shadows it.
    fn lookup_standard_prelude(
        &self,
        namespace: Namespace,
        name: &str,
        caller: &ScopeKey,
        edition: u16,
        reads: &mut reads::NamespaceReads,
    ) -> Result<NameLookup, Diagnostic> {
        if namespace == Namespace::Macro {
            return Ok(NameLookup::Absent);
        }
        let Some(prelude) = self.standard_preludes.get(&caller.crate_key) else {
            return Ok(NameLookup::Absent);
        };
        if !self.table(namespace).implicit_enabled(caller) {
            return Ok(NameLookup::Absent);
        }
        let mut parts = prelude
            .module_path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        parts.push(name);
        self.lookup_in_class_observed(
            namespace,
            &Self::path(prelude.crate_key, &parts, edition)?,
            caller,
            NameBindingClass::Root,
            reads,
        )
    }
    pub fn lookup(
        &self,
        namespace: Namespace,
        path: &NamePath,
        caller: &ScopeKey,
    ) -> Result<NameLookup, Diagnostic> {
        self.lookup_in_class(namespace, path, caller, NameBindingClass::Root)
    }
    /// Candidate facts only: qualified/context module authority stays with
    /// the actual source selector and its current query dependencies.
    pub(crate) fn lookup_in_class(
        &self,
        namespace: Namespace,
        path: &NamePath,
        caller: &ScopeKey,
        class: NameBindingClass,
    ) -> Result<NameLookup, Diagnostic> {
        self.lookup_in_class_observed(
            namespace,
            path,
            caller,
            class,
            &mut reads::NamespaceReads::disabled(),
        )
    }
    pub(crate) fn lookup_in_class_observed(
        &self,
        namespace: Namespace,
        path: &NamePath,
        caller: &ScopeKey,
        class: NameBindingClass,
        reads: &mut reads::NamespaceReads,
    ) -> Result<NameLookup, Diagnostic> {
        // An unknown producer may add a duplicate item or change module prefixes,
        // so disclosed definitions do not establish completeness either.
        if let Some(producer) = self.producer_frontier_observed(namespace, path, false, reads)? {
            return Ok(NameLookup::NeedInput(producer));
        }
        if let Some(paths) = self.ambiguous_prefix_observed(path, caller, reads)? {
            return Ok(NameLookup::AmbiguousModules(paths));
        }
        if class == NameBindingClass::Root
            && namespace == Namespace::Type
            && let super::macro_names::ModuleLookup::Defined {
                unit, components, ..
            } = self.types.lookup_module_from_observed(
                path.unit,
                path.components(),
                caller,
                reads,
            )?
            && components.is_empty()
        {
            return Ok(NameLookup::ModuleRoot(unit));
        }
        let owned = |d: MacroDefinition| NameDefinition {
            namespace,
            identity: d.identity,
            defining_unit: d.defining_unit,
            definition_edition: d.definition_edition,
        };
        Ok(
            match self
                .table(namespace)
                .lookup_from_in_class_observed(path, caller, class, namespace, reads)?
            {
                PathLookup::Defined(d) => NameLookup::Defined(owned(d)),
                PathLookup::Ambiguous(ds) => {
                    NameLookup::Ambiguous(ds.into_iter().map(owned).collect())
                }
                PathLookup::Absent => NameLookup::Absent,
                PathLookup::Inaccessible(paths) => NameLookup::Inaccessible(paths),
                PathLookup::NeedInput(producer) => NameLookup::NeedInput(producer),
            },
        )
    }
    pub fn path(unit: Digest, components: &[&str], edition: u16) -> Result<NamePath, Diagnostic> {
        NamePath::new(
            unit,
            components
                .iter()
                .map(|s| owned_name(s, edition))
                .collect::<Result<_, _>>()?,
        )
    }
}

#[cfg(test)]
mod module_frontier_tests {
    use super::*;
    #[test]
    fn disclosed_module_frontiers_preserve_macro_and_child_uncertainty() {
        use crate::{
            core::profile::ProfileIdentity,
            source::snapshot::{CfgSet, SourceBlob},
            syntax::parse::ParsedFile,
        };
        let unit = Digest([221; 32]);
        let cfg = CfgSet::guest_default();
        let parsed = ParsedFile::parse(
            &SourceBlob::new(b"pub fn known(){}mod child{}".to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let outline = parsed.outline(unit, vec![], &cfg).unwrap();
        let mut graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
        let producer = Digest([222; 32]);
        graph
            .reserve_generated_frontier(outline.root_scope.clone(), producer)
            .unwrap();
        assert!(
            matches!(graph.lookup_unqualified(Namespace::Value, "known", &outline.root_scope, 2024, None).unwrap(), NameLookup::NeedInput(id) if id == producer)
        );
        let child = ScopeKey {
            crate_key: unit,
            module_path: vec!["child".into()],
            owner: None,
        };
        graph.disclose_module_source_scope(producer, child).unwrap();
        assert!(matches!(
            graph
                .lookup_unqualified(Namespace::Value, "known", &outline.root_scope, 2024, None)
                .unwrap(),
            NameLookup::Defined(_)
        ));
        assert!(
            matches!(graph.lookup_unqualified(Namespace::Macro, "unseen_export", &outline.root_scope, 2024, None).unwrap(), NameLookup::NeedInput(id) if id == producer)
        );
        let path = NameNamespaces::path(unit, &["child", "unseen"], 2024).unwrap();
        assert!(
            matches!(graph.lookup(Namespace::Value, &path, &outline.root_scope).unwrap(), NameLookup::NeedInput(id) if id == producer)
        );
        let opaque = Digest([223; 32]);
        graph
            .reserve_generated_frontier(outline.root_scope.clone(), opaque)
            .unwrap();
        assert!(
            matches!(graph.lookup_unqualified(Namespace::Value, "known", &outline.root_scope, 2024, None).unwrap(), NameLookup::NeedInput(id) if id == opaque)
        );
    }
}

#[cfg(test)]
#[path = "names_import_ambiguity_tests.rs"]
mod names_import_ambiguity_tests;

#[cfg(test)]
#[path = "names_scoped_candidates_tests.rs"]
mod names_scoped_candidates_tests;
