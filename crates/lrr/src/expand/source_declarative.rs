//! Pure original-source declarative demands from immutable captured snapshots.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    expand::{
        identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        record::ExpansionRecord,
        source_hygiene::source_root,
    },
    frontend::{
        hygiene::HygieneFacts, macro_scopes::HygienicLookup, source_macros::SourceMacroIndex,
    },
    source::{changes::PreparedSourceView, manifest::ItemKind},
    syntax::spans::SourceRange,
};
use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite},
    token_wire::{self as wire, Key, TokenForest, TokenTree},
};
use std::collections::BTreeMap;
type DependencyRoutes = BTreeMap<crate::expand::scopes::MacroName, Digest>;
type RouteDependencySets = (DependencyRoutes, DependencyRoutes);
pub mod assembly;
#[path = "source_declarative/compiler_attribute_preparation.rs"]
pub(crate) mod compiler_attribute_preparation;
#[path = "source_declarative/imported_definition_context.rs"]
mod imported_definition_context;
#[path = "source_declarative/macro_exports.rs"]
pub mod macro_exports;
#[path = "source_declarative/original_annotations.rs"]
pub(crate) mod original_annotations;
#[path = "source_declarative/retained_provider.rs"]
mod retained_provider;
#[path = "source_declarative/scalar_pattern_replay.rs"]
mod scalar_pattern_replay;
pub struct SourceDeclarative {
    source: PreparedSourceView,
    compiler_attribute_preparation:
        Option<std::sync::Arc<compiler_attribute_preparation::CompilerAttributePreparation>>,
    macro_export_role:
        Option<std::sync::Arc<crate::db::memo::macro_export_inputs::MacroExportSourceObservation>>,
    macro_export_module:
        Option<std::sync::Arc<crate::source::macro_export_module::MacroExportModuleAdoption>>,
    pub(crate) compiler:
        Option<std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>>,
    index: SourceMacroIndex,
    pub(crate) procedural:
        Option<std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>>,
    dependencies:
        std::sync::Arc<std::collections::BTreeMap<crate::expand::scopes::MacroName, Digest>>,
    parents: Vec<(usize, Digest)>,
    catalogs: std::collections::BTreeMap<
        Digest,
        std::sync::Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>,
    >,
    // Each append replays against its preceding source prefix, excluding itself
    // and later outputs. The original producer stays pending in the live index.
    derive_prefixes: std::collections::BTreeMap<Digest, (u64, std::sync::Arc<Self>)>,
    source_modules: std::collections::BTreeMap<Digest, std::sync::Arc<Self>>,
    generated: std::collections::BTreeMap<
        Digest,
        std::sync::Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>,
    >,
    // Immutable token forest of this provider's shared source, tokenized once
    // and shared by snapshots of the same source.
    owned_forest: std::sync::Arc<std::sync::OnceLock<std::sync::Arc<TokenForest>>>,
    // The last original-source route catalogue and the exact inputs it was
    // computed from (see `original_routes`). Derived data only.
    original_routes: std::sync::Mutex<Option<OriginalRoutes>>,
}
/// A memoized original-source route catalogue of one provider.
struct OriginalRoutes {
    key: OriginalRoutesKey,
    routes: std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>,
}
/// Everything the original-source route catalogue reads. The catalogue is a
/// pure function of the provider's source (never replaced), its macro index
/// state, the index state of the parents it resolves through, and the shared
/// handles below. Handles compare by identity and the key keeps them alive, so
/// an address cannot be reused while the entry exists; index states compare by
/// their process-unique stamps, which every index mutation replaces.
struct OriginalRoutesKey {
    index: u64,
    parents: Vec<(usize, Digest, u64)>,
    dependencies: std::sync::Arc<std::collections::BTreeMap<crate::expand::scopes::MacroName, Digest>>,
    compiler: Option<std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>>,
    procedural: Option<std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>>,
    role: Option<std::sync::Arc<crate::db::memo::macro_export_inputs::MacroExportSourceObservation>>,
    preparation:
        Option<std::sync::Arc<compiler_attribute_preparation::CompilerAttributePreparation>>,
}
impl OriginalRoutesKey {
    fn same(&self, other: &Self) -> bool {
        fn handle<T>(a: &Option<std::sync::Arc<T>>, b: &Option<std::sync::Arc<T>>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
                _ => false,
            }
        }
        self.index == other.index
            && self.parents == other.parents
            && std::sync::Arc::ptr_eq(&self.dependencies, &other.dependencies)
            && handle(&self.compiler, &other.compiler)
            && handle(&self.procedural, &other.procedural)
            && handle(&self.role, &other.role)
            && handle(&self.preparation, &other.preparation)
    }
}
#[derive(Clone, Copy)]
pub(crate) struct CapturedMacroContext<'a> {
    pub provider: &'a SourceDeclarative,
    pub owners: &'a [SourceDeclarative],
    pub before: u64,
    pub inherited: Option<&'a InheritedMacroContext<'a>>,
    /// Separate actual provider arena; never used to index caller parents.
    pub exports: Option<&'a super::source_discovery::SealedMacroExportProviders>,
}
pub(crate) struct InheritedMacroContext<'a> {
    facts: HygieneFacts,
    ancestry: crate::frontend::macro_origins::ExpansionAncestry,
    outers: Vec<crate::frontend::macro_scopes::OuterMacroScope<'a>>,
    bodies: Vec<&'a crate::frontend::generated_macros::GeneratedMacroCatalog>,
    sources: Vec<&'a SourceDeclarative>,
}
impl<'a> InheritedMacroContext<'a> {
    pub(crate) fn extend_context(&mut self, parent: &Self) -> Result<(), Diagnostic> {
        self.outers
            .len()
            .checked_add(parent.outers.len())            .ok_or_else(|| fault("LRR-E-MACRO-SCOPE", "inherited module depth limit"))?;
        self.facts.extend(&parent.facts)?;
        self.ancestry.extend(&parent.ancestry)?;
        self.facts
            .metadata_bytes()?
            .checked_add(self.ancestry.metadata_bytes()?)
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "inherited module context budget"))?;
        self.outers
            .try_reserve_exact(parent.outers.len())
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "inherited module outer allocation"))?;
        self.bodies
            .try_reserve_exact(parent.bodies.len())
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "inherited module body allocation"))?;
        self.sources
            .len()
            .checked_add(parent.sources.len())            .ok_or_else(|| fault("LRR-E-MACRO-SCOPE", "inherited source owner count limit"))?;
        self.sources
            .try_reserve_exact(parent.sources.len())
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "inherited source owner allocation"))?;
        self.sources.extend(parent.sources.iter().copied());
        self.outers.extend(parent.outers.iter().copied());
        self.bodies.extend(parent.bodies.iter().copied());
        Ok(())
    }
    pub(crate) fn for_source_module(
        producer: Digest,
        captured: CapturedMacroContext<'a>,
    ) -> Result<Self, Diagnostic> {
        let outer = captured.provider.index.module_outer(producer)?;
        let mut context = Self {
            facts: captured.provider.index.hygiene_facts().clone(),
            ancestry: captured.provider.index.expansion_ancestry().clone(),
            outers: vec![outer],
            bodies: Vec::new(),
            sources: vec![captured.provider],
        };
        for (ordinal, input) in &captured.provider.parents {
            let parent = captured
                .owners
                .get(*ordinal)
                .ok_or_else(|| fault("LRR-E-MACRO-SCOPE", "source module parent absent"))?;
            context.facts.extend(parent.index.hygiene_facts())?;
            context.ancestry.extend(parent.index.expansion_ancestry())?;
            context.outers.push(parent.index.module_outer(*input)?);
            context.sources.push(parent);
        }
        if let Some(parent) = captured.inherited {
            context.extend_context(parent)?;
        }
        Ok(context)
    }
    pub(crate) fn for_module(
        producer: Digest,
        catalog: &'a crate::frontend::generated_macros::GeneratedMacroCatalog,
        ancestors: &[&'a crate::frontend::generated_macros::GeneratedMacroCatalog],
        captured: CapturedMacroContext<'a>,
    ) -> Result<Self, Diagnostic> {
        let count = ancestors
            .len()
            .checked_add(captured.source_scope_count()?)
            .and_then(|n| n.checked_add(1))            .ok_or_else(|| fault("LRR-E-MACRO-SCOPE", "module inherited scope limit"))?;
        let mut facts = catalog.facts().clone();
        let mut ancestry = catalog.ancestry().clone();
        let mut outers = Vec::new();
        outers
            .try_reserve_exact(count)
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "module inherited scope allocation"))?;
        outers.push(catalog.module_parent_outer(producer)?);
        let mut bodies = Vec::new();
        bodies
            .try_reserve_exact(
                ancestors
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "module body count overflow"))?,
            )
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "module body owner allocation"))?;
        bodies.push(catalog);
        for (ordinal, ancestor) in ancestors.iter().enumerate() {
            facts.extend(ancestor.facts())?;
            ancestry.extend(ancestor.ancestry())?;
            let child = if ordinal == 0 {
                catalog
            } else {
                ancestors[ordinal - 1]
            };
            outers.push(ancestor.invocation_outer(child.invocation())?);
            bodies.push(*ancestor);
        }
        facts.extend(captured.provider.index.hygiene_facts())?;
        ancestry.extend(captured.provider.index.expansion_ancestry())?;
        captured.append_source_scopes(
            ancestors
                .last()
                .map_or(catalog.owner(), |root| root.owner()),
            &mut facts,
            &mut ancestry,
            &mut outers,
        )?;
        facts
            .metadata_bytes()?
            .checked_add(ancestry.metadata_bytes()?)
            .ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "module inherited hygiene metadata limit",
                )
            })?;
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(captured.source_scope_count()?)
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "inherited source owner allocation"))?;
        sources.push(captured.provider);
        for (ordinal, _) in &captured.provider.parents {
            sources.push(
                captured
                    .owners
                    .get(*ordinal)
                    .ok_or_else(|| fault("LRR-E-MACRO-SCOPE", "inherited source owner absent"))?,
            );
        }
        if let Some(parent) = captured.inherited {
            sources.extend(parent.sources.iter().copied());
        }
        Ok(Self {
            facts,
            ancestry,
            outers,
            bodies,
            sources,
        })
    }
}
impl<'a> CapturedMacroContext<'a> {
    pub(crate) fn retain_source_scopes(
        self,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> Result<super::body_macro_boundary::RetainedBoundaryParents, Diagnostic> {
        self.source_scope_count()?;
        let mut retained = super::body_macro_boundary::RetainedBoundaryParents::new()?;
        let outer = self.provider.index.outer_scope(scope, self.before)?;
        retained.scopes.push(
            self.provider
                .index
                .retain_outer(outer)?
                .ok_or_else(|| fault("LRR-E-MACRO-SCOPE", "captured source outer root differs"))?,
        );
        retained.sources.push(self.provider.source.share());
        for (ordinal, input) in &self.provider.parents {
            let parent = self
                .owners
                .get(*ordinal)
                .ok_or_else(|| fault("LRR-E-MACRO-SCOPE", "captured source parent absent"))?;
            let outer = parent.index.module_outer(*input)?;
            retained.scopes.push(
                parent.index.retain_outer(outer)?.ok_or_else(|| {
                    fault("LRR-E-MACRO-SCOPE", "captured parent outer root differs")
                })?,
            );
            retained.sources.push(parent.source.share());
        }
        if let Some(parent) = self.inherited {
            for outer in &parent.outers {
                let mut selected = None;
                for body in &parent.bodies {
                    if let Some(root) = body.retain_outer(*outer)? {
                        selected = Some(root);
                        break;
                    }
                }
                if selected.is_none() {
                    for source in &parent.sources {
                        if let Some(root) = source.index.retain_outer(*outer)? {
                            selected = Some(root);
                            break;
                        }
                    }
                }
                retained.scopes.push(selected.ok_or_else(|| {
                    fault(
                        "LRR-E-MACRO-SCOPE",
                        "inherited outer immutable owner absent",
                    )
                })?);
            }
            retained
                .sources
                .extend(parent.sources.iter().map(|source| source.source.share()));
            retained.items.extend(
                parent
                    .bodies
                    .iter()
                    .map(|body| std::sync::Arc::clone(body.retained_items())),
            );
        }
        retained.require_bound()?;
        Ok(retained)
    }
    pub(crate) fn source_scope_count(self) -> Result<usize, Diagnostic> {
        self.provider
            .parents
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_add(self.inherited.map_or(0, |parent| parent.outers.len())))            .ok_or_else(|| fault("LRR-E-MACRO-SCOPE", "captured source ancestry limit"))
    }
    pub(crate) fn append_source_scopes(
        self,
        scope: &crate::syntax::outline::ScopeKey,
        facts: &mut HygieneFacts,
        ancestry: &mut crate::frontend::macro_origins::ExpansionAncestry,
        outers: &mut Vec<crate::frontend::macro_scopes::OuterMacroScope<'a>>,
    ) -> Result<(), Diagnostic> {
        outers.push(self.provider.index.outer_scope(scope, self.before)?);
        for (ordinal, input) in &self.provider.parents {
            let parent = self.owners.get(*ordinal).ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-SCOPE",
                    "captured module parent catalogue absent",
                )
            })?;
            facts.extend(parent.index.hygiene_facts())?;
            ancestry.extend(parent.index.expansion_ancestry())?;
            outers.push(parent.index.module_outer(*input)?);
        }
        if let Some(parent) = self.inherited {
            facts.extend(&parent.facts)?;
            ancestry.extend(&parent.ancestry)?;
            outers.extend(parent.outers.iter().copied());
        }
        Ok(())
    }
    pub(crate) fn inherited_origin(
        self,
        definition: &crate::expand::scopes::MacroDefinition,
        ancestry: &mut crate::frontend::macro_origins::ExpansionAncestry,
    ) -> Result<Option<Option<crate::syntax::token_wire::Key>>, Diagnostic> {
        let mut origin = None;
        if let Some(parent) = self.inherited {
            ancestry.extend(&parent.ancestry)?;
            for catalog in &parent.bodies {
                if let Some(found) = catalog.definition_origin(definition)? {
                    if origin.is_some_and(|old| old != found) {
                        return Err(fault(
                            "LRR-E-EXPANSION-INPUT",
                            "inherited textual origin differs",
                        ));
                    }
                    origin = Some(found);
                }
            }
        }
        if let Some(found) = self.exported_definition_origin(definition, ancestry)? {
            if origin.is_some_and(|old| old != found) {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "imported textual origin differs",
                ));
            }
            origin = Some(found);
        }
        Ok(origin)
    }
    pub(crate) fn inherited_body(
        self,
        definition: &crate::expand::scopes::MacroDefinition,
    ) -> Option<std::sync::Arc<super::generated_items::GeneratedItems>> {
        self.inherited
            .and_then(|parent| {
                parent
                    .bodies
                    .iter()
                    .find_map(|catalog| catalog.module_definition_body(definition))
            })
            .or_else(|| self.exported_definition_body(definition))
    }
    pub(crate) fn definition_owner(
        self,
        definition: &crate::expand::scopes::MacroDefinition,
    ) -> Result<&'a SourceDeclarative, Diagnostic> {
        let mut remaining = usize::MAX;
        for owner in std::iter::once(self.provider)
            .chain(self.owners.iter())
            .chain(
                self.inherited
                    .into_iter()
                    .flat_map(|parent| parent.sources.iter().copied()),
            )
        {
            if let Some(found) = owner.retained_definition_owner(definition, 0, &mut remaining)? {
                return Ok(found);
            }
        }
        if let Some(found) = self.exported_definition_owner(definition, &mut remaining)? {
            return Ok(found);
        }
        // Standard library macro bodies come from the owning compiler facet's
        // real library sources.
        if let Some(library) = self.provider.compiler.as_ref().and_then(|c| c.library()) {
            for owner in library.providers() {
                if let Some(found) = owner.retained_definition_owner(definition, 0, &mut remaining)? {
                    return Ok(found);
                }
            }
        }
        Err(fault(
            "LRR-E-EXPANSION-INPUT",
            "selected captured macro owner absent",
        ))
    }
}
pub(crate) struct CompletedCatalogueContext<'a> {
    pub catalog: &'a crate::frontend::generated_macros::GeneratedMacroCatalog,
    pub ancestors: Vec<&'a crate::frontend::generated_macros::GeneratedMacroCatalog>,
    pub provider: &'a SourceDeclarative,
    pub before: u64,
}
pub enum SourceDeclarativeDemand {
    Ready(Box<SourceDeclarativeRequest>),
    NeedInput(Digest),
}
pub struct SourceDeclarativeRequest {
    key: ExpansionKey,
    body: DeclarativeBody,
    input: TokenForest,
    site: Key,
    owner: Digest,
    origin: Option<crate::syntax::origin::MacroOrigin>,
}
struct CompilerBody {
    catalog: std::sync::Arc<crate::frontend::compile_input_catalog::CompileInputCatalog>,
    definition: crate::expand::scopes::MacroDefinition,
    source: PreparedSourceView,
    scope: crate::syntax::outline::ScopeKey,
    index: CompilerIndex,
    facts: HygieneFacts,
    ancestry: crate::frontend::macro_origins::ExpansionAncestry,
    outers: Vec<OwnedCompilerOuter>,
}
enum CompilerIndex {
    Module(Box<SourceMacroIndex>),
    Declaration(crate::frontend::impl_index::DeclarationMacroParent),
}
struct OwnedCompilerOuter {
    scopes: crate::expand::scopes::TextualScopes,
    declarations: crate::frontend::macro_scopes::MacroDeclarations,
    scope: crate::expand::scopes::ScopeKey,
    before: u64,
}
enum DeclarativeBody {
    /// Original source-owned dialect/groups; never a rules-body rewrite.
    Macro2 {
        arguments: Option<TokenForest>,
        body: TokenForest,
    },
    Procedural(std::sync::Arc<crate::service::function_macro_requests::PreparedFunctionRequest>),
    OwnedGenerated(
        std::sync::Arc<super::generated_items::GeneratedItems>,
        Digest,
    ),
    Compiler(Box<CompilerBody>),
    Source(TokenForest),
    Generated {
        catalog: std::sync::Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>,
        definition: Digest,
    },
    /// A `#[rustc_builtin_macro]` library definition expanded by the pinned
    /// adapter's builtin expander with its rustc call-site facts.
    Builtin {
        kind: lrr_expand_adapter::builtin_macros::BuiltinKind,
        facts: std::sync::Arc<lrr_expand_adapter::builtin_macros::BuiltinCallSite>,
    },
    /// A library `#[rustc_builtin_macro]` derive applied to an item; the
    /// adapter's builtin derive expander produces the appended impl items.
    BuiltinDerive {
        kind: lrr_expand_adapter::builtin_macros::BuiltinDeriveKind,
        facts: lrr_expand_adapter::builtin_macros::DeriveContainerFacts,
    },
}
/// rustc call-site facts for a builtin invoked in `source` at byte `at`.
/// `line!`/`column!`/`file!` use the outermost original call site.
fn builtin_call_site(
    source: &PreparedSourceView,
    at: u32,
    call_edition: u16,
) -> Result<lrr_expand_adapter::builtin_macros::BuiltinCallSite, Diagnostic> {
    let text = std::str::from_utf8(source.captured_macro_blob()?.bytes())
        .map_err(|_| fault("LRR-E-SOURCE-UTF8", "builtin call source is not UTF-8"))?;
    let prefix = text
        .get(..at as usize)
        .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "builtin call site outside source"))?;
    let line = prefix.matches('\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix, |(_, tail)| tail)
        .chars()
        .count()
        + 1;
    let selection = source.selection();
    let path = selection.path.as_str().to_owned();
    // rustc's default crate name is the root file stem with `-` as `_`.
    let crate_name = std::path::Path::new(&path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("main")
        .replace('-', "_");
    let module_path = std::iter::once(crate_name)
        .chain(selection.outline.module_path.iter().cloned())
        .collect::<Vec<_>>()
        .join("::");
    let cfg = source.captured_macro_cfg()?;
    let mut active = std::collections::BTreeSet::new();
    for atom in &cfg.atoms {
        active.insert((atom.clone(), None));
    }
    for (name, values) in &cfg.values {
        for value in values {
            active.insert((name.clone(), Some(value.clone())));
        }
    }
    let unit = selection.outline.crate_key;
    let profile = &selection.outline.parse.profile;
    let mut crate_names = std::collections::BTreeMap::new();
    crate_names.insert(Key(unit.0), "crate".to_owned());
    for name in ["core", "alloc", "std"] {
        let library = crate::facade::unit(profile, name);
        if library != unit {
            crate_names.insert(Key(library.0), name.to_owned());
        }
    }
    Ok(lrr_expand_adapter::builtin_macros::BuiltinCallSite {
        edition: edition(call_edition)?,
        line: u32::try_from(line).map_err(|_| fault("LRR-E-SOURCE-LIMIT", "line overflow"))?,
        column: u32::try_from(column)
            .map_err(|_| fault("LRR-E-SOURCE-LIMIT", "column overflow"))?,
        file: path,
        module_path,
        cfg: active,
        crate_names,
    })
}
/// Actual source producer generation includes the pinned backend and the
/// source declaration/dialect/hygiene implementation, not only its dependency.
/// This is identity data; it does not grant expansion or runtime authority.
pub fn engine_version() -> Digest {
    let mut e = crate::core::digest::StableEncoder::new("lrr/v1/source-declarative-engine");
    e.string(lrr_expand_adapter::UPSTREAM_COMMIT);
    e.digest(Digest(
        lrr_expand_adapter::builtin_attributes::compiler_inert_inventory_identity().0,
    ));
    e.u32(6); // Candidate attribute scheduling remains distinct from final selection.
    e.finish()
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
fn definition_origin_bytes(
    origin: &crate::syntax::origin::MacroOrigin,
) -> Result<usize, Diagnostic> {
    let mut bytes = std::mem::size_of_val(origin);
    let limit = || {
        fault(
            "LRR-E-METADATA-LIMIT",
            "compiler operand origin accounting limit",
        )
    };
    if let Some(invocation) = &origin.invocation {
        bytes = bytes
            .checked_add(invocation.source.path.capacity())
            .ok_or_else(limit)?;
    }
    if let Some(definition) = &origin.definition {
        bytes = bytes
            .checked_add(definition.name.capacity())
            .and_then(|n| n.checked_add(definition.location.source.path.capacity()))
            .ok_or_else(limit)?;
    }
    bytes = bytes
        .checked_add(
            origin
                .parents
                .capacity()
                .checked_mul(std::mem::size_of::<crate::syntax::origin::ExpansionOrigin>())
                .ok_or_else(limit)?,
        )
        .ok_or_else(limit)?;
    for parent in &origin.parents {
        if let Some(invocation) = &parent.invocation {
            bytes = bytes
                .checked_add(invocation.source.path.capacity())
                .ok_or_else(limit)?;
        }
    }
    Ok(bytes)
}
fn adapter(e: lrr_expand_adapter::AdapterError) -> Diagnostic {
    fault(e.code, &e.message)
}
pub(crate) fn edition(n: u16) -> Result<wire::Edition, Diagnostic> {
    match n {
        2015 => Ok(wire::Edition::E2015),
        2018 => Ok(wire::Edition::E2018),
        2021 => Ok(wire::Edition::E2021),
        2024 => Ok(wire::Edition::E2024),
        _ => Err(fault(
            "LRR-E-EXPANSION-INPUT",
            "source macro edition differs",
        )),
    }
}
pub(crate) fn group(forest: &TokenForest, range: SourceRange) -> Result<TokenForest, Diagnostic> {
    let mut stack = forest.roots().iter().collect::<Vec<_>>();
    while let Some(token) = stack.pop() {
        if let TokenTree::Group {
            open,
            close,
            children,
            ..
        } = token
        {
            let first = forest.span_by_key(open).ok_or_else(|| {
                fault("LRR-E-EXPANSION-INPUT", "macro group opening origin absent")
            })?;
            let last = forest.span_by_key(close).ok_or_else(|| {
                fault("LRR-E-EXPANSION-INPUT", "macro group closing origin absent")
            })?;
            if first.start == range.start && last.end == range.end {
                // The same complete source tables with this group as the root.
                return forest.with_roots(vec![token.clone()]).map_err(adapter);
            }
            stack.extend(children);
        }
    }
    Err(fault(
        "LRR-E-EXPANSION-INPUT",
        "captured macro group range absent",
    ))
}
pub(crate) fn tokens_for_source_range(
    forest: &TokenForest,
    selected: SourceRange,
) -> Result<TokenForest, Diagnostic> {
    let mut stack = forest.roots().iter().rev().collect::<Vec<_>>();
    let mut roots = Vec::new();
    while let Some(token) = stack.pop() {
        let (first, last) = match token {
            TokenTree::Group { open, close, .. } => (*open, *close),
            TokenTree::Ident { span, .. }
            | TokenTree::Literal { span, .. }
            | TokenTree::Punct { span, .. } => (*span, *span),
        };
        let first = forest.span_by_key(&first).ok_or_else(|| {
            fault(
                "LRR-E-EXPANSION-INPUT",
                "argument token opening source absent",
            )
        })?;
        let last = forest.span_by_key(&last).ok_or_else(|| {
            fault(
                "LRR-E-EXPANSION-INPUT",
                "argument token closing source absent",
            )
        })?;
        if first.start >= selected.start && last.end <= selected.end {
            roots
                .try_reserve(1)
                .map_err(|_| fault("LRR-E-METADATA-LIMIT", "argument token allocation"))?;
            roots.push(token.clone());
        } else if let TokenTree::Group { children, .. } = token {
            if last.end > selected.start && first.start < selected.end {
                stack.try_reserve(children.len()).map_err(|_| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "argument token traversal allocation",
                    )
                })?;
                stack.extend(children.iter().rev());
            }
        } else if first.start < selected.end && last.end > selected.start {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "argument range splits a source token",
            ));
        }
    }
    if roots.is_empty() {
        return Err(fault(
            "LRR-E-EXPANSION-INPUT",
            "argument source tokens absent",
        ));
    }
    let span_keys = |token: &TokenTree| match token {
        TokenTree::Group { open, close, .. } => (*open, *close),
        TokenTree::Ident { span, .. }
        | TokenTree::Literal { span, .. }
        | TokenTree::Punct { span, .. } => (*span, *span),
    };
    let open = span_keys(
        roots
            .first()
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "argument opening token absent"))?,
    )
    .0;
    let close = span_keys(
        roots
            .last()
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "argument closing token absent"))?,
    )
    .1;
    forest
        .with_roots(vec![TokenTree::Group {
            delimiter: wire::Delimiter::Invisible,
            open,
            close,
            children: roots,
        }])
        .map_err(adapter)
}

/// Resolve a generated call path. A leading `$crate` names the defining crate
/// of that token (definition-site hygiene); other paths resolve from the
/// call's scope exactly like original source.
fn generated_path_lookup(
    index: &crate::frontend::impl_index::GeneratedDeclarationMacroIndex,
    invocation: Digest,
    scope: &crate::syntax::outline::ScopeKey,
    namespaces: &crate::frontend::macro_names::MacroNamespaces,
) -> Result<crate::frontend::macro_names::PathLookup, Diagnostic> {
    use crate::frontend::macro_names::{MacroPath, PathLookup};
    let syntax = index.output().macro_syntax(invocation)?;
    let mut scope = scope.clone();
    scope.owner = None;
    let lookup = if matches!(
        syntax.segments().first(),
        Some(crate::syntax::imports::ImportSegment::Name(name)) if name == "$crate"
    ) {
        let unit = index.output().dollar_crate_unit(invocation)?;
        let mut tail = Vec::new();
        for segment in &syntax.segments()[1..] {
            let crate::syntax::imports::ImportSegment::Name(name) = segment else {
                return Ok(PathLookup::Absent);
            };
            tail.push(crate::expand::scopes::MacroName::parse(
                name,
                syntax.key().profile.edition,
            )?);
        }
        namespaces.lookup_from(&MacroPath::new(unit, tail)?, &scope)?
    } else {
        index.lookup_imports(invocation, syntax.segments(), syntax.absolute(), namespaces)?
    };
    Ok(lookup)
}
fn original_definition_body(
    source: &PreparedSourceView,
    selected: SourceRange,
    forest: &TokenForest,
) -> Result<DeclarativeBody, Diagnostic> {
    // A `#[rustc_builtin_macro]` body is a placeholder; the compiler expands
    // the call. It must never run through the declarative matcher.
    if let Some(item) = source.outline().items.iter().find(|item| {
        item.declaration_span == selected
            && item
                .header
                .attributes
                .iter()
                .any(|attribute| attribute.name == "rustc_builtin_macro")
    }) {
        return Err(fault(
            "LRR-E-UNSUPPORTED",
            &format!(
                "compiler builtin macro `{}` requires its builtin expander",
                item.def_key.declared_name
            ),
        ));
    }
    let shape = source.macro_definition_shape_at(selected)?;
    if shape.dialect() == crate::syntax::macro_calls::MacroDefinitionDialect::Macro2
        && shape.arguments().is_some()
    {
        // Each selected group keeps the owning forest's complete source tables.
        // Reserve a conservative combined ceiling before cloning either group.
        forest
            .encoded_len()
            .map_err(adapter)?
            .checked_mul(2)
            .ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "macro2 original definition groups exceed retained metadata budget",
                )
            })?;
    }
    let body = group(forest, shape.body())?;
    Ok(match shape.dialect() {
        crate::syntax::macro_calls::MacroDefinitionDialect::Rules => DeclarativeBody::Source(body),
        crate::syntax::macro_calls::MacroDefinitionDialect::Macro2 => DeclarativeBody::Macro2 {
            arguments: shape.arguments().map(|r| group(forest, r)).transpose()?,
            body,
        },
    })
}

impl SourceDeclarative {
    pub(crate) fn item_name_context(
        &self,
        context: crate::syntax::token_wire::Key,
    ) -> Result<crate::syntax::token_wire::NameContextKey, Diagnostic> {
        self.index.hygiene_facts().name_item(context)
    }
    fn retained_definition_owner(
        &self,
        definition: &crate::expand::scopes::MacroDefinition,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Option<&Self>, Diagnostic> {
        if *remaining == 0 {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "captured macro owner traversal limit",
            ));
        }
        *remaining -= 1;
        if self.index.definition(definition.identity) == Some(definition)
            && self.index.owns_definition(definition.identity)
        {
            return Ok(Some(self));
        }
        for child in self.source_modules.values() {
            if let Some(owner) =
                child.retained_definition_owner(definition, depth + 1, remaining)?
            {
                return Ok(Some(owner));
            }
        }
        for catalog in self.catalogs.values() {
            if let Some(source) = catalog.captured_definition_owner(definition)
                && let Some(owner) =
                    source.retained_definition_owner(definition, depth + 1, remaining)?
            {
                return Ok(Some(owner));
            }
        }
        Ok(None)
    }
    pub fn new(source: PreparedSourceView) -> Result<Self, Diagnostic> {
        source.require_declaration_syntax()?;
        let h = source_root(
            source.outline().root_scope.crate_key,
            source.outline().profile.edition,
        )?;
        let facts = HygieneFacts::new(h.contexts, h.rows)?.with_name_contexts(h.names)?;
        let index = SourceMacroIndex::from_outline(source.outline(), h.context, facts)?;
        let dependencies = source.source_dependencies()?.aliases_for(
            source.outline().root_scope.crate_key,
            source.outline().profile.edition,
        )?;
        Ok(Self {
            source,
            macro_export_role: None,
            macro_export_module: None,
            compiler_attribute_preparation: None,
            compiler: None,
            index,
            procedural: None,
            dependencies,
            parents: vec![],
            generated: std::collections::BTreeMap::new(),
            catalogs: std::collections::BTreeMap::new(),
            derive_prefixes: std::collections::BTreeMap::new(),
            source_modules: std::collections::BTreeMap::new(),
            owned_forest: Default::default(),
            original_routes: Default::default(),
        })
    }
    pub(crate) fn configure_captured_modules(providers: &mut [Self]) -> Result<(), Diagnostic> {
        let mut links = Vec::new();
        links.try_reserve_exact(providers.len()).map_err(|_| {
            fault(
                "LRR-E-METADATA-LIMIT",
                "module link catalogue allocation failed",
            )
        })?;
        let mut total = 0usize;
        for child in providers.iter() {
            let mut scope = child.source.outline().root_scope.clone();
            if child.macro_export_source_role().is_some()
                && !scope.module_path.is_empty()
                && child.macro_export_module.is_none()
            {
                return Err(fault(
                    "LRR-E-MACRO-EXPORT-PUBLICATION",
                    "macro-export child requires adopted source-parent selection",
                ));
            }
            let mut owner = child;
            let mut chain = Vec::new();
            while !scope.module_path.is_empty() {
                let mut candidates =
                    providers
                        .iter()
                        .enumerate()
                        .filter_map(|(ordinal, parent)| {
                            parent
                                .index
                                .module_input_for(&scope)
                                .map(|id| (ordinal, id))
                        });
                let (ordinal, input) = candidates.next().ok_or_else(|| {
                    fault(
                        "LRR-E-MACRO-SCOPE",
                        "captured external module parent declaration absent",
                    )
                })?;
                if candidates.next().is_some() {
                    return Err(fault(
                        "LRR-E-MACRO-SCOPE",
                        "captured external module parent is ambiguous",
                    ));
                }
                let parent = &providers[ordinal];
                if !child.source.same_publication(&parent.source)
                    || child.source.selection().outline.cfg != parent.source.selection().outline.cfg
                    || child.source.selection().outline.parse.profile
                        != parent.source.selection().outline.parse.profile
                {
                    return Err(fault(
                        "LRR-E-MACRO-SCOPE",
                        "captured external module source/context/publication mismatch",
                    ));
                }
                let next = &parent.source.outline().root_scope;
                if next.crate_key != scope.crate_key || next.module_path.len() >= scope.module_path.len() || !scope.module_path.starts_with(&next.module_path) {
                    return Err(fault(
                        "LRR-E-MACRO-SCOPE",
                        "captured external module hierarchy or depth mismatch",
                    ));
                }
                owner.require_captured_role_parent(parent, input)?;
                parent.index.module_outer(input)?;
                owner = parent;
                total = total
                    .checked_add(64)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-METADATA-LIMIT",
                            "module link catalogue metadata budget",
                        )
                    })?;
                chain.try_reserve(1).map_err(|_| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "module source chain allocation failed",
                    )
                })?;
                chain.push((ordinal, input));
                scope = next.clone();
            }
            links.push(chain);
        }
        for (provider, chain) in providers.iter_mut().zip(links) {
            provider.parents = chain;
        }
        Ok(())
    }
    pub(crate) fn synchronize_macro_uses(providers: &mut [Self]) -> Result<bool, Diagnostic> {
        let mut changed = false;
        loop {
            let mut progress = false;
            for child in 0..providers.len() {
                let Some(&(parent, input)) = providers[child].parents.first() else {
                    continue;
                };
                if parent == child || parent >= providers.len() {
                    return Err(fault(
                        "LRR-E-MACRO-SCOPE",
                        "macro-use captured parent identity invalid",
                    ));
                }
                // A captured plain `mod name;` child publishes nothing into its
                // parent's textual scope; it completes once the child's own
                // producers are complete. `#[macro_use]` children import.
                let macro_use = providers[parent].index.pending_macro_use(input);
                if !macro_use && !providers[parent].index.pending_captured_module(input) {
                    continue;
                }
                if !providers[parent]
                    .source
                    .same_publication(&providers[child].source)
                {
                    return Err(fault(
                        "LRR-E-MACRO-SCOPE",
                        "macro-use captured source lease differs",
                    ));
                }
                let (parent_index, child_index) = if parent < child {
                    let (left, right) = providers.split_at_mut(child);
                    (&mut left[parent].index, &right[0].index)
                } else {
                    let (left, right) = providers.split_at_mut(parent);
                    (&mut right[0].index, &left[child].index)
                };
                let imported = if macro_use {
                    parent_index.complete_captured_macro_use(input, child_index)?
                } else {
                    parent_index.complete_captured_module(input, child_index)?
                };
                progress |= imported;
            }
            changed |= progress;
            if !progress {
                return Ok(changed);
            }
        }
    }
    fn lookup_with_parents(
        &self,
        module: &[String],
        before: u64,
        name: &str,
        owners: &[Self],
    ) -> Result<HygienicLookup, Diagnostic> {
        self.lookup_with_parents_using(module, before, name, owners, false)
    }
    fn lookup_with_parents_using(
        &self,
        module: &[String],
        before: u64,
        name: &str,
        owners: &[Self],
        discovery: bool,
    ) -> Result<HygienicLookup, Diagnostic> {
        let mut facts = self.index.hygiene_facts().clone();
        let mut ancestry = self.index.expansion_ancestry().clone();
        let mut outers = Vec::new();
        outers.try_reserve_exact(self.parents.len()).map_err(|_| {
            fault(
                "LRR-E-METADATA-LIMIT",
                "module outer scope allocation failed",
            )
        })?;
        for (ordinal, input) in &self.parents {
            let parent = owners.get(*ordinal).ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-SCOPE",
                    "captured module parent catalogue absent",
                )
            })?;
            facts.extend(parent.index.hygiene_facts())?;
            ancestry.extend(parent.index.expansion_ancestry())?;
            outers.push(parent.index.module_outer(*input)?);
        }
        if discovery {
            self.index.lookup_with_external_for_discovery(
                module, before, name, &outers, &facts, &ancestry,
            )
        } else {
            self.index
                .lookup_with_external(module, before, name, &outers, &facts, &ancestry)
        }
    }
    pub(crate) fn complete_original_inert_item(
        &mut self,
        invocation: Digest,
        proof: &super::source_derives::SourceEmptyDeriveProof,
    ) -> Result<(), Diagnostic> {
        if self
            .index
            .complete_original_inert_macro(&self.source, invocation, proof)?
        {
            return Ok(());
        }
        self.complete_without_macro_bindings(invocation)
    }
    pub(crate) fn complete_without_macro_bindings(
        &mut self,
        invocation: Digest,
    ) -> Result<(), Diagnostic> {
        let call = self
            .source
            .outline()
            .item_invocations
            .iter()
            .cloned()
            .chain(
                self.source
                    .outline()
                    .items
                    .iter()
                    .filter_map(crate::syntax::outline::potential_attribute_producer),
            )
            .find(|c| c.item_input == invocation)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "completed source invocation absent",
                )
            })?;
        self.index.complete_empty_item_producer(
            &call.scope,
            u64::from(call.range.start),
            invocation,
        )
    }
    pub(crate) fn complete_generated_bindings(
        &mut self,
        invocation: Digest,
        catalog: std::sync::Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>,
    ) -> Result<(), Diagnostic> {
        let call = self
            .source
            .outline()
            .item_invocations
            .iter()
            .cloned()
            .chain(
                self.source
                    .outline()
                    .items
                    .iter()
                    .filter_map(crate::syntax::outline::potential_attribute_producer),
            )
            .find(|c| c.item_input == invocation)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "completed source invocation absent",
                )
            })?;
        self.index.complete_generated_item_producer(
            &call.scope,
            u64::from(call.range.start),
            invocation,
            &catalog,
        )?;
        self.catalogs.insert(invocation, catalog.clone());
        for entry in catalog.outward() {
            self.generated
                .insert(entry.definition.identity, catalog.clone());
        }
        Ok(())
    }
    pub(crate) fn derive_prefix_snapshot(
        &self,
        call: &crate::syntax::outline::ItemInvocation,
    ) -> Result<std::sync::Arc<Self>, Diagnostic> {
        let mut snapshot = self.module_snapshot()?;
        let index = self.index.derive_prefix_snapshot(
            &call.scope,
            u64::from(call.range.start),
            call.item_input,
        )?;
        std::sync::Arc::get_mut(&mut snapshot)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "derive snapshot already shared"))?
            .index = index;
        Ok(snapshot)
    }
    pub(crate) fn publish_derive_bindings(
        &mut self,
        call: &crate::syntax::outline::ItemInvocation,
        catalog: std::sync::Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>,
        prefix: std::sync::Arc<Self>,
    ) -> Result<(), Diagnostic> {
        let id = catalog.record_identity();
        if self.catalogs.contains_key(&id) || !prefix.source.same_publication(&self.source) {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "derive prefix owner or inventory differs",
            ));
        }
        self.index.publish_derive_prefix(
            &call.scope,
            u64::from(call.range.start),
            call.item_input,
            catalog.invocation(),
            &catalog,
        )?;
        for entry in catalog.outward() {
            self.generated
                .insert(entry.definition.identity, catalog.clone());
        }
        let before = u64::from(call.range.start)
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-MACRO-SCOPE", "derive prefix position overflow"))?;
        self.derive_prefixes.insert(id, (before, prefix));
        self.catalogs.insert(id, catalog);
        Ok(())
    }
    pub(crate) fn complete_ordinary_module(
        &mut self,
        input: Digest,
        child: std::sync::Arc<Self>,
    ) -> Result<bool, Diagnostic> {
        if let Some(old) = self.source_modules.get(&input) {
            if old.source.selection() != child.source.selection() {
                return Err(fault(
                    "LRR-E-EXPANSION-CONFLICT",
                    "ordinary module body owner changed",
                ));
            }
            return Ok(false);
        }
        self.source_modules
            .len()
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "ordinary module owner count limit"))?;
        let mut index = self.index.snapshot_clone()?;
        if !index.complete_captured_module(input, &child.index)? {
            return Err(fault(
                "LRR-E-CATALOG-INCOMPLETE",
                "ordinary module source bindings incomplete",
            ));
        }
        self.index = index;
        self.source_modules.insert(input, child);
        Ok(true)
    }
    pub(crate) fn retain_source_module_owner(
        &mut self,
        input: Digest,
        child: std::sync::Arc<Self>,
    ) -> Result<(), Diagnostic> {
        if let Some(old) = self.source_modules.get(&input) {
            if old.source.selection() != child.source.selection() {
                return Err(fault(
                    "LRR-E-EXPANSION-CONFLICT",
                    "retained source module owner changed",
                ));
            }
            return Ok(());
        }
        self.source_modules
            .len()
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "retained source module owner limit"))?;
        if child.source.outline().root_scope.crate_key != self.source.outline().root_scope.crate_key
            || child.source.outline().profile != self.source.outline().profile
            || child.source.outline().cfg != self.source.outline().cfg
            || child.index.pending_producers().next().is_some()
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "retained source module owner context differs",
            ));
        }
        self.source_modules.insert(input, child);
        Ok(())
    }
    pub(crate) fn module_snapshot_bytes(&self) -> Result<usize, Diagnostic> {
        let rows = self
            .catalogs
            .len()
            .checked_add(self.generated.len())
            .and_then(|count| count.checked_add(self.source_modules.len()))
            .and_then(|count| count.checked_add(self.derive_prefixes.len()))
            .and_then(|count| count.checked_mul(128))
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "module owner index overflow"))?;
        self.module_index_bytes()?
            .checked_add(
                self.macro_export_role
                    .as_ref()
                    .map_or(0, |role| role.metadata_bytes()),
            )
            .and_then(|bytes| {
                bytes.checked_add(
                    self.macro_export_module
                        .as_ref()
                        .map_or(0, |module| module.retained_admission_bytes()),
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    self.compiler_attribute_preparation
                        .as_ref()
                        .map_or(0, |plan| plan.metadata_bytes()),
                )
            })
            .and_then(|bytes| bytes.checked_add(rows))
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "module owner snapshot limit"))
    }
    pub(crate) fn module_snapshot(&self) -> Result<std::sync::Arc<Self>, Diagnostic> {
        self.module_snapshot_bytes()?;
        Ok(std::sync::Arc::new(Self {
            source: self.source.share(),
            macro_export_role: self.macro_export_role.clone(),
            macro_export_module: self.macro_export_module.clone(),
            compiler_attribute_preparation: self.compiler_attribute_preparation.clone(),
            compiler: self.compiler.clone(),
            procedural: self.procedural.clone(),
            index: self.index.snapshot_clone()?,
            dependencies: self.dependencies.clone(),
            parents: self.parents.clone(),
            catalogs: self.catalogs.clone(),
            derive_prefixes: self.derive_prefixes.clone(),
            source_modules: self.source_modules.clone(),
            generated: self.generated.clone(),
            owned_forest: self.owned_forest.clone(),
            original_routes: Default::default(),
        }))
    }
    pub(crate) fn bind_macro_export_source(
        &mut self,
        role: std::sync::Arc<crate::db::memo::macro_export_inputs::MacroExportSourceObservation>,
        graph: Digest,
    ) -> Result<(), Diagnostic> {
        role.require_source(&self.source, graph)?;
        if self.compiler.is_some() || self.macro_export_role.is_some() {
            return Err(fault(
                "LRR-E-MACRO-EXPORT-PUBLICATION",
                "macro source role must precede compiler routes",
            ));
        }
        self.macro_export_role = Some(role);
        Ok(())
    }
    pub(crate) fn macro_export_source_role(
        &self,
    ) -> Option<&std::sync::Arc<crate::db::memo::macro_export_inputs::MacroExportSourceObservation>>
    {
        self.macro_export_role.as_ref()
    }
    pub(crate) fn bind_macro_export_module(
        &mut self,
        module: std::sync::Arc<crate::source::macro_export_module::MacroExportModuleAdoption>,
    ) -> Result<(), Diagnostic> {
        if self.compiler.is_some() || self.macro_export_module.is_some() {
            return Err(fault(
                "LRR-E-MACRO-EXPORT-PUBLICATION",
                "macro-export module must precede compiler routes",
            ));
        }
        let role = self.macro_export_role.as_ref().ok_or_else(|| {
            fault(
                "LRR-E-MACRO-EXPORT-PUBLICATION",
                "macro-export module source role is absent",
            )
        })?;
        module.require_child(&self.source, role)?;
        self.macro_export_module = Some(module);
        Ok(())
    }
    /// Actual retained adoption allocation, for source-owner accounting.
    pub fn macro_export_module_adoption(
        &self,
    ) -> Option<&std::sync::Arc<crate::source::macro_export_module::MacroExportModuleAdoption>>
    {
        self.macro_export_module.as_ref()
    }
    fn require_captured_role_parent(&self, parent: &Self, input: Digest) -> Result<(), Diagnostic> {
        let Some(role) = &self.macro_export_role else {
            return Ok(());
        };
        let module = self.macro_export_module.as_ref().ok_or_else(|| {
            fault(
                "LRR-E-MACRO-EXPORT-PUBLICATION",
                "macro-export child requires adopted source-parent selection",
            )
        })?;
        module.require_child(&self.source, role)?;
        let request = module.request();
        let parent_role = parent.macro_export_role.as_ref().ok_or_else(|| {
            fault(
                "LRR-E-MACRO-EXPORT-PUBLICATION",
                "macro-export adopted parent source role is absent",
            )
        })?;
        parent_role.require_facet(request.parent())?;
        if request.parent().source().selection() != parent.source.selection()
            || !request.parent().source().same_publication(&parent.source)
            || request.declaration().producer() != input
            || request.declaration().scope() != &self.source.outline().root_scope
            || parent
                .index
                .module_input_for(&self.source.outline().root_scope)
                != Some(input)
            || parent
                .macro_export_module
                .as_ref()
                .map(|module| module.identity())
                != request.parent_adoption().map(|module| module.identity())
        {
            return Err(fault(
                "LRR-E-MACRO-EXPORT-PUBLICATION",
                "macro-export adopted source-parent attachment differs",
            ));
        }
        Ok(())
    }
    pub(crate) fn revalidate_captured_role_parents(providers: &[Self]) -> Result<(), Diagnostic> {
        for child in providers {
            let mut owner = child;
            for (ordinal, input) in &child.parents {
                let parent = providers.get(*ordinal).ok_or_else(|| {
                    fault(
                        "LRR-E-MACRO-EXPORT-PUBLICATION",
                        "macro-export captured ancestor is absent",
                    )
                })?;
                owner.require_captured_role_parent(parent, *input)?;
                owner = parent;
            }
            if child.macro_export_role.is_some()
                && !child.source.outline().root_scope.module_path.is_empty()
                && child.parents.is_empty()
            {
                return Err(fault(
                    "LRR-E-MACRO-EXPORT-PUBLICATION",
                    "macro-export child requires adopted source-parent selection",
                ));
            }
        }
        Ok(())
    }
    fn source_prelude_policy(
        &self,
    ) -> Result<crate::syntax::macro_prelude::MacroPreludePolicy, Diagnostic> {
        if self.macro_export_role.is_some() {
            self.source.macro_prelude_facts()
        } else {
            self.source.macro_prelude_policy()
        }
    }
    pub(crate) fn module_source(&self) -> &PreparedSourceView {
        &self.source
    }
    pub(crate) fn collect_helper_counts(
        &self,
        counts: &mut std::collections::BTreeMap<Digest, (usize, usize)>,
    ) -> Result<(), Diagnostic> {
        for root in self.catalogs.values() {
            root.collect_helper_counts(counts, 0)?;
        }
        Ok(())
    }
    pub(crate) fn completed_root_catalog(
        &self,
        invocation: Digest,
    ) -> Result<
        Option<std::sync::Arc<crate::frontend::generated_macros::GeneratedMacroCatalog>>,
        Diagnostic,
    > {
        // Ordinary calls and original derives use distinct storage keys. Match
        // the actual owned invocation rather than guessing the storage key.
        let mut roots = self
            .catalogs
            .values()
            .filter(|catalog| catalog.invocation() == invocation);
        let owner = roots.next().cloned();
        if roots.next().is_some() {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "completed invocation owner is ambiguous",
            ));
        }
        Ok(owner)
    }
    pub(crate) fn completed_catalog_roots(
        &self,
    ) -> impl Iterator<Item = &crate::frontend::generated_macros::GeneratedMacroCatalog> {
        self.catalogs.values().map(std::sync::Arc::as_ref)
    }
    /// A catalogue's captured source context, keyed by its actual storage
    /// identity. Appends retain the immutable prefix preceding that output.
    fn catalogue_context(&self, storage: Digest) -> Result<(&Self, u64), Diagnostic> {
        if let Some((before, prefix)) = self.derive_prefixes.get(&storage) {
            return Ok((prefix.as_ref(), *before));
        }
        let call = self
            .source
            .outline()
            .item_invocations
            .iter()
            .cloned()
            .chain(
                self.source
                    .outline()
                    .items
                    .iter()
                    .filter_map(crate::syntax::outline::potential_attribute_producer),
            )
            .find(|call| call.item_input == storage)
            .ok_or_else(|| {
                fault("LRR-E-EXPANSION-INPUT", "catalogue source producer absent")
            })?;
        Ok((self, u64::from(call.range.start)))
    }
    pub(crate) fn completed_catalogue(
        &self,
        identity: Digest,
    ) -> Result<Option<CompletedCatalogueContext<'_>>, Diagnostic> {
        let mut result = None;
        for (storage, root) in &self.catalogs {
            let mut ancestors = Vec::new();
            if let Some(catalog) = root.find_catalogue(identity, &mut ancestors)? {
                if result.is_some() {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "completed module catalogue ambiguous",
                    ));
                }
                ancestors.reverse();
                let (provider, before) = self.catalogue_context(*storage)?;
                result = Some(CompletedCatalogueContext {
                    catalog,
                    ancestors,
                    provider,
                    before,
                });
            }
        }
        Ok(result)
    }
    pub(crate) fn revalidate_module_catalogs(
        &self,
        owners: &[Self],
        inherited: &InheritedMacroContext<'_>,
    ) -> Result<(), Diagnostic> {
        self.revalidate_module_catalogs_in_context(CapturedMacroContext {
            provider: self,
            owners,
            before: 0,
            inherited: Some(inherited),
            exports: None,
        })
    }
    pub(crate) fn revalidate_module_catalogs_in_context(
        &self,
        context: CapturedMacroContext<'_>,
    ) -> Result<(), Diagnostic> {
        context.require_provider(self)?;
        for (invocation, catalog) in &self.catalogs {
            let (provider, before) = self.catalogue_context(*invocation)?;
            catalog.revalidate_calls(
                CapturedMacroContext {
                    provider,
                    before,
                    ..context
                },
                &[],
            )?;
        }
        Ok(())
    }
    pub(crate) fn profile(&self) -> &crate::core::profile::ProfileIdentity {
        &self.source.outline().profile
    }
    pub(crate) fn reserve_test_attribute(
        &mut self,
        call: &crate::syntax::outline::ItemInvocation,
    ) -> Result<(), Diagnostic> {
        self.index.reserve_test_attribute(call)
    }
    pub(crate) fn macro_index(&self) -> &SourceMacroIndex {
        &self.index
    }
    pub(crate) fn completed_published_index(
        &self,
    ) -> Result<crate::frontend::source_macros::PublishedMacroIndex, Diagnostic> {
        crate::frontend::source_macros::PublishedMacroIndex::from_completed_index(
            self.source.share(),
            self.index.snapshot_clone()?,
        )
    }
    pub(crate) fn completed_runtime_index(
        &self,
        owners: &[Self],
        inherited: Option<&InheritedMacroContext<'_>>,
        budget: usize,
    ) -> Result<crate::frontend::source_macros::PublishedMacroIndex, Diagnostic> {
        if self.macro_export_role.is_some() {
            return Err(fault(
                "LRR-E-MACRO-EXPORT-PUBLICATION",
                "macro-only source cannot publish a runtime index",
            ));
        }
        let mut facts = self.index.hygiene_facts().clone();
        let mut ancestry = self.index.expansion_ancestry().clone();
        let count = self
            .parents
            .len()
            .checked_add(inherited.map_or(0, |parent| parent.outers.len()))            .ok_or_else(|| fault("LRR-E-MACRO-SCOPE", "runtime source ancestry limit"))?;
        let mut outers = Vec::new();
        outers
            .try_reserve_exact(count)
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "runtime source ancestry allocation"))?;
        for (ordinal, input) in &self.parents {
            let parent = owners
                .get(*ordinal)
                .ok_or_else(|| fault("LRR-E-MACRO-SCOPE", "runtime original parent absent"))?;
            facts.extend(parent.index.hygiene_facts())?;
            ancestry.extend(parent.index.expansion_ancestry())?;
            outers.push(parent.index.module_outer(*input)?);
        }
        if let Some(parent) = inherited {
            facts.extend(&parent.facts)?;
            ancestry.extend(&parent.ancestry)?;
            outers.extend(parent.outers.iter().copied());
        }
        crate::frontend::source_macros::PublishedMacroIndex::runtime_snapshot(
            self.source.share(),
            &self.index,
            &facts,
            &ancestry,
            &outers,
            budget,
        )
    }
    pub(crate) fn revalidate_generated_selections(
        &self,
        owners: &[Self],
    ) -> Result<(), Diagnostic> {
        self.revalidate_generated_selections_in_context(CapturedMacroContext {
            provider: self,
            owners,
            before: 0,
            inherited: None,
            exports: None,
        })
    }
    pub(crate) fn revalidate_generated_selections_in_context(
        &self,
        context: CapturedMacroContext<'_>,
    ) -> Result<(), Diagnostic> {
        context.require_provider(self)?;
        let calls = &self.source.outline().item_invocations;
        let count = calls
            .len()
            .checked_add(self.source.outline().items.len())
            .ok_or_else(|| fault("LRR-E-DISCOVERY-LIMIT", "generated owner count overflow"))?;
        count
            .checked_mul(std::mem::size_of::<(Digest, u32)>())
            .ok_or_else(|| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated owner index metadata limit",
                )
            })?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(count)
            .map_err(|_| fault("LRR-E-DISCOVERY-LIMIT", "generated owner index allocation"))?;
        positions.extend(calls.iter().map(|call| (call.item_input, call.range.start)));
        positions.extend(
            self.source
                .outline()
                .items
                .iter()
                .filter_map(crate::syntax::outline::potential_attribute_producer)
                .map(|call| (call.item_input, call.range.start)),
        );
        positions.sort();
        if positions.windows(2).any(|w| w[0].0 == w[1].0) {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "duplicate generated selection owner",
            ));
        }
        for (invocation, catalog) in &self.catalogs {
            if let Some((before, prefix)) = self.derive_prefixes.get(invocation) {
                catalog.revalidate_calls(
                    CapturedMacroContext {
                        provider: prefix,
                        before: *before,
                        ..context
                    },
                    &[],
                )?;
                continue;
            }
            let index = positions
                .binary_search_by_key(invocation, |(id, _)| *id)
                .map_err(|_| fault("LRR-E-EXPANSION-INPUT", "generated selection owner absent"))?;
            catalog.revalidate_calls(
                CapturedMacroContext {
                    before: u64::from(positions[index].1),
                    ..context
                },
                &[],
            )?;
        }
        Ok(())
    }
    /// Build an exact immutable demand from already-selected metadata and owned call tokens.
    pub(crate) fn original_invocation_origin(
        &self,
        invocation: Digest,
    ) -> Result<Option<crate::syntax::origin::SourceOrigin>, Diagnostic> {
        let Some(call) = self
            .source
            .outline()
            .item_invocations
            .iter()
            .find(|call| call.item_input == invocation && call.attribute_input.is_none())
        else {
            return Ok(None);
        };
        let syntax = self.source.macro_call_at(call.range)?;
        let selection = self.source.selection();
        Ok(Some(crate::syntax::origin::SourceOrigin {
            source: crate::syntax::token_wire::SourceIdentity {
                unit: crate::syntax::token_wire::Key(selection.outline.crate_key.0),
                content: crate::syntax::token_wire::Key(selection.outline.parse.content.0),
                path: selection.path.as_str().into(),
            },
            range: syntax.range(),
        }))
    }
    fn original_failure_origin(
        &self,
        definition: Digest,
        invocation: Digest,
        input: &TokenForest,
        site: Key,
    ) -> Result<crate::syntax::origin::MacroOrigin, Diagnostic> {
        use crate::syntax::{
            origin::{DefinitionOrigin, MacroOrigin, SourceOrigin},
            spans::SourceRange,
        };
        let selection = self.source.selection();
        let source = crate::syntax::token_wire::SourceIdentity {
            unit: crate::syntax::token_wire::Key(selection.outline.crate_key.0),
            content: crate::syntax::token_wire::Key(selection.outline.parse.content.0),
            path: selection.path.as_str().into(),
        };
        let invocation_location = match input.spans().iter().find(|s| s.key() == site) {
            Some(span)
                if span.source.unit.0 == source.unit.0
                    && span.source.content.0 == source.content.0
                    && span.source.path == source.path =>
            {
                self.source
                    .macro_call_for_opening_at(SourceRange::checked(span.start, span.end)?)?
                    .map(|call| SourceOrigin {
                        source: source.clone(),
                        range: call.range(),
                    })
            }
            _ => None,
        };
        let definition_location = if self.source.outline().items.iter().any(|i| {
            i.def_key.item_kind == ItemKind::Macro && i.def_key.fingerprint() == definition
        }) {
            let (range, name) = self.source.macro_definition_header(definition)?;
            Some(DefinitionOrigin {
                identity: definition,
                name,
                location: SourceOrigin { source, range },
            })
        } else {
            None
        };
        Ok(MacroOrigin {
            invocation_identity: invocation,
            invocation: invocation_location,
            definition: definition_location,
            failure_kind: None,
            failure_span_key: None,
            failure_span: None,
            parents: Vec::new(),
        })
    }
    pub(crate) fn generated_request(
        &self,
        definition: &crate::expand::scopes::MacroDefinition,
        key: ExpansionKey,
        input: &crate::syntax::token_wire::TokenForest,
        site: crate::syntax::token_wire::Key,
    ) -> Result<SourceDeclarativeRequest, Diagnostic> {
        if self.index.definition(definition.identity) != Some(definition)
            || key.macro_identity
                != (MacroIdentity::Declaration {
                    definition: definition.identity,
                })
            || key.definition_edition != definition.definition_edition
            || key.input_tokens
                != Digest::of_bytes(&input.encode().map_err(|e| fault(e.code, &e.message))?)
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated enclosing definition/key differs",
            ));
        }
        let span = input
            .spans()
            .iter()
            .find(|s| s.key() == site)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated enclosing site absent"))?;
        if key.parent_context != Digest(span.context.0)
            || edition(key.call_edition)?
                != match span.edition {
                    crate::syntax::token_wire::Edition::E2015 => wire::Edition::E2015,
                    crate::syntax::token_wire::Edition::E2018 => wire::Edition::E2018,
                    crate::syntax::token_wire::Edition::E2021 => wire::Edition::E2021,
                    crate::syntax::token_wire::Edition::E2024 => wire::Edition::E2024,
                }
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated enclosing site context/edition differs",
            ));
        }
        key.validate()?;
        let generated = self.generated.get(&definition.identity).cloned();
        let body = if let Some(catalog) = generated {
            DeclarativeBody::Generated {
                catalog,
                definition: definition.identity,
            }
        } else {
            let item = self
                .source
                .outline()
                .items
                .iter()
                .find(|i| {
                    i.def_key.item_kind == ItemKind::Macro
                        && i.def_key.fingerprint() == definition.identity
                })
                .ok_or_else(|| {
                    fault(
                        "LRR-E-EXPANSION-INPUT",
                        "captured enclosing macro body absent",
                    )
                })?;
            self.original_definition_body(item.declaration_span)?
        };
        let wire = input.encode().map_err(|e| fault(e.code, &e.message))?;
        let input = TokenForest::decode(&wire).map_err(adapter)?;
        let origin =
            self.original_failure_origin(definition.identity, key.invocation, &input, Key(site.0))?;
        Ok(SourceDeclarativeRequest {
            key,
            body,
            input,
            site: Key(site.0),
            origin: Some(origin),
            owner: definition.defining_unit,
        })
    }
    pub(crate) fn module_index_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self.index.metadata_bytes()?;
        for name in self.dependencies.keys() {
            bytes = bytes
                .checked_add(name.as_str().len())
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "module source dependency index overflow",
                    )
                })?;
        }
        Ok(bytes)
    }
    pub(crate) fn module_invocation(
        &self,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        inherited: &InheritedMacroContext<'_>,
    ) -> Result<crate::frontend::macro_names::PathLookup, Diagnostic> {
        let call = self
            .source
            .outline()
            .item_invocations
            .iter()
            .find(|c| c.item_input == invocation)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "module invocation absent or inactive",
                )
            })?;
        if let Some(input) = call.attribute_input {
            return Ok(crate::frontend::macro_names::PathLookup::NeedInput(input));
        }
        let syntax = self.source.macro_call_at(call.range)?;
        let mut facts = self.index.hygiene_facts().clone();
        facts.extend(&inherited.facts)?;
        let mut ancestry = self.index.expansion_ancestry().clone();
        ancestry.extend(&inherited.ancestry)?;
        self.index.lookup_invocation_using(
            &syntax,
            namespaces,
            &call.scope,
            |module, before, name| {
                self.index.lookup_with_external(
                    module,
                    before,
                    name,
                    &inherited.outers,
                    &facts,
                    &ancestry,
                )
            },
        )
    }
    pub(crate) fn module_invocation_input(
        &self,
        invocation: Digest,
        definition: &crate::expand::scopes::MacroDefinition,
        compile_env: Digest,
        engine: Digest,
    ) -> Result<
        (
            ExpansionKey,
            crate::syntax::token_wire::TokenForest,
            crate::syntax::token_wire::Key,
        ),
        Diagnostic,
    > {
        let call = self
            .source
            .outline()
            .item_invocations
            .iter()
            .find(|c| c.item_input == invocation && c.attribute_input.is_none())
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "module declarative invocation absent",
                )
            })?;
        let input = group(
            &*self.owned_forest()?,
            self.source.macro_token_group(call.range, false)?,
        )?;
        let [TokenTree::Group { open, .. }] = input.roots() else {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "module argument root differs",
            ));
        };
        let site = crate::syntax::token_wire::Key(open.0);
        let input =
            crate::syntax::token_wire::TokenForest::decode(&input.encode().map_err(adapter)?)
                .map_err(|e| fault(e.code, &e.message))?;
        let span = input
            .spans()
            .iter()
            .find(|s| s.key() == site)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "module invocation origin absent"))?;
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: definition.identity,
            },
            invocation,
            input_tokens: Digest::of_bytes(&input.encode().map_err(|e| fault(e.code, &e.message))?),
            consumer_compile_env: compile_env,
            parent_context: Digest(span.context.0),
            definition_edition: definition.definition_edition,
            call_edition: self.source.selection().outline.parse.profile.edition,
            grammar: GrammarSlot::Items,
            engine_version: engine,
            procedural_epoch: None,
        };
        key.validate()?;
        Ok((key, input, site))
    }
    pub(crate) fn module_compiler_request(
        &self,
        invocation: Digest,
        definition: &crate::expand::scopes::MacroDefinition,
        compile_env: Digest,
        engine: Digest,
        inherited: Option<&InheritedMacroContext<'_>>,
    ) -> Result<Option<SourceDeclarativeRequest>, Diagnostic> {
        let Some(catalog) = &self.compiler else {
            return Ok(None);
        };
        if catalog.select(definition)?.is_none() {
            return Ok(None);
        }
        let (mut key, input, site) =
            self.module_invocation_input(invocation, definition, compile_env, engine)?;
        key.macro_identity = MacroIdentity::Builtin {
            definition: definition.identity,
        };
        key.validate()?;
        let call = self
            .source
            .outline()
            .item_invocations
            .iter()
            .find(|call| call.item_input == invocation)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "compiler invocation absent"))?;
        let index = self
            .index
            .argument_snapshot(&call.scope, u64::from(call.range.start))?;
        let mut facts = index.hygiene_facts().clone();
        let mut ancestry = index.expansion_ancestry().clone();
        let mut outers = Vec::new();
        if let Some(parent) = inherited {
            facts.extend(&parent.facts)?;
            ancestry.extend(&parent.ancestry)?;
            let mut bytes = facts
                .metadata_bytes()?
                .checked_add(ancestry.metadata_bytes()?)
                .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "compiler context overflow"))?;
            for outer in &parent.outers {
                bytes = bytes
                    .checked_add(outer.scopes.metadata_bytes()?)
                    .and_then(|n| n.checked_add(outer.declarations.metadata_bytes().ok()?))
                    .and_then(|n| n.checked_add(std::mem::size_of::<OwnedCompilerOuter>()))
                    .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "compiler context budget"))?;
            }
            outers
                .try_reserve_exact(parent.outers.len())
                .map_err(|_| fault("LRR-E-METADATA-LIMIT", "compiler context allocation"))?;
            outers.extend(parent.outers.iter().map(|outer| OwnedCompilerOuter {
                scopes: outer.scopes.clone(),
                declarations: outer.declarations.clone(),
                scope: outer.scope,
                before: outer.before,
            }));
        }
        Ok(Some(SourceDeclarativeRequest {
            key,
            input: TokenForest::decode(&input.encode().map_err(|e| fault(e.code, &e.message))?)
                .map_err(adapter)?,
            site: Key(site.0),
            origin: None,
            owner: definition.defining_unit,
            body: DeclarativeBody::Compiler(Box::new(CompilerBody {
                catalog: catalog.clone(),
                definition: definition.clone(),
                source: self.source.share(),
                scope: call.scope.clone(),
                index: CompilerIndex::Module(Box::new(index)),
                facts,
                ancestry,
                outers,
            })),
        }))
    }
    pub(crate) fn compiler_scopes(
        &self,
    ) -> impl Iterator<Item = &crate::syntax::outline::ScopeKey> {
        self.source.outline().scopes.keys()
    }
    pub(crate) fn declaration_compiler_request(
        &self,
        index: std::sync::Arc<crate::frontend::impl_index::DeclarationMacroIndex>,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        compile_env: Digest,
        engine: Digest,
    ) -> Result<Option<SourceDeclarativeRequest>, Diagnostic> {
        let Some(definition) = self.declaration_macro_definition(&index, invocation, namespaces)?
        else {
            return Ok(None);
        };
        // Probe only the selected compiler facet. Ordinary source macros build
        // their argument forest in declaration_demand instead.
        let Some(catalog) = &self.compiler else {
            return Ok(None);
        };
        if catalog.select(&definition)?.is_none() {
            return Ok(None);
        }
        let producer = index.producer(invocation)?;
        let input = group(
            &*self.owned_forest()?,
            self.source.macro_token_group(producer.range, false)?,
        )?;
        let [TokenTree::Group { open, .. }] = input.roots() else {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "cold compiler argument root differs",
            ));
        };
        let site = *open;
        self.declaration_compiler_input(
            definition,
            producer,
            &input,
            site,
            crate::frontend::impl_index::DeclarationMacroParent::Source(index.clone()),
            compile_env,
            engine,
        )
    }
    pub(crate) fn argument_compiler_request(
        &self,
        index: std::sync::Arc<crate::frontend::impl_index::ArgumentDeclarationMacroIndex>,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        compile_env: Digest,
        engine: Digest,
    ) -> Result<Option<SourceDeclarativeRequest>, Diagnostic> {
        let Some(definition) = self.argument_macro_definition(&index, invocation, namespaces)?
        else {
            return Ok(None);
        };
        let producer = index
            .argument()
            .scan()
            .producers()
            .iter()
            .find(|producer| producer.identity == invocation)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "argument compiler producer absent"))?;
        let call = index.argument().macro_call(invocation)?;
        self.declaration_compiler_input(
            definition,
            producer,
            &call.input,
            call.site,
            crate::frontend::impl_index::DeclarationMacroParent::Argument(index.clone()),
            compile_env,
            engine,
        )
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "exact compiler binding and lexical source attachment"
    )]
    fn declaration_compiler_input(
        &self,
        definition: crate::expand::scopes::MacroDefinition,
        producer: &crate::syntax::declaration_scan::DeclarationProducer,
        input: &TokenForest,
        site: Key,
        parent: crate::frontend::impl_index::DeclarationMacroParent,
        compile_env: Digest,
        engine: Digest,
    ) -> Result<Option<SourceDeclarativeRequest>, Diagnostic> {
        let Some(catalog) = &self.compiler else {
            return Ok(None);
        };
        if catalog.select(&definition)?.is_none() {
            return Ok(None);
        }
        let selected = self.source.selection();
        let span = input
            .spans()
            .iter()
            .find(|span| span.key() == site)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "compiler declaration site absent"))?;
        // A generated argument may retain an included source identity. The
        // coordinator must supply its exact admitted view before compiler work.
        if span.source.unit.0 != selected.outline.crate_key.0
            || ((span.source.content.0 != selected.outline.parse.content.0
                || span.source.path != selected.path.as_str())
                && parent.compiler_origin().is_none())
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "compiler declaration original source differs",
            ));
        }
        let encoded = input.encode().map_err(adapter)?;
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Builtin {
                definition: definition.identity,
            },
            invocation: producer.identity,
            input_tokens: Digest::of_bytes(&encoded),
            consumer_compile_env: compile_env,
            parent_context: Digest(span.context.0),
            definition_edition: definition.definition_edition,
            call_edition: selected.outline.parse.profile.edition,
            grammar: producer.grammar,
            engine_version: engine,
            procedural_epoch: None,
        };
        key.validate()?;
        let parent = parent.compiler_argument_snapshot(producer.identity)?;
        let facts = parent.facts();
        let ancestry = parent.compiler_ancestry();
        let outer_contexts = parent.compiler_outers()?;
        let mut bytes = facts
            .metadata_bytes()?
            .checked_add(ancestry.metadata_bytes()?)
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "compiler declaration context limit"))?;
        for outer in &outer_contexts {
            bytes = bytes
                .checked_add(outer.scopes.metadata_bytes()?)
                .and_then(|n| n.checked_add(outer.declarations.metadata_bytes().ok()?))
                .and_then(|n| n.checked_add(std::mem::size_of::<OwnedCompilerOuter>()))
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "compiler declaration outer context limit",
                    )
                })?;
        }
        let mut outers = Vec::new();
        outers
            .try_reserve_exact(outer_contexts.len())
            .map_err(|_| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "compiler declaration outer allocation",
                )
            })?;
        outers.extend(outer_contexts.iter().map(|outer| OwnedCompilerOuter {
            scopes: outer.scopes.clone(),
            declarations: outer.declarations.clone(),
            scope: outer.scope,
            before: outer.before,
        }));
        let facts = facts.clone();
        let ancestry = ancestry.clone();
        let mut scope = producer.scope.clone();
        scope.owner = None;
        let mut origin =
            self.original_failure_origin(definition.identity, producer.identity, input, site)?;
        // Nested compiler calls inside opaque outer macro arguments are absent
        // from the physical parser's macro-call index. Their retained registry
        // still carries the exact admitted opening token; preserve that anchor
        // instead of inventing a whole-source call range or losing the parent.
        if origin.invocation.is_none() {
            let limit = || {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "compiler declaration origin storage limit",
                )
            };
            let mut path = String::new();
            path.try_reserve_exact(span.source.path.len())
                .map_err(|_| limit())?;
            path.push_str(&span.source.path);
            origin.invocation = Some(crate::syntax::origin::SourceOrigin {
                source: crate::syntax::token_wire::SourceIdentity {
                    unit: crate::syntax::token_wire::Key(span.source.unit.0),
                    content: crate::syntax::token_wire::Key(span.source.content.0),
                    path,
                },
                range: SourceRange::checked(span.start, span.end)?,
            });
        }
        Ok(Some(SourceDeclarativeRequest {
            key,
            input: TokenForest::decode(&encoded).map_err(adapter)?,
            site,
            owner: definition.defining_unit,
            origin: Some(origin),
            body: DeclarativeBody::Compiler(Box::new(CompilerBody {
                catalog: catalog.clone(),
                definition,
                source: self.source.share(),
                scope,
                index: CompilerIndex::Declaration(parent),
                facts,
                ancestry,
                outers,
            })),
        }))
    }
    pub fn demand(
        &self,
        invocation: Digest,
        compile_env: Digest,
        engine: Digest,
    ) -> Result<SourceDeclarativeDemand, Diagnostic> {
        self.demand_in(invocation, compile_env, engine, &Default::default(), &[])
    }
    /// Explicit cold declaration demand. Reuses the same pinned engine and owned
    /// source tokens; it neither prepares a runtime body nor certifies a catalog.
    pub(crate) fn declaration_parent_frontier(
        &self,
        producer: &crate::syntax::declaration_scan::DeclarationProducer,
    ) -> Result<Option<Digest>, Diagnostic> {
        let mut scope = producer.scope.clone();
        scope.owner = None;
        let outer = self
            .index
            .outer_scope(&scope, u64::from(producer.range.start))?;
        match outer.scopes.local_bindings(outer.scope, outer.before)? {
            crate::expand::scopes::LocalBindings::NeedInput(input) => Ok(Some(input)),
            crate::expand::scopes::LocalBindings::Ready(_) => Ok(None),
        }
    }
    pub(crate) fn declaration_runtime_plan(
        &self,
        index: &crate::frontend::impl_index::DeclarationMacroIndex,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        owners: &[Self],
    ) -> Result<Option<Box<crate::frontend::runtime_macro_plan::RuntimeIntrinsicPlan>>, Diagnostic>
    {
        use crate::frontend::{
            macro_origins::MacroSelection, runtime_macro_plan::RuntimeMacroPreparation,
        };
        let selection = self.source.selection();
        if index.scan().parse() != &selection.outline.parse
            || index.scan().path() != &selection.path
            || index.scan().cfg() != selection.outline.cfg
            || index.scan().scope() != &self.source.outline().root_scope
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "intrinsic declaration source attachment differs",
            ));
        }
        let producer = index.producer(invocation)?;
        let selection = index.lookup(invocation)?;
        let unresolved_qualified = producer.path.len() > 1
            && matches!(selection, HygienicLookup::NeedInput(input) if input == invocation);
        if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Macro
            || !matches!(
                producer.grammar,
                GrammarSlot::Expression | GrammarSlot::Statements
            )
            || !(matches!(selection, HygienicLookup::Selection(MacroSelection::Absent))
                || unresolved_qualified)
        {
            return Ok(None);
        }
        if self.macro_export_role.is_some() {
            return Ok(None);
        }
        let Some(compiler) = &self.compiler else {
            return Ok(None);
        };
        let Some(catalog) = compiler.runtime_catalog()? else {
            return Ok(None);
        };
        let syntax = self.source.macro_call_at(producer.range)?;
        let mut scope = producer.scope.clone();
        scope.owner = None;
        let lookup = self.index.lookup_declaration_invocation_using(
            &syntax,
            namespaces,
            &scope,
            |module, before, name| self.lookup_with_parents(module, before, name, owners),
        )?;
        let RuntimeMacroPreparation::Intrinsic(plan) =
            self.source
                .prepare_runtime_macro(producer.range, lookup, &catalog)?
        else {
            return Ok(None);
        };
        Ok(Some(plan))
    }
    pub(crate) fn declaration_runtime_no_item_proof(
        &self,
        index: &crate::frontend::impl_index::DeclarationMacroIndex,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        owners: &[Self],
    ) -> Result<Option<crate::frontend::runtime_macro_plan::RuntimeNoItemProof>, Diagnostic> {
        let Some(plan) = self.declaration_runtime_plan(index, invocation, namespaces, owners)?
        else {
            return Ok(None);
        };
        let producer = index.producer(invocation)?;
        let body = self
            .source
            .outline()
            .items
            .iter()
            .filter_map(|i| i.body.as_ref())
            .find(|body| {
                producer.range.start >= body.range.start && producer.range.end <= body.range.end
            });
        Ok(body.and_then(|body| plan.no_item_proof(&body.key())))
    }
    pub(crate) fn declaration_runtime_arguments(
        &self,
        index: &crate::frontend::impl_index::DeclarationMacroIndex,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        owners: &[Self],
    ) -> Result<
        Option<Vec<std::sync::Arc<super::argument_declarations::ArgumentDeclarations>>>,
        Diagnostic,
    > {
        let Some(plan) = self.declaration_runtime_plan(index, invocation, namespaces, owners)?
        else {
            return Ok(None);
        };
        let forest = self.owned_forest()?;
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(plan.arguments().declaration_arguments().len())
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "intrinsic argument allocation"))?;
        for range in plan.arguments().declaration_arguments() {
            let tokens = tokens_for_source_range(&forest, *range)?;
            arguments.push(std::sync::Arc::new(
                super::argument_declarations::ArgumentDeclarations::new(
                    index.scan(),
                    invocation,
                    plan.definition().clone(),
                    *range,
                    tokens,
                    self.source.captured_macro_cfg()?,
                )?,
            ));
        }
        Ok(Some(arguments))
    }
    fn declaration_macro_definition(
        &self,
        index: &crate::frontend::impl_index::DeclarationMacroIndex,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
    ) -> Result<Option<crate::expand::scopes::MacroDefinition>, Diagnostic> {
        use crate::frontend::{macro_names::PathLookup, macro_origins::MacroSelection};
        let selected = self.source.selection();
        if index.scan().parse() != &selected.outline.parse
            || index.scan().path() != &selected.path
            || index.scan().cfg() != selected.outline.cfg
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "cold procedural source differs",
            ));
        }
        let producer = index.producer(invocation)?;
        if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Macro {
            return Ok(None);
        }
        let selection = index.lookup(invocation)?;
        let definition = match selection {
            HygienicLookup::Selection(MacroSelection::Defined(definition)) => definition,
            selection
                if matches!(selection, HygienicLookup::Selection(MacroSelection::Absent))
                    || matches!(selection, HygienicLookup::NeedInput(input) if input == invocation && producer.path.len() > 1) =>
            {
                let syntax = self.source.macro_call_at(producer.range)?;
                match index.lookup_imports(
                    invocation,
                    syntax.segments(),
                    syntax.absolute(),
                    namespaces,
                )? {
                    PathLookup::Defined(definition) => definition,
                    PathLookup::Ambiguous(_) => {
                        return Err(fault(
                            "LRR-E-MACRO-AMBIGUOUS",
                            "cold procedural source macro ambiguous",
                        ));
                    }
                    _ => return Ok(None),
                }
            }
            HygienicLookup::Selection(MacroSelection::Ambiguous(_)) => {
                return Err(fault(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "cold procedural source macro ambiguous",
                ));
            }
            _ => return Ok(None),
        };
        Ok(Some(definition))
    }
    pub(crate) fn declaration_procedural_definition(
        &self,
        index: &crate::frontend::impl_index::DeclarationMacroIndex,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
    ) -> Result<Option<crate::expand::scopes::MacroDefinition>, Diagnostic> {
        let Some(definition) = self.declaration_macro_definition(index, invocation, namespaces)?
        else {
            return Ok(None);
        };
        let Some(catalog) = &self.procedural else {
            return Ok(None);
        };
        Ok(catalog
            .select(
                &definition,
                crate::expand::identity::ProceduralKind::Function,
            )?
            .map(|_| definition))
    }
    pub fn generated_function_input(
        &self,
        index: &std::sync::Arc<crate::frontend::impl_index::GeneratedDeclarationMacroIndex>,
        invocation: Digest,
        owners: &[Self],
    ) -> Result<crate::service::function_macro_requests::FunctionInvocationInput, Diagnostic> {
        crate::service::function_macro_requests::FunctionInvocationInput::from_generated_declaration(
            &self.source,
            index.clone(),
            invocation,
            owners,
        )
    }
    pub fn declaration_demand(
        &self,
        index: &crate::frontend::impl_index::DeclarationMacroIndex,
        invocation: Digest,
        compile_env: Digest,
        engine: Digest,
    ) -> Result<SourceDeclarativeDemand, Diagnostic> {
        self.declaration_demand_selected(
            index,
            invocation,
            compile_env,
            engine,
            index.lookup(invocation)?,
        )
    }
    fn declaration_demand_selected(
        &self,
        index: &crate::frontend::impl_index::DeclarationMacroIndex,
        invocation: Digest,
        compile_env: Digest,
        engine: Digest,
        lookup: HygienicLookup,
    ) -> Result<SourceDeclarativeDemand, Diagnostic> {
        use crate::frontend::macro_origins::MacroSelection;
        let scan = index.scan();
        let selection = self.source.selection();
        if scan.parse() != &selection.outline.parse
            || scan.path() != &selection.path
            || scan.scope() != &self.source.outline().root_scope
            || scan.cfg() != selection.outline.cfg
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "cold declaration source attachment differs",
            ));
        }
        let definition = match lookup {
            HygienicLookup::NeedInput(input) => {
                return Ok(SourceDeclarativeDemand::NeedInput(input));
            }
            HygienicLookup::Selection(MacroSelection::Absent) => {
                return Ok(SourceDeclarativeDemand::NeedInput(invocation));
            }
            HygienicLookup::Selection(MacroSelection::Ambiguous(_)) => {
                return Err(fault(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "cold declaration macro is ambiguous",
                ));
            }
            HygienicLookup::Selection(MacroSelection::Defined(definition)) => definition,
        };
        let producer = index.producer(invocation)?;
        let forest = self.owned_forest()?;
        let mut definition_range = None;
        let body = match index.body_owner(definition.identity)? {
            crate::frontend::impl_index::DeclarationBodyOwner::Source(source) => {
                let header = source.definition(definition.identity)?;
                definition_range = Some(header.range);
                let range = self.source.macro_definition_shape_at(header.range)?.body();
                if range != header.body {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "cold macro body attachment differs",
                    ));
                }
                original_definition_body(&self.source, header.range, &forest)?
            }
            crate::frontend::impl_index::DeclarationBodyOwner::Argument(argument) => {
                DeclarativeBody::Source(
                    argument.macro_definition(definition.identity)?.body.clone(),
                )
            }
            crate::frontend::impl_index::DeclarationBodyOwner::Generated(output) => {
                DeclarativeBody::Source(output.macro_definition(definition.identity)?.body.clone())
            }
        };
        let input = group(
            &forest,
            self.source.macro_token_group(producer.range, false)?,
        )?;
        let [TokenTree::Group { open, .. }] = input.roots() else {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "cold macro argument root differs",
            ));
        };
        let site = *open;
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: definition.identity,
            },
            invocation,
            input_tokens: Digest::of_bytes(&input.encode().map_err(adapter)?),
            consumer_compile_env: compile_env,
            parent_context: Digest(
                source_root(
                    selection.outline.crate_key,
                    selection.outline.parse.profile.edition,
                )?
                .context
                .0,
            ),
            definition_edition: definition.definition_edition,
            call_edition: selection.outline.parse.profile.edition,
            grammar: producer.grammar,
            engine_version: engine,
            procedural_epoch: None,
        };
        key.validate()?;
        let mut origin =
            self.original_failure_origin(definition.identity, invocation, &input, site)?;
        if let Some(range) = definition_range {
            let (range, name) = self.source.macro_definition_header_at(range)?;
            let selected = self.source.selection();
            origin.definition = Some(crate::syntax::origin::DefinitionOrigin {
                identity: definition.identity,
                name,
                location: crate::syntax::origin::SourceOrigin {
                    source: crate::syntax::token_wire::SourceIdentity {
                        unit: crate::syntax::token_wire::Key(selected.outline.crate_key.0),
                        content: crate::syntax::token_wire::Key(selected.outline.parse.content.0),
                        path: selected.path.as_str().into(),
                    },
                    range,
                },
            });
        }
        Ok(SourceDeclarativeDemand::Ready(Box::new(
            SourceDeclarativeRequest {
                key,
                body,
                input,
                site,
                origin: Some(origin),
                owner: definition.defining_unit,
            },
        )))
    }
    /// Captured path/prelude fallback after the declaration-local lookup is absent.
    /// Local pending producers and ambiguous bindings retain their existing gate.
    pub(crate) fn declaration_import_request(
        &self,
        index: &crate::frontend::impl_index::DeclarationMacroIndex,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        owners: &[Self],
        compile_env: Digest,
        engine: Digest,
    ) -> Result<SourceDeclarativeDemand, Diagnostic> {
        self.declaration_import_request_in_context(
            index,
            invocation,
            namespaces,
            CapturedMacroContext {
                provider: self,
                owners,
                before: 0,
                inherited: None,
                exports: None,
            },
            compile_env,
            engine,
        )
    }
    pub(crate) fn declaration_import_request_in_context(
        &self,
        index: &crate::frontend::impl_index::DeclarationMacroIndex,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        context: CapturedMacroContext<'_>,
        compile_env: Digest,
        engine: Digest,
    ) -> Result<SourceDeclarativeDemand, Diagnostic> {
        context.require_provider(self)?;
        let owners = context.owners;
        use crate::frontend::{macro_names::PathLookup, macro_origins::MacroSelection};
        let selection = self.source.selection();
        let scan = index.scan();
        if scan.parse() != &selection.outline.parse
            || scan.path() != &selection.path
            || scan.scope() != &self.source.outline().root_scope
            || scan.cfg() != selection.outline.cfg
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "imported declaration source attachment differs",
            ));
        }
        let producer = index.producer(invocation)?;
        // Attribute producers retain their attribute-authority frontier. This
        // fallback only resolves function-like macro calls from captured routes.
        if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Macro {
            return Ok(SourceDeclarativeDemand::NeedInput(invocation));
        }
        let require_module_macro2 = match index.lookup(invocation)? {
            HygienicLookup::Selection(MacroSelection::Absent) => false,
            HygienicLookup::NeedInput(input) if input == invocation => producer.path.len() == 1,
            HygienicLookup::NeedInput(input) => {
                return Ok(SourceDeclarativeDemand::NeedInput(input));
            }
            _ => return Ok(SourceDeclarativeDemand::NeedInput(invocation)),
        };
        let syntax = self.source.macro_call_at(producer.range)?;
        let imported =
            index.lookup_imports(invocation, syntax.segments(), syntax.absolute(), namespaces)?;
        let imported = match imported {
            PathLookup::Defined(definition) if index.contains_definition(definition.identity) => {
                return self.declaration_demand_selected(
                    index,
                    invocation,
                    compile_env,
                    engine,
                    HygienicLookup::Selection(MacroSelection::Defined(definition)),
                );
            }
            PathLookup::Absent => {
                let mut scope = producer.scope.clone();
                scope.owner = None;
                self.index.lookup_declaration_invocation_using(
                    &syntax,
                    namespaces,
                    &scope,
                    |module, before, name| self.lookup_with_parents(module, before, name, owners),
                )?
            }
            found => found,
        };
        let definition = match imported {
            PathLookup::Defined(definition) => definition,
            PathLookup::NeedInput(input) => return Ok(SourceDeclarativeDemand::NeedInput(input)),
            PathLookup::Absent => return Ok(SourceDeclarativeDemand::NeedInput(invocation)),
            PathLookup::Ambiguous(_) => {
                return Err(fault(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "imported declaration macro is ambiguous",
                ));
            }
            PathLookup::Inaccessible(_) => {
                return Err(fault(
                    "LRR-E-MACRO-INACCESSIBLE",
                    "imported declaration macro is inaccessible",
                ));
            }
        };
        if let Some(input) = namespaces.pending_body_owner(definition.identity) {
            return Ok(SourceDeclarativeDemand::NeedInput(input));
        }
        let generated_body = context.inherited_body(&definition);
        let source_owner = if generated_body.is_none() {
            Some(context.definition_owner(&definition)?)
        } else {
            None
        };
        let range = source_owner.and_then(|owner| {
            owner
                .source
                .outline()
                .items
                .iter()
                .find(|item| {
                    item.def_key.item_kind == ItemKind::Macro
                        && item.def_key.fingerprint() == definition.identity
                        && (!require_module_macro2 || item.scope.owner.is_none())
                })
                .map(|item| item.declaration_span)
        });
        // The unresolved lexical slot can settle only the existing actual
        // source Macro2 route, not grant a generated definition by absence.
        if require_module_macro2 && range.is_none() {
            return Ok(SourceDeclarativeDemand::NeedInput(invocation));
        }
        if require_module_macro2
            && source_owner
                .expect("source Macro2 range owner")
                .source
                .macro_definition_shape_at(range.expect("source Macro2 range"))?
                .dialect()
                != crate::syntax::macro_calls::MacroDefinitionDialect::Macro2
        {
            // The current invocation's own unresolved index slot is not a
            // general lexical/import completion certificate. Only an actual
            // original module Macro2 route can settle this distinct producer.
            return Ok(SourceDeclarativeDemand::NeedInput(invocation));
        }
        let body = match (self.library_builtin(&definition), source_owner, range) {
            (Some(kind), _, _) => Some(DeclarativeBody::Builtin {
                kind,
                facts: std::sync::Arc::new(builtin_call_site(
                    &self.source,
                    producer.range.start,
                    selection.outline.parse.profile.edition,
                )?),
            }),
            (None, Some(owner), Some(range)) => Some(owner.original_definition_body(range)?),
            _ if generated_body.is_some() => None,
            _ => return Ok(SourceDeclarativeDemand::NeedInput(invocation)),
        };
        let input = group(
            &*self.owned_forest()?,
            self.source.macro_token_group(producer.range, false)?,
        )?;
        let [TokenTree::Group { open, .. }] = input.roots() else {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "imported declaration argument root differs",
            ));
        };
        let site = *open;
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: definition.identity,
            },
            invocation,
            input_tokens: Digest::of_bytes(&input.encode().map_err(adapter)?),
            consumer_compile_env: compile_env,
            parent_context: Digest(
                source_root(
                    selection.outline.crate_key,
                    selection.outline.parse.profile.edition,
                )?
                .context
                .0,
            ),
            definition_edition: definition.definition_edition,
            call_edition: selection.outline.parse.profile.edition,
            grammar: producer.grammar,
            engine_version: engine,
            procedural_epoch: None,
        };
        key.validate()?;
        if let Some(items) = generated_body {
            let input =
                crate::syntax::token_wire::TokenForest::decode(&input.encode().map_err(adapter)?)
                    .map_err(|e| fault(e.code, &e.message))?;
            let site = crate::syntax::token_wire::Key(site.0);
            return Ok(SourceDeclarativeDemand::Ready(Box::new(
                SourceDeclarativeRequest::module_generated(&definition, key, &input, site, items)?,
            )));
        }
        let owner = source_owner.expect("captured source request owner");
        let body = body.expect("captured source request body");
        let mut origin =
            owner.original_failure_origin(definition.identity, invocation, &input, site)?;
        origin.invocation = self
            .original_failure_origin(definition.identity, invocation, &input, site)?
            .invocation;
        Ok(SourceDeclarativeDemand::Ready(Box::new(
            SourceDeclarativeRequest {
                key,
                body,
                input,
                site,
                origin: Some(origin),
                owner: definition.defining_unit,
            },
        )))
    }
    fn argument_macro_definition(
        &self,
        index: &crate::frontend::impl_index::ArgumentDeclarationMacroIndex,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
    ) -> Result<Option<crate::expand::scopes::MacroDefinition>, Diagnostic> {
        use crate::frontend::{macro_names::PathLookup, macro_origins::MacroSelection};
        let original = index.source_index().scan();
        let selected = self.source.selection();
        if original.parse() != &selected.outline.parse
            || original.path() != &selected.path
            || original.cfg() != selected.outline.cfg
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "argument procedural source differs",
            ));
        }
        let producer = index
            .argument()
            .scan()
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "argument producer absent"))?;
        if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Macro {
            return Ok(None);
        }
        let lookup = index.lookup(invocation)?;
        let qualified = matches!(&lookup,HygienicLookup::NeedInput(input) if *input==invocation)
            && producer.path.len() > 1;
        let absent = matches!(&lookup, HygienicLookup::Selection(MacroSelection::Absent));
        let definition = match lookup {
            HygienicLookup::Selection(MacroSelection::Defined(definition)) => definition,
            HygienicLookup::Selection(MacroSelection::Ambiguous(_)) => {
                return Err(fault(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "argument procedural macro ambiguous",
                ));
            }
            HygienicLookup::Selection(MacroSelection::Absent) | HygienicLookup::NeedInput(_)
                if qualified || absent =>
            {
                let syntax = index.argument().macro_syntax(invocation)?;
                let mut scope = producer.scope.clone();
                scope.owner = None;
                let PathLookup::Defined(definition) = namespaces.lookup_invocation_segments(
                    syntax.segments(),
                    syntax.absolute(),
                    &scope,
                    syntax.key().profile.edition,
                )?
                else {
                    return Ok(None);
                };
                definition
            }
            _ => return Ok(None),
        };
        Ok(Some(definition))
    }
    pub(crate) fn argument_procedural_selection(
        &self,
        index: std::sync::Arc<crate::frontend::impl_index::ArgumentDeclarationMacroIndex>,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        owners: &[Self],
    ) -> Result<Option<super::source_discovery::ProceduralSourceSelection>, Diagnostic> {
        let Some(definition) = self.argument_macro_definition(&index, invocation, namespaces)?
        else {
            return Ok(None);
        };
        let Some(catalog) = &self.procedural else {
            return Ok(None);
        };
        if catalog
            .select(
                &definition,
                crate::expand::identity::ProceduralKind::Function,
            )?
            .is_none()
        {
            return Ok(None);
        }
        let input=crate::service::function_macro_requests::FunctionInvocationInput::from_argument_declaration(&self.source,index,invocation,owners)?;
        Ok(Some(
            super::source_discovery::ProceduralSourceSelection::generated(
                input,
                definition,
                catalog.clone(),
            ),
        ))
    }
    pub fn argument_declaration_demand(
        &self,
        index: &crate::frontend::impl_index::ArgumentDeclarationMacroIndex,
        invocation: Digest,
        compile_env: Digest,
        engine: Digest,
    ) -> Result<SourceDeclarativeDemand, Diagnostic> {
        use crate::frontend::{impl_index::DeclarationBodyOwner, macro_origins::MacroSelection};
        let source = index.source_index().scan();
        let selected = self.source.selection();
        if source.parse() != &selected.outline.parse
            || source.path() != &selected.path
            || source.scope() != &self.source.outline().root_scope
            || source.cfg() != selected.outline.cfg
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "original argument source prefix differs",
            ));
        }
        let definition = match index.lookup(invocation)? {
            HygienicLookup::NeedInput(input) => {
                return Ok(SourceDeclarativeDemand::NeedInput(input));
            }
            HygienicLookup::Selection(MacroSelection::Absent) => {
                return Ok(SourceDeclarativeDemand::NeedInput(invocation));
            }
            HygienicLookup::Selection(MacroSelection::Ambiguous(_)) => {
                return Err(fault(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "original argument macro ambiguous",
                ));
            }
            HygienicLookup::Selection(MacroSelection::Defined(definition)) => definition,
        };
        let mut definition_range = None;
        let body = match index.body_owner(definition.identity)? {
            DeclarationBodyOwner::Argument(argument) => DeclarativeBody::Source(
                argument.macro_definition(definition.identity)?.body.clone(),
            ),
            DeclarationBodyOwner::Generated(output) => {
                DeclarativeBody::Source(output.macro_definition(definition.identity)?.body.clone())
            }
            DeclarationBodyOwner::Source(source) => {
                let header = source.definition(definition.identity)?;
                definition_range = Some(header.range);
                self.original_definition_body(header.range)?
            }
        };
        let call = index.argument().macro_call(invocation)?;
        let producer = index
            .argument()
            .scan()
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "original argument producer absent"))?;
        let site = call
            .input
            .spans()
            .iter()
            .find(|s| s.key() == call.site)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "original argument call site absent",
                )
            })?;
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: definition.identity,
            },
            invocation,
            input_tokens: Digest::of_bytes(&call.input.encode().map_err(adapter)?),
            consumer_compile_env: compile_env,
            parent_context: Digest(site.context.0),
            definition_edition: definition.definition_edition,
            call_edition: call.edition,
            grammar: producer.grammar,
            engine_version: engine,
            procedural_epoch: None,
        };
        key.validate()?;
        let mut origin =
            self.original_failure_origin(definition.identity, invocation, &call.input, call.site)?;
        if let Some(range) = definition_range {
            let (range, name) = self.source.macro_definition_header_at(range)?;
            let selected = self.source.selection();
            origin.definition = Some(crate::syntax::origin::DefinitionOrigin {
                identity: definition.identity,
                name,
                location: crate::syntax::origin::SourceOrigin {
                    source: crate::syntax::token_wire::SourceIdentity {
                        unit: crate::syntax::token_wire::Key(selected.outline.crate_key.0),
                        content: crate::syntax::token_wire::Key(selected.outline.parse.content.0),
                        path: selected.path.as_str().into(),
                    },
                    range,
                },
            });
        }
        Ok(SourceDeclarativeDemand::Ready(Box::new(
            SourceDeclarativeRequest {
                key,
                body,
                input: call.input.clone(),
                site: call.site,
                origin: Some(origin),
                owner: definition.defining_unit,
            },
        )))
    }
    pub(crate) fn generated_runtime_definition(
        &self,
        index: &crate::frontend::impl_index::GeneratedDeclarationMacroIndex,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
    ) -> Result<
        Option<(
            crate::expand::scopes::MacroDefinition,
            crate::facade::macros::RuntimeMacroKind,
        )>,
        Diagnostic,
    > {
        if self.macro_export_role.is_some() {
            return Ok(None);
        }
        let Some(definition) = self.generated_macro_definition(index, invocation, namespaces)?
        else {
            return Ok(None);
        };
        let Some(catalog) = self
            .compiler
            .as_ref()
            .map(|compiler| compiler.runtime_catalog())
            .transpose()?
            .flatten()
        else {
            return Ok(None);
        };
        Ok(catalog.select(&definition)?.map(|kind| (definition, kind)))
    }
    fn generated_macro_definition(
        &self,
        index: &crate::frontend::impl_index::GeneratedDeclarationMacroIndex,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
    ) -> Result<Option<crate::expand::scopes::MacroDefinition>, Diagnostic> {
        use crate::frontend::macro_origins::MacroSelection;
        let source = index.source_index()?.scan();
        let selected = self.source.selection();
        if source.parse() != &selected.outline.parse
            || source.path() != &selected.path
            || source.cfg() != selected.outline.cfg
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated intrinsic source differs",
            ));
        }
        let producer = index
            .output()
            .scan()
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated intrinsic invocation absent",
                )
            })?;
        if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Macro {
            return Ok(None);
        }
        let selected = index.lookup(invocation)?;
        let definition = match selected {
            HygienicLookup::Selection(MacroSelection::Defined(definition)) => definition,
            selection
                if matches!(selection, HygienicLookup::Selection(MacroSelection::Absent))
                    || matches!(selection, HygienicLookup::NeedInput(input) if input == invocation && producer.path.len() > 1) =>
            {
                match generated_path_lookup(index, invocation, &producer.scope, namespaces)? {
                    crate::frontend::macro_names::PathLookup::Defined(definition) => definition,
                    crate::frontend::macro_names::PathLookup::Ambiguous(_) => {
                        return Err(fault(
                            "LRR-E-MACRO-AMBIGUOUS",
                            "generated source macro ambiguous",
                        ));
                    }
                    _ => return Ok(None),
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(definition))
    }
    pub(crate) fn generated_procedural_selection(
        &self,
        index: std::sync::Arc<crate::frontend::impl_index::GeneratedDeclarationMacroIndex>,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        owners: &[Self],
    ) -> Result<Option<super::source_discovery::ProceduralSourceSelection>, Diagnostic> {
        let Some(definition) = self.generated_macro_definition(&index, invocation, namespaces)?
        else {
            return Ok(None);
        };
        let Some(catalog) = &self.procedural else {
            return Ok(None);
        };
        if catalog
            .select(
                &definition,
                crate::expand::identity::ProceduralKind::Function,
            )?
            .is_none()
        {
            return Ok(None);
        }
        let input = self.generated_function_input(&index, invocation, owners)?;
        Ok(Some(
            super::source_discovery::ProceduralSourceSelection::generated(
                input,
                definition,
                catalog.clone(),
            ),
        ))
    }
    /// Pure nested demand from an immutable generated lexical index. Selection
    /// uses captured hygiene/ancestry; execution and adoption stay explicit.
    pub fn generated_declaration_demand(
        &self,
        index: &crate::frontend::impl_index::GeneratedDeclarationMacroIndex,
        invocation: Digest,
        namespaces: Option<&crate::frontend::macro_names::MacroNamespaces>,
    ) -> Result<SourceDeclarativeDemand, Diagnostic> {
        use crate::frontend::{impl_index::DeclarationBodyOwner, macro_origins::MacroSelection};
        let source = index.source_index()?.scan();
        let selection = self.source.selection();
        if source.parse() != &selection.outline.parse
            || source.path() != &selection.path
            || source.scope() != &self.source.outline().root_scope
            || source.cfg() != selection.outline.cfg
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated declaration source prefix differs",
            ));
        }
        use crate::frontend::macro_names::PathLookup;
        let producer = index.producer(invocation)?;
        let path_lookup = || -> Result<PathLookup, Diagnostic> {
            match namespaces {
                Some(namespaces) => {
                    generated_path_lookup(index, invocation, &producer.scope, namespaces)
                }
                None => Ok(PathLookup::NeedInput(invocation)),
            }
        };
        let selected = match index.lookup(invocation)? {
            HygienicLookup::NeedInput(input) if input == invocation => path_lookup()?,
            HygienicLookup::NeedInput(input) => PathLookup::NeedInput(input),
            HygienicLookup::Selection(MacroSelection::Absent) => path_lookup()?,
            HygienicLookup::Selection(MacroSelection::Ambiguous(definitions)) => {
                PathLookup::Ambiguous(definitions)
            }
            HygienicLookup::Selection(MacroSelection::Defined(definition)) => {
                PathLookup::Defined(definition)
            }
        };
        let definition = match selected {
            PathLookup::Defined(definition) => definition,
            PathLookup::NeedInput(input) => return Ok(SourceDeclarativeDemand::NeedInput(input)),
            PathLookup::Absent => return Ok(SourceDeclarativeDemand::NeedInput(invocation)),
            PathLookup::Ambiguous(_) => {
                return Err(fault(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "generated declaration macro is ambiguous",
                ));
            }
            PathLookup::Inaccessible(_) => {
                return Err(fault(
                    "LRR-E-MACRO-INACCESSIBLE",
                    "generated declaration macro is inaccessible",
                ));
            }
        };
        let output = index.output();
        let call = output.macro_call(invocation)?;
        let builtin = self.library_builtin(&definition);
        let body = match index.body_owner(definition.identity) {
            _ if builtin.is_some() => {
                // `line!`/`column!` use the outermost original call site: the
                // original producer that started this generated frame.
                let root = output.parent_invocation();
                let at = source
                    .producers()
                    .iter()
                    .find(|producer| producer.identity == root)
                    .map_or(0, |producer| producer.range.start);
                DeclarativeBody::Builtin {
                    kind: builtin.expect("builtin checked"),
                    facts: std::sync::Arc::new(builtin_call_site(
                        &self.source,
                        at,
                        call.edition,
                    )?),
                }
            }
            Ok(DeclarationBodyOwner::Argument(argument)) => DeclarativeBody::Source(
                argument.macro_definition(definition.identity)?.body.clone(),
            ),
            Ok(DeclarationBodyOwner::Generated(output)) => {
                DeclarativeBody::Source(output.macro_definition(definition.identity)?.body.clone())
            }
            Ok(DeclarationBodyOwner::Source(source)) => {
                let header = source.definition(definition.identity)?;
                self.original_definition_body(header.range)?
            }
            // A definition outside this generated frame (another crate's export,
            // including the standard library) keeps its own original source.
            Err(error) => match self.foreign_definition_body(&definition)? {
                Some(body) => body,
                None => return Err(error),
            },
        };
        let producer = output
            .scan()
            .producers()
            .iter()
            .find(|p| p.identity == invocation)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated declaration producer absent",
                )
            })?;
        let site = call
            .input
            .spans()
            .iter()
            .find(|s| s.key() == call.site)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "generated declaration argument site absent",
                )
            })?;
        let parent = output.record().key();
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: definition.identity,
            },
            invocation,
            input_tokens: Digest::of_bytes(&call.input.encode().map_err(adapter)?),
            consumer_compile_env: parent.consumer_compile_env,
            parent_context: Digest(site.context.0),
            definition_edition: definition.definition_edition,
            call_edition: call.edition,
            grammar: producer.grammar,
            engine_version: parent.engine_version,
            procedural_epoch: None,
        };
        key.validate()?;
        Ok(SourceDeclarativeDemand::Ready(Box::new(
            SourceDeclarativeRequest {
                key,
                body,
                input: call.input.clone(),
                site: call.site,
                origin: None,
                owner: definition.defining_unit,
            },
        )))
    }
    /// The adapter builtin for a library `#[rustc_builtin_macro]` definition.
    fn library_builtin(
        &self,
        definition: &crate::expand::scopes::MacroDefinition,
    ) -> Option<lrr_expand_adapter::builtin_macros::BuiltinKind> {
        use lrr_expand_adapter::builtin_macros::{BuiltinDisposition, builtin_kind};
        let library = self.compiler.as_ref()?.library()?;
        let kind = builtin_kind(library.builtin(definition)?)?;
        (kind.disposition() == BuiltinDisposition::Adapter).then_some(kind)
    }
    /// The original body of a definition owned by this source or by the
    /// standard library sources of its compiler facet.
    fn foreign_definition_body(
        &self,
        definition: &crate::expand::scopes::MacroDefinition,
    ) -> Result<Option<DeclarativeBody>, Diagnostic> {
        let mut remaining = usize::MAX;
        let library = self.compiler.as_ref().and_then(|catalog| catalog.library());
        for owner in std::iter::once(self).chain(
            library
                .into_iter()
                .flat_map(|library| library.providers().iter()),
        ) {
            let Some(owner) = owner.retained_definition_owner(definition, 0, &mut remaining)? else {
                continue;
            };
            let Some(item) = owner.source.outline().items.iter().find(|item| {
                item.def_key.item_kind == ItemKind::Macro
                    && item.def_key.fingerprint() == definition.identity
            }) else {
                continue;
            };
            return owner.original_definition_body(item.declaration_span).map(Some);
        }
        Ok(None)
    }
    pub(crate) fn install_procedural(
        &mut self,
        catalog: std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
    ) -> Result<(), Diagnostic> {
        if self.source.outline().root_scope.crate_key != catalog.consumer()
            || self.procedural.is_some()
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "procedural source consumer/configuration differs",
            ));
        }
        let mut aliases = (*self.dependencies).clone();
        for (alias, unit) in catalog.aliases() {
            if aliases.get(alias).is_some_and(|old| old != unit) {
                return Err(fault(
                    "LRR-E-MACRO-SCOPE",
                    "prepared procedural alias conflicts or exceeds limit",
                ));
            }
            aliases.insert(alias.clone(), *unit);
        }
        self.dependencies = std::sync::Arc::new(aliases);
        self.procedural = Some(catalog);
        Ok(())
    }
    pub(crate) fn resolved_function_call(
        &self,
        invocation: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
    ) -> Result<crate::frontend::macro_names::PathLookup, Diagnostic> {
        let call = self
            .source
            .outline()
            .item_invocations
            .iter()
            .find(|call| call.item_input == invocation && call.attribute_input.is_none())
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "active function-like source invocation absent",
                )
            })?;
        self.index.lookup_invocation(
            &self.source.macro_call_at(call.range)?,
            namespaces,
            &call.scope,
        )
    }
    pub(crate) fn routes_in(
        &self,
        owners: &[Self],
    ) -> Result<
        Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
        Diagnostic,
    > {
        self.routes_using(CapturedMacroContext {
            provider: self,
            owners,
            before: 0,
            inherited: None,
            exports: None,
        })
    }
    pub(crate) fn module_routes(
        &self,
        owners: &[Self],
        inherited: &InheritedMacroContext<'_>,
    ) -> Result<
        Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
        Diagnostic,
    > {
        self.routes_using(CapturedMacroContext {
            provider: self,
            owners,
            before: 0,
            inherited: Some(inherited),
            exports: None,
        })
    }
    pub(crate) fn routes_in_context(
        &self,
        context: CapturedMacroContext<'_>,
    ) -> Result<
        Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
        Diagnostic,
    > {
        self.routes_using(context)
    }
    /// Captured Cargo aliases plus the installed owned compiler facets. This
    /// is pure route metadata and grants no expansion or runtime authority.
    pub(crate) fn route_dependency_sets(&self) -> Result<RouteDependencySets, Diagnostic> {
        self.route_dependency_sets_for_standard(self.source_prelude_policy()?.standard)
    }
    fn route_dependency_sets_for_standard(
        &self,
        standard: bool,
    ) -> Result<RouteDependencySets, Diagnostic> {
        let mut available = (*self.dependencies).clone();
        let mut dependencies = available.clone();
        if self.macro_export_role.is_none()
            && let Some(catalog) = &self.compiler
        {
            for (alias, unit) in catalog.facade_aliases() {
                let name = crate::expand::scopes::MacroName::parse(
                    alias,
                    self.source.outline().profile.edition,
                )?;
                available.entry(name.clone()).or_insert(unit);
                if alias != "std" || standard {
                    dependencies.entry(name).or_insert(unit);
                }
            }
        }
        Ok((dependencies, available))
    }
    /// The route catalogue of this provider's own source. It is recomputed only
    /// when one of the inputs in `OriginalRoutesKey` changed: every nested
    /// expansion would otherwise rebuild the catalogue of every provider.
    /// `LRR_VERIFY_ROUTE_CACHE` recomputes on each hit and faults on a stale one.
    fn original_routes(
        &self,
        owners: &[Self],
        inherited: Option<&InheritedMacroContext<'_>>,
    ) -> Result<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>, Diagnostic>
    {
        if inherited.is_some() {
            return Ok(std::sync::Arc::new(
                self.compute_original_routes(owners, inherited)?,
            ));
        }
        let mut parents = Vec::new();
        parents
            .try_reserve_exact(self.parents.len())
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "route parent allocation failed"))?;
        for (ordinal, input) in &self.parents {
            let parent = owners.get(*ordinal).ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-SCOPE",
                    "captured module parent catalogue absent",
                )
            })?;
            parents.push((*ordinal, *input, parent.index.stamp()));
        }
        let key = OriginalRoutesKey {
            index: self.index.stamp(),
            parents,
            dependencies: self.dependencies.clone(),
            compiler: self.compiler.clone(),
            procedural: self.procedural.clone(),
            role: self.macro_export_role.clone(),
            preparation: self.compiler_attribute_preparation.clone(),
        };
        let cached = self
            .original_routes
            .lock()
            .map_err(|_| fault("LRR-E-INTERNAL", "original route cache poisoned"))?
            .as_ref()
            .filter(|entry| entry.key.same(&key))
            .map(|entry| entry.routes.clone());
        if let Some(routes) = cached {
            static VERIFY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            if *VERIFY.get_or_init(|| std::env::var_os("LRR_VERIFY_ROUTE_CACHE").is_some())
                && !routes.same_routes(&self.compute_original_routes(owners, None)?)
            {
                return Err(fault(
                    "LRR-E-INTERNAL",
                    "original route cache differs from a fresh computation",
                ));
            }
            return Ok(routes);
        }
        let routes = std::sync::Arc::new(self.compute_original_routes(owners, None)?);
        *self
            .original_routes
            .lock()
            .map_err(|_| fault("LRR-E-INTERNAL", "original route cache poisoned"))? =
            Some(OriginalRoutes {
                key,
                routes: routes.clone(),
            });
        Ok(routes)
    }
    fn compute_original_routes(
        &self,
        owners: &[Self],
        inherited: Option<&InheritedMacroContext<'_>>,
    ) -> Result<crate::frontend::source_macro_routes::SourceMacroRoutes, Diagnostic> {
        let policy = self.source_prelude_policy()?;
        let (dependencies, available) = self.route_dependency_sets_for_standard(policy.standard)?;
        let disabled = policy
            .disabled
            .into_iter()
            .map(|suffix| {
                let mut scope = self.source.outline().root_scope.clone();
                scope.module_path.extend(suffix);
                scope
            })
            .collect::<Vec<_>>();
        let mut original =
            crate::frontend::source_macro_routes::SourceMacroRoutes::from_lookup_preparing(
                self.source.outline(),
                &dependencies,
                &available,
                &disabled,
                |id| self.index.definition(id).cloned(),
                |module, at, name| {
                    if let Some(parent) = inherited {
                        let mut facts = self.index.hygiene_facts().clone();
                        facts.extend(&parent.facts)?;
                        let mut ancestry = self.index.expansion_ancestry().clone();
                        ancestry.extend(&parent.ancestry)?;
                        self.index.lookup_with_external(
                            module,
                            at,
                            name,
                            &parent.outers,
                            &facts,
                            &ancestry,
                        )
                    } else {
                        self.lookup_with_parents(module, at, name, owners)
                    }
                },
                self.compiler_attribute_preparation.as_deref(),
            )?;
        original.export_pending = self.index.pending_producers().collect();
        original.include_restored_original_macros(self.source.outline(), &self.index)?;
        Ok(original)
    }
    fn routes_using(
        &self,
        context: CapturedMacroContext<'_>,
    ) -> Result<
        Vec<std::sync::Arc<crate::frontend::source_macro_routes::SourceMacroRoutes>>,
        Diagnostic,
    > {
        context.require_provider(self)?;
        let owners = context.owners;
        let inherited = context.inherited;
        let mut routes = Vec::new();
        routes
            .try_reserve_exact(
                self.catalogs.len().checked_add(1).ok_or_else(|| {
                    fault("LRR-E-METADATA-LIMIT", "route catalogue count overflow")
                })?,
            )
            .map_err(|_| fault("LRR-E-METADATA-LIMIT", "route catalogue allocation failed"))?;
        routes.push(self.original_routes(owners, inherited)?);
        let mut bytes = routes[0].metadata_bytes()?;
        for (invocation, catalog) in &self.catalogs {
            let (provider, before) = self.catalogue_context(*invocation)?;
            let route = catalog.routes(CapturedMacroContext {
                provider,
                before,
                ..context
            })?;
            bytes = bytes
                .checked_add(route.metadata_bytes()?)
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "generated route catalogue metadata budget",
                    )
                })?;
            routes.push(std::sync::Arc::new(route));
            for child in catalog.completed_child_routes() {
                bytes = bytes
                    .checked_add(child.metadata_bytes()?)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-METADATA-LIMIT",
                            "child route catalogue metadata budget",
                        )
                    })?;
                routes.try_reserve(1).map_err(|_| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "child route catalogue allocation failed",
                    )
                })?;
                routes.push(child);
            }
        }
        Ok(routes)
    }
    pub(crate) fn demand_in(
        &self,
        invocation: Digest,
        compile_env: Digest,
        engine: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        owners: &[Self],
    ) -> Result<SourceDeclarativeDemand, Diagnostic> {
        self.demand_using(
            invocation,
            compile_env,
            engine,
            namespaces,
            CapturedMacroContext {
                provider: self,
                owners,
                before: 0,
                inherited: None,
                exports: None,
            },
            false,
        )
    }
    /// Only pure unsealed discovery may use an available candidate across frontiers.
    pub(crate) fn demand_for_discovery(
        &self,
        invocation: Digest,
        compile_env: Digest,
        engine: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        owners: &[Self],
    ) -> Result<SourceDeclarativeDemand, Diagnostic> {
        self.demand_for_discovery_in_context(
            invocation,
            compile_env,
            engine,
            namespaces,
            CapturedMacroContext {
                provider: self,
                owners,
                before: 0,
                inherited: None,
                exports: None,
            },
        )
    }
    pub(crate) fn demand_for_discovery_in_context(
        &self,
        invocation: Digest,
        compile_env: Digest,
        engine: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        context: CapturedMacroContext<'_>,
    ) -> Result<SourceDeclarativeDemand, Diagnostic> {
        self.demand_using(invocation, compile_env, engine, namespaces, context, true)
    }
    fn demand_using(
        &self,
        invocation: Digest,
        compile_env: Digest,
        engine: Digest,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        context: CapturedMacroContext<'_>,
        discovery: bool,
    ) -> Result<SourceDeclarativeDemand, Diagnostic> {
        context.require_provider(self)?;
        let owners = context.owners;
        let call = self
            .source
            .outline()
            .item_invocations
            .iter()
            .find(|c| c.item_input == invocation)
            .ok_or_else(|| {
                fault(
                    "LRR-E-EXPANSION-INPUT",
                    "source macro invocation is absent or inactive",
                )
            })?;
        if let Some(input) = call.attribute_input {
            return Ok(SourceDeclarativeDemand::NeedInput(input));
        }
        let syntax = self.source.macro_call_at(call.range)?;
        use crate::frontend::macro_names::PathLookup;
        let definition = match self.index.lookup_invocation_using(
            &syntax,
            namespaces,
            &call.scope,
            |module, before, name| {
                self.lookup_with_parents_using(module, before, name, owners, discovery)
            },
        )? {
            PathLookup::Defined(definition) => definition,
            PathLookup::NeedInput(input) => return Ok(SourceDeclarativeDemand::NeedInput(input)),
            PathLookup::Absent => return Ok(SourceDeclarativeDemand::NeedInput(invocation)),
            PathLookup::Ambiguous(_) => {
                return Err(fault(
                    "LRR-E-MACRO-AMBIGUOUS",
                    "ambiguous source declarative definition",
                ));
            }
            PathLookup::Inaccessible(_) => {
                return Err(fault(
                    "LRR-E-MACRO-INACCESSIBLE",
                    "source declarative definition is inaccessible",
                ));
            }
        };
        if !self.index.owns_definition(definition.identity)
            && let Some(producer) = namespaces.pending_body_owner(definition.identity)
        {
            return Ok(SourceDeclarativeDemand::NeedInput(producer));
        }
        if !syntax.absolute() && syntax.segments().len() == 1 {
            let Some(crate::syntax::imports::ImportSegment::Name(name)) = syntax.segments().last()
            else {
                return Err(fault("LRR-E-EXPANSION-INPUT", "bare macro name absent"));
            };
            if matches!(self.lookup_with_parents_using(&call.scope.module_path,u64::from(call.range.start),name,owners,discovery)?,HygienicLookup::Selection(crate::frontend::macro_origins::MacroSelection::Defined(ref found)) if found==&definition)
            {
                let owner = std::iter::once(self)
                    .chain(owners.iter())
                    .find(|owner| owner.index.definition(definition.identity) == Some(&definition))
                    .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "textual origin owner absent"))?;
                let origin = owner
                    .index
                    .definition_origin(&definition)?
                    .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "textual origin facts absent"))?;
                if origin.is_some() {
                    let mut ancestry = owner.index.expansion_ancestry().clone();
                    let other = match namespaces.lookup_invocation_path(&syntax, &call.scope)? {
                        PathLookup::Defined(other) if other != definition => Some(other),
                        PathLookup::NeedInput(input) => {
                            return Ok(SourceDeclarativeDemand::NeedInput(input));
                        }
                        PathLookup::Ambiguous(_) => {
                            return Err(fault(
                                "LRR-E-MACRO-AMBIGUOUS",
                                "textual macro conflicts with ambiguous path bindings",
                            ));
                        }
                        _ => None,
                    };
                    if let Some(other) = other {
                        let path = namespaces
                            .source_call_path(&syntax, &call.scope)?
                            .ok_or_else(|| {
                                fault("LRR-E-EXPANSION-INPUT", "path binding origin route absent")
                            })?;
                        for binding in
                            namespaces.binding_origins_from(&path, &other, &call.scope)?
                        {
                            if let Some(key) = binding
                                && !ancestry.contains(key)
                            {
                                let provider = std::iter::once(self)
                                    .chain(owners.iter())
                                    .find(|p| p.index.expansion_ancestry().contains(key))
                                    .ok_or_else(|| {
                                        fault(
                                            "LRR-E-EXPANSION-INPUT",
                                            "path binding origin ancestry absent",
                                        )
                                    })?;
                                ancestry.extend(provider.index.expansion_ancestry())?;
                            }
                            let candidates = [
                                crate::frontend::macro_origins::MacroCandidate {
                                    definition: definition.clone(),
                                    origin,
                                },
                                crate::frontend::macro_origins::MacroCandidate {
                                    definition: other.clone(),
                                    origin: binding,
                                },
                            ];
                            if matches!(
                                ancestry.select_compatible(None, &candidates)?,
                                crate::frontend::macro_origins::MacroSelection::Ambiguous(_)
                            ) {
                                return Err(fault(
                                    "LRR-E-MACRO-AMBIGUOUS",
                                    "source textual macro conflicts with an imported binding from another expansion origin",
                                ));
                            }
                        }
                    }
                }
            }
        }
        if let Some(request) =
            self.module_compiler_request(invocation, &definition, compile_env, engine, None)?
        {
            return Ok(SourceDeclarativeDemand::Ready(Box::new(request)));
        }
        let inherited_body = context.inherited_body(&definition);
        let source_owner = if inherited_body.is_none() {
            Some(context.definition_owner(&definition)?)
        } else {
            None
        };
        let generated =
            source_owner.and_then(|owner| owner.generated.get(&definition.identity).cloned());
        let body_range = source_owner.and_then(|owner| {
            owner
                .source
                .outline()
                .items
                .iter()
                .find(|i| {
                    i.def_key.item_kind == ItemKind::Macro
                        && i.def_key.fingerprint() == definition.identity
                })
                .map(|i| i.declaration_span)
        });
        if body_range.is_none() && generated.is_none() && inherited_body.is_none() {
            return Err(fault("LRR-E-EXPANSION-INPUT", "resolved macro body absent"));
        }
        let input_range = self.source.macro_token_group(call.range, false)?;
        let selection = self.source.selection();
        let forest = self.owned_forest()?;
        let parent_context = Digest(
            source_root(
                selection.outline.crate_key,
                selection.outline.parse.profile.edition,
            )?
            .context
            .0,
        );
        let body = match body_range {
            Some(range) => Some(
                source_owner
                    .expect("captured definition range owner")
                    .original_definition_body(range)?,
            ),
            None => generated.map(|catalog| DeclarativeBody::Generated {
                catalog,
                definition: definition.identity,
            }),
        };
        let input = group(&forest, input_range)?;
        let [TokenTree::Group { open, .. }] = input.roots() else {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "source macro argument root differs",
            ));
        };
        let site = *open;
        let key = ExpansionKey {
            macro_identity: MacroIdentity::Declaration {
                definition: definition.identity,
            },
            invocation,
            input_tokens: Digest::of_bytes(&input.encode().map_err(adapter)?),
            consumer_compile_env: compile_env,
            parent_context,
            definition_edition: definition.definition_edition,
            call_edition: selection.outline.parse.profile.edition,
            grammar: GrammarSlot::Items,
            engine_version: engine,
            procedural_epoch: None,
        };
        key.validate()?;
        if let Some(items) = inherited_body {
            let input =
                crate::syntax::token_wire::TokenForest::decode(&input.encode().map_err(adapter)?)
                    .map_err(|e| fault(e.code, &e.message))?;
            let site = crate::syntax::token_wire::Key(site.0);
            return Ok(SourceDeclarativeDemand::Ready(Box::new(
                SourceDeclarativeRequest::module_generated(&definition, key, &input, site, items)?,
            )));
        }
        let owner = source_owner.expect("captured selected definition owner");
        let body = body.expect("captured selected definition body");
        let mut origin =
            owner.original_failure_origin(definition.identity, invocation, &input, site)?;
        origin.invocation = self
            .original_failure_origin(definition.identity, invocation, &input, site)?
            .invocation;
        Ok(SourceDeclarativeDemand::Ready(Box::new(
            SourceDeclarativeRequest {
                key,
                body,
                input,
                site,
                origin: Some(origin),
                owner: definition.defining_unit,
            },
        )))
    }
    fn original_definition_body(
        &self,
        selected: SourceRange,
    ) -> Result<DeclarativeBody, Diagnostic> {
        original_definition_body(&self.source, selected, &*self.owned_forest()?)
    }

    pub(crate) fn owned_forest(&self) -> Result<std::sync::Arc<TokenForest>, Diagnostic> {
        self.owned_forest_handle().get()
    }
    /// The shared, lazily tokenized forest of this provider's source.
    pub(crate) fn owned_forest_handle(&self) -> OwnedForestHandle {
        OwnedForestHandle {
            source: self.source.share(),
            cell: self.owned_forest.clone(),
        }
    }
}
/// A provider source and its once-tokenized owned forest, usable after the
/// provider itself is no longer borrowed.
pub(crate) struct OwnedForestHandle {
    source: PreparedSourceView,
    cell: std::sync::Arc<std::sync::OnceLock<std::sync::Arc<TokenForest>>>,
}
impl OwnedForestHandle {
    /// A handle with its own (not provider-shared) forest cell.
    pub(crate) fn detached(source: PreparedSourceView) -> Self {
        Self {
            source,
            cell: Default::default(),
        }
    }
    pub(crate) fn source(&self) -> &PreparedSourceView {
        &self.source
    }
    pub(crate) fn get(&self) -> Result<std::sync::Arc<TokenForest>, Diagnostic> {
        if let Some(forest) = self.cell.get() {
            return Ok(forest.clone());
        }
        let forest = std::sync::Arc::new(tokenize_owned_forest(&self.source)?);
        Ok(self.cell.get_or_init(|| forest).clone())
    }
}
fn tokenize_owned_forest(view: &PreparedSourceView) -> Result<TokenForest, Diagnostic> {
    {
        let selection = view.selection();
        let source = wire::SourceIdentity {
            unit: Key(selection.outline.crate_key.0),
            content: Key(selection.outline.parse.content.0),
            path: selection.path.as_str().into(),
        };
        let context = wire::ContextRecord {
            parent: None,
            expansion: None,
            transparency: wire::Transparency::Opaque,
            defining_unit: source.unit,
            edition: edition(selection.outline.parse.profile.edition)?,
        };
        let forest = wire::parse_source(
            view.captured_macro_blob()?.text()?,
            source,
            context,
            Key(crate::db::key::QueryKey::Outline(selection.outline.clone())
                .fingerprint()
                .0),
        )
        .map_err(adapter)?;
        forest
            .encoded_len()
            .map_err(adapter)?;
        Ok(forest)
    }
}
impl SourceDeclarativeRequest {
    pub(crate) fn procedural(
        request: std::sync::Arc<crate::service::function_macro_requests::PreparedFunctionRequest>,
        owner: Digest,
    ) -> Result<Self, Diagnostic> {
        Ok(Self {
            key: request.key().clone(),
            input: request.request().input.clone(),
            site: request.request().call_site,
            origin: None,
            owner,
            body: DeclarativeBody::Procedural(request),
        })
    }
    /// A builtin derive occurrence over its exact item input (`input` has one
    /// root group of item tokens and carries the derive path span `site`).
    pub(crate) fn builtin_derive(
        key: ExpansionKey,
        input: TokenForest,
        site: Key,
        owner: Digest,
        kind: lrr_expand_adapter::builtin_macros::BuiltinDeriveKind,
        facts: lrr_expand_adapter::builtin_macros::DeriveContainerFacts,
    ) -> Result<Self, Diagnostic> {
        key.validate()?;
        if !matches!(key.macro_identity, MacroIdentity::BuiltinDerive { .. })
            || key.grammar != GrammarSlot::Items
            || !input.spans().iter().any(|span| span.key() == site)
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "builtin derive request identity, grammar or site differs",
            ));
        }
        Ok(Self {
            key,
            input,
            site,
            origin: None,
            owner,
            body: DeclarativeBody::BuiltinDerive { kind, facts },
        })
    }
    pub fn procedural_request(&self) -> Option<&lrr_expand_adapter::proc_client::ProcInvocation> {
        match &self.body {
            DeclarativeBody::Procedural(request) => Some(request.request()),
            _ => None,
        }
    }
    pub(crate) fn module_generated(
        definition: &crate::expand::scopes::MacroDefinition,
        key: ExpansionKey,
        input: &crate::syntax::token_wire::TokenForest,
        site: crate::syntax::token_wire::Key,
        items: std::sync::Arc<super::generated_items::GeneratedItems>,
    ) -> Result<Self, Diagnostic> {
        if items.macro_edition(definition.identity)? != definition.definition_edition {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "module definition edition differs",
            ));
        }
        Ok(Self {
            key,
            input: TokenForest::decode(&input.encode().map_err(|e| fault(e.code, &e.message))?)
                .map_err(adapter)?,
            site: Key(site.0),
            origin: None,
            owner: definition.defining_unit,
            body: DeclarativeBody::OwnedGenerated(items, definition.identity),
        })
    }
    pub(crate) fn with_origin_parents(
        mut self,
        parents: Vec<crate::syntax::origin::ExpansionOrigin>,
    ) -> Result<Self, Diagnostic> {
        let mut bytes = parents
            .capacity()
            .checked_mul(std::mem::size_of::<crate::syntax::origin::ExpansionOrigin>())
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "diagnostic origin size overflow"))?;
        for parent in &parents {
            if let Some(at) = &parent.invocation {
                bytes = bytes
                    .checked_add(at.source.path.capacity())
                    .ok_or_else(|| {
                        fault("LRR-E-METADATA-LIMIT", "diagnostic origin retention limit")
                    })?;
            }
        }
        if let Some(origin) = &mut self.origin {
            origin.parents = parents;
        }
        Ok(self)
    }
    pub fn key(&self) -> &ExpansionKey {
        &self.key
    }
    pub(crate) fn source_base(&self) -> Result<crate::syntax::token_wire::SpanRecord, Diagnostic> {
        let input =
            crate::syntax::token_wire::TokenForest::decode(&self.input.encode().map_err(adapter)?)
                .map_err(|e| fault(e.code, &e.message))?;
        input
            .spans()
            .iter()
            .find(|span| span.key().0 == self.site.0)
            .cloned()
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "source invocation site absent"))
    }
    pub fn is_compiler(&self) -> bool {
        matches!(self.body, DeclarativeBody::Compiler(_))
    }
    pub(crate) fn included_view(
        &self,
        db: &mut crate::db::memo::Database,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        environment: &crate::expand::builtins::FrozenCompileEnvironment,
        authorities: &crate::source::include_paths::IncludeAuthorities,
        caller_include: Option<&crate::source::include_view::IncludedSourceView>,
    ) -> Result<Option<crate::source::include_view::IncludedSourceView>, Diagnostic> {
        let mut view = None;
        self.compiler_work(
            db,
            namespaces,
            environment,
            authorities,
            caller_include,
            Some(&mut view),
        )?;
        Ok(view)
    }
    pub(crate) fn poll_compiler(
        &self,
        db: &mut crate::db::memo::Database,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        environment: &crate::expand::builtins::FrozenCompileEnvironment,
        authorities: &crate::source::include_paths::IncludeAuthorities,
        caller_include: Option<&crate::source::include_view::IncludedSourceView>,
    ) -> Result<crate::service::eager_compile_inputs::EagerCompileInputPoll, Diagnostic> {
        self.compiler_work(
            db,
            namespaces,
            environment,
            authorities,
            caller_include,
            None,
        )
    }
    fn compiler_work(
        &self,
        db: &mut crate::db::memo::Database,
        namespaces: &crate::frontend::macro_names::MacroNamespaces,
        environment: &crate::expand::builtins::FrozenCompileEnvironment,
        authorities: &crate::source::include_paths::IncludeAuthorities,
        caller_include: Option<&crate::source::include_view::IncludedSourceView>,
        view: Option<&mut Option<crate::source::include_view::IncludedSourceView>>,
    ) -> Result<crate::service::eager_compile_inputs::EagerCompileInputPoll, Diagnostic> {
        let DeclarativeBody::Compiler(body) = &self.body else {
            return Err(fault(
                "LRR-E-COMPILE-INPUT",
                "pending request is not a compiler input",
            ));
        };
        let CompilerBody {
            catalog,
            definition,
            source,
            scope,
            index,
            facts,
            ancestry,
            outers,
        } = body.as_ref();
        let input =
            crate::syntax::token_wire::TokenForest::decode(&self.input.encode().map_err(adapter)?)
                .map_err(|e| fault(e.code, &e.message))?;
        let span = input
            .spans()
            .iter()
            .find(|s| s.key().0 == self.site.0)
            .ok_or_else(|| fault("LRR-E-COMPILE-INPUT", "compiler site absent"))?;
        let from =
            match caller_include {
                Some(captured) => {
                    if captured.location().unit.0 != span.source.unit.0
                        || captured.blob().content().0 != span.source.content.0
                        || captured.location().path.as_str() != span.source.path
                        || captured.origin() != span.origin
                        || captured.caller_context() != span.context
                        || !captured
                            .tokens()
                            .spans()
                            .iter()
                            .any(|original| original == span)
                        || !captured.matches_authorities(authorities)
                    {
                        return Err(fault(
                            "LRR-E-INCLUDE-SNAPSHOT",
                            "compiler declaration included caller differs",
                        ));
                    }
                    match captured.validate(db) {
                        crate::db::poll::QueryPoll::Ready(result) => result?,
                        crate::db::poll::QueryPoll::NeedInput(input) => return Ok(
                            crate::service::eager_compile_inputs::EagerCompileInputPoll::NeedInput(
                                input,
                            ),
                        ),
                    }
                    captured.location().clone()
                }
                None => {
                    let selected = source.selection();
                    if span.source.unit.0 != selected.outline.crate_key.0
                        || span.source.content.0 != selected.outline.parse.content.0
                        || span.source.path != selected.path.as_str()
                    {
                        return Err(fault(
                            "LRR-E-INCLUDE-SNAPSHOT",
                            "compiler declaration requires an admitted included caller",
                        ));
                    }
                    crate::source::include_paths::IncludeLocation {
                        unit: scope.crate_key,
                        root: authorities.unique_source_root()?,
                        path: selected.path.clone(),
                        origin: span.origin,
                    }
                }
            };
        let (outer, origin) = match index {
            CompilerIndex::Module(index) => {
                (index.outer_scope(scope, u64::from(span.start))?, None)
            }
            CompilerIndex::Declaration(parent) => {
                (parent.outer(self.key.invocation)?, parent.compiler_origin())
            }
        };
        let mut caller_source = source.selection().outline.parse.clone();
        caller_source.content = Digest(span.source.content.0);
        caller_source.profile.edition = self.key.call_edition;
        let caller = crate::frontend::nested_compile_inputs::NestedLexicalContext {
            source: caller_source,
            scope: scope.clone(),
            textual_scope: outer.scope,
            origin,
        };
        let resolver = crate::frontend::nested_compile_inputs::NestedCompileInputResolver {
            namespaces,
            textual: outer.scopes,
            declarations: outer.declarations,
            hygiene: facts,
            ancestry,
            compiler: catalog,
        };
        let borrowed = outers
            .iter()
            .map(|outer| crate::frontend::macro_scopes::OuterMacroScope {
                scopes: &outer.scopes,
                declarations: &outer.declarations,
                scope: outer.scope,
                before: outer.before,
            })
            .collect::<Vec<_>>();
        let resolver = crate::frontend::nested_compile_inputs::InheritedCompileInputResolver {
            inner: resolver,
            outers: &borrowed,
            lexical_before: match index {
                CompilerIndex::Module(_) => None,
                CompilerIndex::Declaration(_) => Some(outer.before),
            },
        };
        let request = crate::service::eager_compile_inputs::EagerCompilerRequest {
            selected: definition,
            key: &self.key,
            original: &input,
            site: crate::syntax::token_wire::Key(self.site.0),
            source_revision: db.source_revision(),
            limits: Default::default(),
        };
        let sources = crate::service::eager_compile_inputs::EagerSourceContext {
            authorities,
            from: &from,
            included: caller_include,
        };
        if let Some(view) = view {
            match crate::service::eager_compile_inputs::adopted_eager_include_view(
                db,
                request,
                &resolver,
                &caller,
                environment,
                sources,
            ) {
                crate::db::poll::QueryPoll::Ready(result) => *view = result?,
                crate::db::poll::QueryPoll::NeedInput(_) => {
                    return Err(fault(
                        "LRR-E-COMPILE-INPUT",
                        "include view requires adopted inputs",
                    ));
                }
            }
            return Ok(
                crate::service::eager_compile_inputs::EagerCompileInputPoll::Complete(
                    match db.input(&crate::db::dependency::InputKey::ExpansionRecord(
                        self.key.fingerprint(),
                    )) {
                        Some(input) => match input.as_ref() {
                            crate::db::dependency::InputValue::ExpansionRecord(record) => {
                                record.clone()
                            }
                            _ => {
                                return Err(fault(
                                    "LRR-E-COMPILE-INPUT",
                                    "include view expansion variant differs",
                                ));
                            }
                        },
                        None => {
                            return Err(fault(
                                "LRR-E-COMPILE-INPUT",
                                "include view expansion absent",
                            ));
                        }
                    },
                ),
            );
        }
        crate::service::eager_compile_inputs::poll_eager_compile_input_with_sources(
            db,
            request,
            &resolver,
            &caller,
            environment,
            sources,
        )
    }

    /// The coordinator has revalidated this exact eager demand. Resolve its body
    /// only through the retained declaration index; no body query or tool runs here.
    pub(crate) fn expand_declaration_compiler_operand(
        &self,
        request: &crate::service::eager_compile_inputs::EagerExternalCall,
    ) -> Result<ExpansionRecord, Diagnostic> {
        use crate::frontend::impl_index::DeclarationBodyOwner;
        use crate::syntax::origin::{DefinitionOrigin, MacroOrigin, SourceOrigin};
        let DeclarativeBody::Compiler(compiler) = &self.body else {
            return Err(fault(
                "LRR-E-COMPILE-INPUT",
                "compiler operand parent differs",
            ));
        };
        let CompilerIndex::Declaration(parent) = &compiler.index else {
            return Err(fault(
                "LRR-E-COMPILE-INPUT",
                "compiler operand declaration index absent",
            ));
        };
        let selected = request.selected();
        let key = request.key();
        if key.grammar != GrammarSlot::Expression
            || key.macro_identity
                != (MacroIdentity::Declaration {
                    definition: selected.identity,
                })
            || key.definition_edition != selected.definition_edition
            || key.consumer_compile_env != self.key.consumer_compile_env
            || key.engine_version != self.key.engine_version
            || key.procedural_epoch.is_some()
            || selected.defining_unit != compiler.source.outline().root_scope.crate_key
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "compiler operand definition/key differs",
            ));
        }
        let original = request
            .original()
            .encode()
            .map_err(|e| fault(e.code, &e.message))?;
        if Digest::of_bytes(&original) != key.input_tokens {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "compiler operand input differs or exceeds budget",
            ));
        }
        let input = TokenForest::decode(&original).map_err(adapter)?;
        let site = input
            .spans()
            .iter()
            .find(|span| span.key().0 == request.site().0)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "compiler operand call site absent"))?;
        if Digest(site.context.0) != key.parent_context
            || edition(key.call_edition)? != site.edition
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "compiler operand call context/edition differs",
            ));
        }
        let captured_definition =
            |capture: &lrr_expand_adapter::declaration_output::DefinitionCapture,
             name: &str,
             location: SourceOrigin|
             -> Result<DefinitionOrigin, Diagnostic> {
                // Declaration ownership and physical source can differ for
                // native-produced tokens; retain the exact captured source.
                if capture.edition != selected.definition_edition {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "compiler operand captured definition identity/edition differs",
                    ));
                }
                name.len()
                    .checked_add(location.source.path.len())
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-METADATA-LIMIT",
                            "compiler operand definition origin budget",
                        )
                    })?;
                let mut owned_name = String::new();
                owned_name.try_reserve_exact(name.len()).map_err(|_| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "compiler operand definition name allocation",
                    )
                })?;
                owned_name.push_str(name);
                Ok(DefinitionOrigin {
                    identity: selected.identity,
                    name: owned_name,
                    location,
                })
            };
        let (body, definition) = match parent.body_owner(selected.identity)? {
            DeclarationBodyOwner::Source(index) => {
                let source = &compiler.source;
                let scan = index.scan();
                let selection = source.selection();
                if scan.parse() != &selection.outline.parse
                    || scan.path() != &selection.path
                    || scan.scope() != &source.outline().root_scope
                    || scan.cfg() != selection.outline.cfg
                    || scan.parse().profile.edition != selected.definition_edition
                {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "compiler operand source owner differs",
                    ));
                }
                let header = index.definition(selected.identity)?;
                let range = source.macro_definition_shape_at(header.range)?.body();
                if range != header.body {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "compiler operand source body differs",
                    ));
                }
                let forest = super::argument_declarations::ArgumentDeclarations::source_tokens(
                    selection,
                    source.captured_macro_blob()?,
                )?;
                let (location, name) = source.macro_definition_header_at(header.range)?;
                (
                    original_definition_body(source, header.range, &forest)?,
                    DefinitionOrigin {
                        identity: selected.identity,
                        name,
                        location: SourceOrigin {
                            source: crate::syntax::token_wire::SourceIdentity {
                                unit: crate::syntax::token_wire::Key(selection.outline.crate_key.0),
                                content: crate::syntax::token_wire::Key(
                                    selection.outline.parse.content.0,
                                ),
                                path: selection.path.as_str().into(),
                            },
                            range: location,
                        },
                    },
                )
            }
            DeclarationBodyOwner::Argument(argument) => {
                let capture = argument.macro_definition(selected.identity)?;
                capture
                    .body
                    .encoded_len()
                    .map_err(adapter)?;
                let header = argument
                    .scan()
                    .macro_definitions()
                    .iter()
                    .find(|header| header.identity == selected.identity)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-EXPANSION-INPUT",
                            "compiler operand argument definition absent",
                        )
                    })?;
                (
                    DeclarativeBody::Source(capture.body.clone()),
                    captured_definition(
                        capture,
                        &header.name,
                        argument.definition_name_origin(selected.identity)?,
                    )?,
                )
            }
            DeclarationBodyOwner::Generated(output) => {
                let capture = output.macro_definition(selected.identity)?;
                capture
                    .body
                    .encoded_len()
                    .map_err(adapter)?;
                let header = output
                    .scan()
                    .macro_definitions()
                    .iter()
                    .find(|header| header.identity == selected.identity)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-EXPANSION-INPUT",
                            "compiler operand generated definition absent",
                        )
                    })?;
                let span = output
                    .tokens()
                    .spans()
                    .iter()
                    .find(|span| span.key().0 == capture.name_span.0)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-EXPANSION-INPUT",
                            "compiler operand generated name span absent",
                        )
                    })?;
                let location = SourceOrigin {
                    source: span.source.clone(),
                    range: SourceRange::checked(span.start, span.end)?,
                };
                (
                    DeclarativeBody::Source(capture.body.clone()),
                    captured_definition(capture, &header.name, location)?,
                )
            }
        };
        let origin = MacroOrigin {
            invocation_identity: key.invocation,
            invocation: Some(SourceOrigin {
                source: crate::syntax::token_wire::SourceIdentity {
                    unit: crate::syntax::token_wire::Key(site.source.unit.0),
                    content: crate::syntax::token_wire::Key(site.source.content.0),
                    path: site.source.path.clone(),
                },
                range: SourceRange::checked(site.start, site.end)?,
            }),
            definition: Some(definition),
            failure_kind: None,
            failure_span_key: None,
            failure_span: None,
            parents: vec![],
        };
        let mut parents = Vec::new();
        if let Some(parent_origin) = &self.origin {
            let count = parent_origin
                .parents
                .len()
                .checked_add(1)                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "compiler operand origin depth limit",
                    )
                })?;
            parents
                .try_reserve_exact(count)
                .map_err(|_| fault("LRR-E-METADATA-LIMIT", "compiler operand origin allocation"))?;
            parents.extend(parent_origin.parents.iter().cloned());
            parents.push(crate::syntax::origin::ExpansionOrigin {
                expansion: self.key.fingerprint(),
                invocation_identity: Some(self.key.invocation),
                invocation: parent_origin.invocation.clone(),
            });
        }
        let request = Self {
            key: key.clone(),
            body,
            input,
            site: Key(request.site().0),
            owner: selected.defining_unit,
            origin: Some(origin),
        };
        request.with_origin_parents(parents)?.expand()
    }

    pub fn expand(&self) -> Result<ExpansionRecord, Diagnostic> {
        let (body, arguments) = match &self.body {
            DeclarativeBody::Macro2 { arguments, body } => (body, Some(arguments)),
            DeclarativeBody::Procedural(_) => {
                return Err(fault(
                    "LRR-E-MACRO-PERMISSION",
                    "procedural source demand requires authorized coordinator execution",
                ));
            }
            DeclarativeBody::Compiler(_) => {
                return Err(fault(
                    "LRR-E-COMPILE-INPUT",
                    "compiler expansion requires explicit capture and adoption",
                ));
            }
            DeclarativeBody::OwnedGenerated(items, definition) => {
                let input = crate::syntax::token_wire::TokenForest::decode(
                    &self.input.encode().map_err(adapter)?,
                )
                .map_err(|e| fault(e.code, &e.message))?;
                return items.expand_macro_body(
                    *definition,
                    self.owner,
                    &self.key,
                    &input,
                    crate::syntax::token_wire::Key(self.site.0),
                );
            }
            DeclarativeBody::Source(body) => (body, None),
            DeclarativeBody::Builtin { kind, facts } => {
                use lrr_expand_adapter::builtin_macros::{BuiltinSite, expand_builtin};
                let output = expand_builtin(
                    *kind,
                    &self.input,
                    self.site,
                    &BuiltinSite {
                        expansion: Key(self.key.fingerprint().0),
                        defining_unit: Key(self.owner.0),
                        definition_edition: edition(self.key.definition_edition)?,
                    },
                    facts.as_ref(),
                )
                .map_err(adapter)?
                .map_err(|failure| {
                    fault(
                        match failure.kind {
                            lrr_expand_adapter::builtin_macros::BuiltinFailureKind::CompileError => {
                                "LRR-E-COMPILE-ERROR"
                            }
                            lrr_expand_adapter::builtin_macros::BuiltinFailureKind::Unsupported => {
                                "LRR-E-UNSUPPORTED"
                            }
                            _ => "LRR-E-MACRO-EXPANSION",
                        },
                        &failure.message,
                    )
                })?;
                return ExpansionRecord::from_wire(
                    self.key.clone(),
                    output.tokens.encode().map_err(adapter)?,
                );
            }
            DeclarativeBody::Generated {
                catalog,
                definition,
            } => {
                let input = crate::syntax::token_wire::TokenForest::decode(
                    &self.input.encode().map_err(adapter)?,
                )
                .map_err(|e| fault(e.code, &e.message))?;
                return catalog.expand(
                    *definition,
                    &self.key,
                    &input,
                    crate::syntax::token_wire::Key(self.site.0),
                );
            }
            DeclarativeBody::BuiltinDerive { kind, facts } => {
                use lrr_expand_adapter::builtin_macros::{BuiltinSite, expand_builtin_derive};
                let output = expand_builtin_derive(
                    *kind,
                    &self.input,
                    self.site,
                    &BuiltinSite {
                        expansion: Key(self.key.fingerprint().0),
                        defining_unit: Key(self.owner.0),
                        definition_edition: edition(self.key.definition_edition)?,
                    },
                    *facts,
                )
                .map_err(adapter)?
                .map_err(|failure| {
                    fault(
                        match failure.kind {
                            lrr_expand_adapter::builtin_macros::BuiltinFailureKind::Unsupported => {
                                "LRR-E-UNSUPPORTED"
                            }
                            _ => "LRR-E-MACRO-EXPANSION",
                        },
                        &format!("`#[derive({})]`: {}", kind.name(), failure.message),
                    )
                })?;
                return ExpansionRecord::from_wire(
                    self.key.clone(),
                    output.tokens.encode().map_err(adapter)?,
                );
            }
        };
        let definition = DeclarativeMacro::shared(arguments, body).map_err(adapter)?;
        let (result, _) = definition
            .expand_hygienic_with_limits(
                &self.input,
                self.site,
                DeclarativeSite {
                    expansion: Key(self.key.fingerprint().0),
                    defining_unit: Key(self.owner.0),
                    definition_edition: edition(self.key.definition_edition)?,
                },
                usize::MAX,
                usize::MAX,
            )
            .map_err(adapter)?;
        let output = result.map_err(|e| {
            use crate::syntax::origin::MacroFailureKind as Owned;
            use lrr_expand_adapter::declarative::FailureKind as Adapter;
            let mut diagnostic = fault("LRR-E-MACRO-EXPANSION", &e.to_string());
            let mut origin = self
                .origin
                .clone()
                .unwrap_or(crate::syntax::origin::MacroOrigin {
                    invocation_identity: self.key.invocation,
                    invocation: None,
                    definition: None,
                    failure_kind: None,
                    failure_span_key: None,
                    failure_span: None,
                    parents: Vec::new(),
                });
            origin.failure_kind = Some(match e.kind {
                Adapter::DefinitionSyntax => Owned::DefinitionSyntax,
                Adapter::NoMatchingRule => Owned::NoMatchingRule,
                Adapter::LeftoverTokens => Owned::LeftoverTokens,
                Adapter::Binding => Owned::Binding,
                Adapter::UnresolvedBinding => Owned::UnresolvedBinding,
                Adapter::UnexpectedToken => Owned::UnexpectedToken,
                Adapter::Limit => Owned::Limit,
            });
            origin.failure_span_key = e.span.map(|k| crate::syntax::token_wire::Key(k.0));
            if let Some(span) = e.span.and_then(|key| {
                self.input
                    .spans()
                    .iter()
                    .chain(body.spans())
                    .find(|span| span.key() == key)
            }) {
                // Decode the stable owned schema, never retain an upstream span handle.
                origin.failure_span = Some(crate::syntax::token_wire::SpanRecord {
                    source: crate::syntax::token_wire::SourceIdentity {
                        unit: crate::syntax::token_wire::Key(span.source.unit.0),
                        content: crate::syntax::token_wire::Key(span.source.content.0),
                        path: span.source.path.clone(),
                    },
                    start: span.start,
                    end: span.end,
                    context: crate::syntax::token_wire::Key(span.context.0),
                    edition: match span.edition {
                        wire::Edition::E2015 => crate::syntax::token_wire::Edition::E2015,
                        wire::Edition::E2018 => crate::syntax::token_wire::Edition::E2018,
                        wire::Edition::E2021 => crate::syntax::token_wire::Edition::E2021,
                        wire::Edition::E2024 => crate::syntax::token_wire::Edition::E2024,
                    },
                    origin: crate::syntax::token_wire::Key(span.origin.0),
                });
            }
            diagnostic.macro_origin = Some(Box::new(origin));
            diagnostic
        })?;
        ExpansionRecord::from_wire(self.key.clone(), output.tokens.encode().map_err(adapter)?)
    }
}
/// Captured physical sources for actual emitted spans; no filesystem reads occur.
pub(crate) fn captured_procedural_sources(
    input: &crate::syntax::token_wire::TokenForest,
    providers: &[SourceDeclarative],
) -> Result<Vec<lrr_expand_adapter::proc_client::CapturedSpanSource>, Diagnostic> {
    use lrr_expand_adapter::proc_client::{CapturedSpanSource, SourceDigestEncoding};
    let mut sources = std::collections::BTreeMap::new();
    let mut bytes = input
        .allocated_bytes()
        .map_err(|e| fault("LRR-E-METADATA-LIMIT", &e.message))?;
    for provider in providers {
        let forest = provider.owned_forest()?;
        let Some(base) = forest.spans().first() else {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated procedural provider span absent",
            ));
        };
        if !input.spans().iter().any(|span| {
            span.source.unit.0 == base.source.unit.0
                && span.source.content.0 == base.source.content.0
                && span.source.path == base.source.path
        }) {
            continue;
        }
        let key = (
            base.source.unit.0,
            base.source.content.0,
            base.source.path.clone(),
        );
        if sources.contains_key(&key) {
            continue;
        }
        let text = provider.source.captured_macro_blob()?.text()?;
        bytes = bytes
            .checked_add(text.len())
            .and_then(|n| n.checked_add(base.source.path.len().checked_mul(3)?))
            .and_then(|n| n.checked_add(std::mem::size_of::<CapturedSpanSource>() + 256))
            .ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "generated procedural source capture limit",
                )
            })?;
        let captured = CapturedSpanSource::new(
            base.source.clone(),
            text.into(),
            SourceDigestEncoding::LrrSourceBytes,
            None,
        )
        .map_err(adapter)?;
        sources.insert(key, captured);
    }
    if input.spans().iter().any(|span| {
        !sources.contains_key(&(
            span.source.unit.0,
            span.source.content.0,
            span.source.path.clone(),
        ))
    }) {
        return Err(fault(
            "LRR-E-EXPANSION-INPUT",
            "generated procedural span source is uncaptured",
        ));
    }
    let mut result = Vec::new();
    result.try_reserve_exact(sources.len()).map_err(|_| {
        fault(
            "LRR-E-METADATA-LIMIT",
            "generated procedural source vector allocation",
        )
    })?;
    result.extend(sources.into_values());
    Ok(result)
}

#[cfg(test)]
mod generated_runtime_tests {
    use super::*;
}

#[cfg(test)]
#[path = "source_declarative/macro2_tests.rs"]
mod macro2_tests;
