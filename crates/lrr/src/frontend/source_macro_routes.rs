//! Original source contributions to path lookup. Membership seals and visibility
//! checks remain the caller's responsibility; these are not a complete namespace.
use super::{
    hygiene::HygieneFacts,
    macro_names::{MacroNamespaces, MacroPath},
    macro_origins::MacroSelection,
    macro_scopes::HygienicLookup,
    source_macros::SourceMacroIndex,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    expand::scopes::{MacroDefinition, MacroName},
    syntax::{
        imports::{ImportKind, ImportSegment, SourceImport},
        outline::{AttributeClass, Outline, ScopeKey, TextualEventKind, Visibility},
        token_wire::Key,
    },
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DeferredImportReason {
    Glob,
    ModuleAlias,
    AttributeProducer,
    TextualProducer(Digest),
    TextualAmbiguity,
    DependencyModuleCollision,
    ModuleLookup,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DeferredImport {
    pub source: SourceImport,
    pub reason: DeferredImportReason,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DefinitionRoute {
    pub path: MacroPath,
    pub definition: MacroDefinition,
    pub visibility: Visibility,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ImportRoute {
    pub path: MacroPath,
    pub target: MacroPath,
    pub source: SourceImport,
}
/// A declaration remains distinct even when another declaration names the same unit.
#[derive(Debug, Clone)]
pub(crate) struct ExternAlias {
    pub declaration: Digest,
    pub scope: ScopeKey,
    pub name: Option<MacroName>,
    pub provider: Digest,
    pub visibility: Visibility,
    pub macro_use: Option<crate::syntax::imports::MacroUseRecipe>,
    pub order: Vec<u32>,
}
impl StableEncode for ExternAlias {
    const DOMAIN: &'static str = "lrr/v2/source-extern-alias";
    fn encode_fields(&self, e: &mut crate::core::digest::StableEncoder) {
        e.digest(self.declaration);
        e.record(&self.scope);
        e.option(&self.name, |e, name| e.string(name.as_str()));
        e.digest(self.provider);
        e.record(&self.visibility);
        e.option(&self.macro_use, |e, recipe| e.record(recipe));
        e.sequence(&self.order, |e, position| e.u32(*position));
    }
}
pub struct SourceMacroRoutes {
    pub(crate) binding_origin: Option<Key>,
    pub(super) definitions: Vec<DefinitionRoute>,
    pub(super) imports: Vec<ImportRoute>,
    pub(super) deferred: Vec<DeferredImport>,
    pub(crate) module_access:
        BTreeMap<(Digest, Vec<MacroName>), crate::syntax::visibility::VisibilityScope>,
    pub(crate) edition: u16,
    pub(crate) extern_aliases: Vec<ExternAlias>,
    pub(crate) export_pending: BTreeSet<Digest>,
    pub(crate) body_owner_pending: BTreeMap<Digest, Digest>,
    pub(crate) modules: BTreeSet<(Digest, Vec<MacroName>)>,
    pub(crate) dependencies: BTreeMap<MacroName, Digest>,
    pub(crate) source_imports: Vec<SourceImport>,
    pub(crate) textual_import_definitions: BTreeMap<Digest, MacroDefinition>,
    pub(crate) implicit_disabled: BTreeSet<ScopeKey>,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
pub(crate) fn semantic(name: &str, edition: u16) -> Result<MacroName, Diagnostic> {
    // Logical module names have already lost raw spelling. Revalidate them as
    // raw identifiers through the pinned parser, including reserved `gen`.
    MacroName::parse(&format!("r#{name}"), edition)
}
pub(crate) fn module_names(names: &[String], edition: u16) -> Result<Vec<MacroName>, Diagnostic> {
    let mut result = Vec::new();
    result
        .try_reserve(names.len())
        .map_err(|_| error("macro route allocation failed"))?;
    for name in names {
        result.push(semantic(name, edition)?);
    }
    Ok(result)
}
impl SourceMacroRoutes {
    pub(crate) fn owned_copy(&self) -> Result<Self, Diagnostic> {
        if self.metadata_bytes()? > 64 * 1024 * 1024 {
            return Err(error("owned macro route metadata limit"));
        }
        Ok(Self {
            binding_origin: self.binding_origin,
            definitions: self.definitions.clone(),
            imports: self.imports.clone(),
            deferred: self.deferred.clone(),
            module_access: self.module_access.clone(),
            edition: self.edition,
            extern_aliases: self.extern_aliases.clone(),
            export_pending: self.export_pending.clone(),
            body_owner_pending: self.body_owner_pending.clone(),
            modules: self.modules.clone(),
            dependencies: self.dependencies.clone(),
            source_imports: self.source_imports.clone(),
            textual_import_definitions: self.textual_import_definitions.clone(),
            implicit_disabled: self.implicit_disabled.clone(),
        })
    }
    pub(crate) fn defer_body_owners(
        &mut self,
        unit: Digest,
        producer: Digest,
    ) -> Result<(), Diagnostic> {
        self.metadata_bytes()?
            .checked_add(
                self.definitions
                    .len()
                    .checked_mul(96)
                    .ok_or_else(|| error("pending body owner metadata overflow"))?,
            )
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(|| error("pending body owner metadata limit"))?;
        for route in &self.definitions {
            if route.definition.defining_unit == unit {
                if self.body_owner_pending.len() >= 100_000 {
                    return Err(error("pending macro body owner count limit"));
                }
                self.body_owner_pending
                    .insert(route.definition.identity, producer);
            }
        }
        self.metadata_bytes()?;
        Ok(())
    }
    /// SDK inventory is a compiled macro-export facet, not a source parser or helper list.
    pub fn from_procedural_catalog(
        catalog: &super::procedural_exports::PreparedProcCatalog,
    ) -> Result<Vec<Self>, Diagnostic> {
        let mut units = BTreeMap::new();
        for export in catalog.exports() {
            let definition = export.definition();
            let routes = units
                .entry(definition.defining_unit)
                .or_insert_with(|| Self {
                    binding_origin: None,
                    definitions: vec![],
                    imports: vec![],
                    deferred: vec![],
                    module_access: BTreeMap::from([(
                        (definition.defining_unit, vec![]),
                        crate::syntax::visibility::VisibilityScope::Public,
                    )]),
                    edition: definition.definition_edition,
                    extern_aliases: vec![],
                    export_pending: BTreeSet::new(),
                    body_owner_pending: BTreeMap::new(),
                    modules: BTreeSet::from([(definition.defining_unit, vec![])]),
                    dependencies: BTreeMap::new(),
                    source_imports: vec![],
                    textual_import_definitions: BTreeMap::new(),
                    implicit_disabled: BTreeSet::new(),
                });
            if routes.edition != definition.definition_edition {
                return Err(error("compiled macro unit has conflicting editions"));
            }
            routes.definitions.push(DefinitionRoute {
                path: MacroPath::new(definition.defining_unit, vec![export.name().clone()])?,
                definition: definition.clone(),
                visibility: Visibility::Public,
            });
        }
        Ok(units.into_values().collect())
    }
    pub fn from_outline(
        outline: &Outline,
        context: Key,
        facts: HygieneFacts,
        dependencies: &BTreeMap<MacroName, Digest>,
    ) -> Result<Self, Diagnostic> {
        Self::from_outline_scoped(outline, context, facts, dependencies, &[])
    }
    pub(crate) fn from_outline_scoped(
        outline: &Outline,
        context: Key,
        facts: HygieneFacts,
        dependencies: &BTreeMap<MacroName, Digest>,
        disabled: &[ScopeKey],
    ) -> Result<Self, Diagnostic> {
        let index = SourceMacroIndex::from_outline(outline, context, facts)?;
        Self::from_completed_index(outline, &index, dependencies, disabled)
    }
    pub(crate) fn from_outline_scoped_available(
        outline: &Outline,
        context: Key,
        facts: HygieneFacts,
        dependencies: &BTreeMap<MacroName, Digest>,
        available: &BTreeMap<MacroName, Digest>,
        disabled: &[ScopeKey],
    ) -> Result<Self, Diagnostic> {
        let index = SourceMacroIndex::from_outline(outline, context, facts)?;
        let mut routes = Self::from_lookup_with_available(
            outline,
            dependencies,
            available,
            disabled,
            |id| index.definition(id).cloned(),
            |module, at, name| index.lookup(module, at, name),
        )?;
        routes.export_pending = index.pending_producers().collect();
        Ok(routes)
    }
    /// Candidate routes from the actual interleaved textual publication state.
    pub(crate) fn from_completed_index(
        outline: &Outline,
        index: &SourceMacroIndex,
        dependencies: &BTreeMap<MacroName, Digest>,
        disabled: &[ScopeKey],
    ) -> Result<Self, Diagnostic> {
        let mut result = Self::from_lookup(
            outline,
            dependencies,
            disabled,
            |id| index.definition(id).cloned(),
            |module, at, name| index.lookup(module, at, name),
        )?;
        result.export_pending = index.pending_producers().collect();
        Ok(result)
    }
    pub(crate) fn from_lookup(
        outline: &Outline,
        dependencies: &BTreeMap<MacroName, Digest>,
        disabled: &[ScopeKey],
        definition: impl Fn(Digest) -> Option<MacroDefinition>,
        lookup: impl Fn(&[String], u64, &str) -> Result<HygienicLookup, Diagnostic>,
    ) -> Result<Self, Diagnostic> {
        Self::from_lookup_with_available(
            outline,
            dependencies,
            dependencies,
            disabled,
            definition,
            lookup,
        )
    }
    pub(crate) fn from_lookup_with_available(
        outline: &Outline,
        dependencies: &BTreeMap<MacroName, Digest>,
        available: &BTreeMap<MacroName, Digest>,
        disabled: &[ScopeKey],
        definition: impl Fn(Digest) -> Option<MacroDefinition>,
        lookup: impl Fn(&[String], u64, &str) -> Result<HygienicLookup, Diagnostic>,
    ) -> Result<Self, Diagnostic> {
        if disabled.len() > 100_000
            || disabled
                .iter()
                .try_fold(0usize, |total, scope| {
                    total
                        .checked_add(scope.heap_bytes())
                        .and_then(|n| n.checked_add(64))
                        .filter(|n| *n <= 32 * 1024 * 1024)
                })
                .is_none()
        {
            return Err(error("disabled import scope metadata budget"));
        }
        if dependencies.len() > 100_000 || available.len() > 100_000 {
            return Err(error("dependency route limit exceeded"));
        }
        if outline.imports.len() > 100_000 {
            return Err(error("source route count exceeded"));
        }
        let mut bytes = 0usize;
        let mut charge = |amount: usize| -> Result<(), Diagnostic> {
            bytes = bytes
                .checked_add(amount)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| error("source route metadata budget exceeded"))?;
            Ok(())
        };
        for alias in dependencies.keys().chain(available.keys()) {
            charge(alias.as_str().len())?;
        }
        for import in &outline.imports {
            // Account for the retained source row and two owned route paths
            // before any cloned names or scopes are materialized.
            charge(std::mem::size_of::<ImportRoute>() + 128)?;
            charge(
                import
                    .scope
                    .heap_bytes()
                    .checked_mul(3)
                    .ok_or_else(|| error("source route size overflow"))?,
            )?;
            charge(
                import
                    .path
                    .len()
                    .checked_mul(3 * std::mem::size_of::<ImportSegment>())
                    .ok_or_else(|| error("source route size overflow"))?,
            )?;
            for segment in &import.path {
                if let ImportSegment::Name(name) = segment {
                    charge(
                        name.len()
                            .checked_mul(3)
                            .ok_or_else(|| error("source route size overflow"))?,
                    )?;
                }
            }
            if let ImportKind::Named { alias: Some(alias) } = &import.kind {
                charge(
                    alias
                        .len()
                        .checked_mul(2)
                        .ok_or_else(|| error("source route size overflow"))?,
                )?;
            }
            charge(crate::syntax::imports::visibility_bytes(&import.visibility))?;
        }
        for event in &outline.textual_events {
            if let TextualEventKind::Definition { source_name, .. }
            | TextualEventKind::ModuleDefinition { source_name, .. } = &event.kind
            {
                charge(source_name.len())?;
                charge(std::mem::size_of::<DefinitionRoute>())?;
            }
        }
        if outline.scopes.len() > 100_000 {
            return Err(error("source module count exceeded"));
        }
        for scope in outline.scopes.keys() {
            charge(scope.heap_bytes())?;
        }
        let mut result = Self {
            definitions: Vec::new(),
            imports: Vec::new(),
            deferred: Vec::new(),
            module_access: BTreeMap::new(),
            edition: outline.profile.edition,
            extern_aliases: vec![],
            export_pending: BTreeSet::new(),
            body_owner_pending: BTreeMap::new(),
            modules: BTreeSet::new(),
            dependencies: dependencies.clone(),
            source_imports: outline.imports.clone(),
            textual_import_definitions: BTreeMap::new(),
            implicit_disabled: disabled.iter().cloned().collect(),
            binding_origin: None,
        };
        let items: BTreeMap<_, _> = outline
            .items
            .iter()
            .map(|i| (i.def_key.fingerprint(), i))
            .collect();
        result
            .export_pending
            .extend(
                outline
                    .textual_events
                    .iter()
                    .filter_map(|event| match event.kind {
                        TextualEventKind::Producer { identity, .. } => Some(identity),
                        _ => None,
                    }),
            );
        let mut explicit_names = BTreeSet::new();
        for row in &outline.extern_crates {
            charge(
                row.scope
                    .heap_bytes()
                    .checked_add(row.alias.as_ref().map_or(0, String::len))
                    .and_then(|n| n.checked_add(row.dependency.len()))
                    .and_then(|n| {
                        n.checked_add(crate::syntax::imports::visibility_bytes(&row.visibility))
                    })
                    .and_then(|n| n.checked_add(256))
                    .ok_or_else(|| error("extern alias metadata overflow"))?,
            )?;
            let item = items
                .get(&row.identity)
                .ok_or_else(|| error("extern crate header absent"))?;
            if item.header.attributes.iter().any(|attribute| {
                matches!(
                    attribute.class,
                    AttributeClass::TransformUnsupported | AttributeClass::Derive
                )
            }) {
                continue;
            }
            if row.macro_use.is_some()
                && (!row.scope.module_path.is_empty() || row.dependency == "self")
            {
                return Err(error(
                    "extern macro-use requires a dependency declaration at the crate root",
                ));
            }
            if let Some(recipe) = &row.macro_use {
                charge(recipe.bytes()?)?;
            }
            charge(
                row.order
                    .len()
                    .checked_mul(4)
                    .ok_or_else(|| error("extern macro-use order overflow"))?,
            )?;
            let provider = if row.dependency == "self" {
                row.scope.crate_key
            } else {
                *available
                    .get(&MacroName::parse(&row.dependency, outline.profile.edition)?)
                    .ok_or_else(|| error("extern crate dependency is not captured"))?
            };
            let alias = row
                .alias
                .as_ref()
                .map(|name| semantic(name, outline.profile.edition))
                .transpose()?;
            if let Some(alias) = &alias
                && !explicit_names.insert((row.scope.clone(), alias.clone()))
            {
                return Err(error("duplicate explicit extern crate alias"));
            }
            result
                .extern_aliases
                .try_reserve(1)
                .map_err(|_| error("extern alias allocation failed"))?;
            result.extern_aliases.push(ExternAlias {
                declaration: row.identity,
                scope: row.scope.clone(),
                name: alias,
                provider,
                visibility: row.visibility.clone(),
                macro_use: row.macro_use.clone(),
                order: row.order.clone(),
            });
        }
        let edition = outline.profile.edition;
        for scope in outline.scopes.keys().filter(|scope| scope.owner.is_none()) {
            result
                .modules
                .insert((scope.crate_key, module_names(&scope.module_path, edition)?));
        }
        for event in &outline.textual_events {
            let (TextualEventKind::Definition {
                identity,
                source_name,
            }
            | TextualEventKind::ModuleDefinition {
                identity,
                source_name,
            }) = &event.kind
            else {
                continue;
            };
            let item = items
                .get(identity)
                .ok_or_else(|| error("export header is missing"))?;
            if matches!(event.kind, TextualEventKind::ModuleDefinition { .. }) {
                let mut path = module_names(&item.scope.module_path, edition)?;
                path.push(MacroName::parse(source_name, edition)?);
                result
                    .definitions
                    .try_reserve(1)
                    .map_err(|_| error("module macro route allocation"))?;
                result.definitions.push(DefinitionRoute {
                    path: MacroPath::new(item.scope.crate_key, path)?,
                    definition: definition(*identity)
                        .ok_or_else(|| error("module macro definition missing"))?,
                    visibility: item.header.visibility.clone(),
                });
                continue;
            }
            if item
                .header
                .attributes
                .iter()
                .any(|a| a.class == AttributeClass::MacroExport)
            {
                result
                    .definitions
                    .try_reserve(1)
                    .map_err(|_| error("macro export allocation failed"))?;
                result.definitions.push(DefinitionRoute {
                    path: MacroPath::new(
                        outline.root_scope.crate_key,
                        vec![MacroName::parse(source_name, edition)?],
                    )?,
                    definition: definition(*identity)
                        .ok_or_else(|| error("export definition is missing"))?,
                    visibility: Visibility::Public,
                });
            }
        }
        if outline.root_scope.module_path.is_empty() {
            result.module_access.insert(
                (outline.root_scope.crate_key, vec![]),
                crate::syntax::visibility::VisibilityScope::Public,
            );
        }
        for item in &outline.items {
            if item.def_key.item_kind == crate::source::manifest::ItemKind::Module
                && !outline
                    .extern_crates
                    .iter()
                    .any(|row| row.identity == item.def_key.fingerprint())
            {
                let mut path = module_names(&item.scope.module_path, edition)?;
                path.push(semantic(&item.def_key.declared_name, edition)?);
                let visibility = crate::syntax::visibility::VisibilityScope::for_declaration(
                    &item.header.visibility,
                    &item.scope,
                    edition,
                )?;
                if result
                    .module_access
                    .insert((item.scope.crate_key, path), visibility)
                    .is_some()
                {
                    return Err(error("duplicate module visibility header"));
                }
            }
        }
        for import in &outline.imports {
            let item = items
                .get(&import.identity)
                .ok_or_else(|| error("import header is missing"))?;
            let deferred = if item.header.attributes.iter().any(|a| {
                matches!(
                    a.class,
                    AttributeClass::TransformUnsupported | AttributeClass::Derive
                )
            }) {
                Some(DeferredImportReason::AttributeProducer)
            } else if import.kind == ImportKind::Glob {
                Some(DeferredImportReason::Glob)
            } else if matches!(
                import.path.last(),
                Some(ImportSegment::SelfModule | ImportSegment::Crate | ImportSegment::Super)
            ) {
                Some(DeferredImportReason::ModuleAlias)
            } else {
                None
            };
            if let Some(reason) = deferred {
                result.defer(import, reason)?;
                continue;
            }
            if import.kind == ImportKind::Anonymous {
                continue;
            }
            let Some(ImportSegment::Name(last)) = import.path.last() else {
                return Err(error("named macro import has no identifier"));
            };
            let name = MacroName::parse(last, edition)?;
            let alias = match &import.kind {
                ImportKind::Named { alias: Some(a) } => MacroName::parse(a, edition)?,
                _ => name.clone(),
            };
            let mut destination = module_names(&import.scope.module_path, edition)?;
            destination.push(alias);
            let destination = MacroPath::new(import.scope.crate_key, destination)?;
            // A single unqualified import can refer to the visible textual
            // macro, whose declaration identity must survive its rename.
            if !import.absolute && import.path.len() == 1 {
                match lookup(
                    &import.scope.module_path,
                    u64::from(import.range.start),
                    last,
                )? {
                    HygienicLookup::Selection(MacroSelection::Defined(definition)) => {
                        if let Some(attribute) =
                            crate::expand::builtin_attributes::inert_descriptor(name.as_str())?
                        {
                            return Err(Diagnostic::new(
                                "LRR-E-MACRO-AMBIGUOUS",
                                format!(
                                    "unqualified import {} selects user macro {} and compiler inert attribute {} ({})",
                                    name.as_str(),
                                    definition.identity.hex(),
                                    attribute.name(),
                                    attribute.definition_identity().hex()
                                ),
                            ));
                        }
                        result.textual_import_definitions.insert(
                            super::names::NameNamespaces::import_identity(import),
                            definition.clone(),
                        );
                        result
                            .definitions
                            .try_reserve(1)
                            .map_err(|_| error("macro import allocation failed"))?;
                        result.definitions.push(DefinitionRoute {
                            path: destination,
                            definition,
                            visibility: import.visibility.clone(),
                        });
                        continue;
                    }
                    HygienicLookup::NeedInput(input) => {
                        result.defer(import, DeferredImportReason::TextualProducer(input))?;
                        continue;
                    }
                    HygienicLookup::Selection(MacroSelection::Ambiguous(_)) => {
                        result.defer(import, DeferredImportReason::TextualAmbiguity)?;
                        continue;
                    }
                    HygienicLookup::Selection(MacroSelection::Absent) => {}
                }
            }
            if !import.absolute
                && let Some(ImportSegment::Name(first)) = import.path.first()
            {
                let first = MacroName::parse(first, edition)?;
                let mut module = import.scope.module_path.clone();
                module.push(first.as_str().into());
                if dependencies.contains_key(&first)
                    && outline.scopes.keys().any(|scope| {
                        scope.crate_key == import.scope.crate_key
                            && scope.owner.is_none()
                            && scope.module_path == module
                    })
                {
                    result.defer(import, DeferredImportReason::DependencyModuleCollision)?;
                    continue;
                }
            }
            if import.path.len() == 1
                && let Some(ImportSegment::Name(first)) = import.path.first()
                && dependencies.contains_key(&MacroName::parse(first, edition)?)
            {
                result.defer(import, DeferredImportReason::ModuleAlias)?;
                continue;
            }
            if !import.absolute
                && import.path.len() > 1
                && let Some(ImportSegment::Name(first)) = import.path.first()
            {
                let first = MacroName::parse(first, edition)?;
                let mut module = import.scope.module_path.clone();
                module.push(first.as_str().into());
                if !dependencies.contains_key(&first)
                    && !outline.scopes.keys().any(|scope| {
                        scope.crate_key == import.scope.crate_key
                            && scope.owner.is_none()
                            && scope.module_path == module
                    })
                {
                    result.defer(import, DeferredImportReason::ModuleLookup)?;
                    continue;
                }
            }
            let empty = BTreeMap::new();
            let target = target_path(
                import,
                outline,
                if result.extern_prelude_enabled(&import.scope) {
                    dependencies
                } else {
                    &empty
                },
            )?;
            let target_module: Vec<_> = target
                .components()
                .iter()
                .map(|name| name.as_str().to_owned())
                .collect();
            if outline.scopes.keys().any(|scope| {
                scope.crate_key == target.unit
                    && scope.owner.is_none()
                    && scope.module_path == target_module
            }) {
                result.defer(import, DeferredImportReason::ModuleAlias)?;
                continue;
            }
            result
                .imports
                .try_reserve(1)
                .map_err(|_| error("macro import allocation failed"))?;
            result.imports.push(ImportRoute {
                path: destination,
                target,
                source: import.clone(),
            });
        }
        Ok(result)
    }
    fn defer(
        &mut self,
        source: &SourceImport,
        reason: DeferredImportReason,
    ) -> Result<(), Diagnostic> {
        self.deferred
            .try_reserve(1)
            .map_err(|_| error("deferred import allocation failed"))?;
        self.deferred.push(DeferredImport {
            source: source.clone(),
            reason,
        });
        Ok(())
    }
    pub(crate) fn extern_prelude_enabled(&self, scope: &ScopeKey) -> bool {
        self.edition >= 2018 && self.implicit_enabled(scope)
    }
    pub(crate) fn implicit_enabled(&self, scope: &ScopeKey) -> bool {
        !self.implicit_disabled.iter().any(|disabled| {
            disabled.crate_key == scope.crate_key
                && scope.module_path.starts_with(&disabled.module_path)
        })
    }
    pub(crate) fn canonical_generated_extern_aliases(
        &mut self,
        declarations: &BTreeMap<Digest, Digest>,
    ) -> Result<(), Diagnostic> {
        for row in &mut self.extern_aliases {
            row.declaration = *declarations
                .get(&row.declaration)
                .ok_or_else(|| error("generated extern alias canonical header absent"))?;
        }
        Ok(())
    }
    pub(crate) fn canonical_generated_imports(
        &mut self,
        imports: &[SourceImport],
    ) -> Result<(), Diagnostic> {
        let replacements = imports
            .iter()
            .map(|i| ((i.range.start, i.range.end, i.leaf_ordinal), i))
            .collect::<BTreeMap<_, _>>();
        let mut textual = BTreeMap::new();
        for import in &self.source_imports {
            let next = replacements
                .get(&(import.range.start, import.range.end, import.leaf_ordinal))
                .ok_or_else(|| error("generated import canonical header absent"))?;
            if let Some(definition) = self
                .textual_import_definitions
                .get(&super::names::NameNamespaces::import_identity(import))
            {
                textual.insert(
                    super::names::NameNamespaces::import_identity(next),
                    definition.clone(),
                );
            }
        }
        for import in self
            .source_imports
            .iter_mut()
            .chain(self.imports.iter_mut().map(|r| &mut r.source))
            .chain(self.deferred.iter_mut().map(|r| &mut r.source))
        {
            let next = replacements
                .get(&(import.range.start, import.range.end, import.leaf_ordinal))
                .ok_or_else(|| error("generated import canonical source absent"))?;
            if import.scope != next.scope
                || import.path != next.path
                || import.kind != next.kind
                || import.absolute != next.absolute
                || import.visibility != next.visibility
            {
                return Err(error("generated import canonical syntax differs"));
            }
            *import = (*next).clone();
        }
        self.textual_import_definitions = textual;
        Ok(())
    }
    pub fn definitions(&self) -> &[DefinitionRoute] {
        &self.definitions
    }
    pub fn imports(&self) -> &[ImportRoute] {
        &self.imports
    }
    pub fn deferred(&self) -> &[DeferredImport] {
        &self.deferred
    }
    /// Checked retained route metadata, including duplicated source import rows.
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        fn path(names: &[MacroName]) -> Option<usize> {
            names.iter().try_fold(64usize, |n, name| {
                n.checked_add(64)?
                    .checked_add(name.as_str().len().checked_mul(4)?)
            })
        }
        fn source(import: &SourceImport) -> Option<usize> {
            let mut n = 256usize
                .checked_add(import.scope.heap_bytes())?
                .checked_add(crate::syntax::imports::visibility_bytes(&import.visibility))?;
            for part in &import.path {
                n = n.checked_add(64)?;
                if let ImportSegment::Name(name) = part {
                    n = n.checked_add(name.capacity())?;
                }
            }
            if let ImportKind::Named { alias: Some(name) } = &import.kind {
                n = n.checked_add(name.capacity())?;
            }
            Some(n)
        }
        let mut n = std::mem::size_of::<Self>();
        let mut charge = |bytes: Option<usize>| -> Result<(), Diagnostic> {
            n = n
                .checked_add(bytes.ok_or_else(|| error("macro route metadata overflow"))?)
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or_else(|| error("macro route metadata budget exceeded"))?;
            Ok(())
        };
        for route in &self.definitions {
            charge(path(route.path.components()).and_then(|n| {
                n.checked_add(256)?
                    .checked_add(crate::syntax::imports::visibility_bytes(&route.visibility))
            }))?;
        }
        for route in &self.imports {
            charge(path(route.path.components()))?;
            charge(path(route.target.components()))?;
            charge(source(&route.source))?;
        }
        for row in &self.deferred {
            charge(source(&row.source).and_then(|n| n.checked_add(64)))?;
        }
        for row in &self.source_imports {
            charge(source(row))?;
        }
        for row in &self.extern_aliases {
            charge(row.order.len().checked_mul(4))?;
            if let Some(recipe) = &row.macro_use {
                charge(Some(recipe.bytes()?))?;
            }
            let (scope, alias, visibility) = (&row.scope, &row.name, &row.visibility);
            charge(
                Some(scope.heap_bytes())
                    .and_then(|n| {
                        n.checked_add(alias.as_ref().map_or(0, |name| name.as_str().len()))
                    })
                    .and_then(|n| {
                        n.checked_add(crate::syntax::imports::visibility_bytes(visibility))
                    })
                    .and_then(|n| n.checked_add(256)),
            )?;
        }
        for (_, names) in &self.modules {
            charge(path(names))?;
        }
        for ((_, names), access) in &self.module_access {
            charge(path(names))?;
            if let crate::syntax::visibility::VisibilityScope::Within { module_path, .. } = access {
                charge(
                    module_path
                        .iter()
                        .try_fold(64usize, |n, s| n.checked_add(64)?.checked_add(s.capacity())),
                )?;
            }
        }
        for name in self.dependencies.keys() {
            charge(
                Some(128).and_then(|n: usize| n.checked_add(name.as_str().len().checked_mul(4)?)),
            )?;
        }
        charge(self.export_pending.len().checked_mul(32))?;
        charge(self.body_owner_pending.len().checked_mul(96))?;
        charge(self.textual_import_definitions.len().checked_mul(128))?;
        for scope in &self.implicit_disabled {
            charge(Some(scope.heap_bytes()).and_then(|n| n.checked_add(64)))?;
        }
        Ok(n)
    }
    /// Resolve this exact import facet, rather than any definition sharing its alias.
    pub(crate) fn import_definition(
        &self,
        import: &SourceImport,
        names: &MacroNamespaces,
    ) -> Result<Option<MacroDefinition>, Diagnostic> {
        use super::macro_names::PathLookup;
        let id = super::names::NameNamespaces::import_identity(import);
        if let Some(definition) = self.textual_import_definitions.get(&id) {
            let ImportKind::Named { alias } = &import.kind else {
                return Ok(None);
            };
            let Some(ImportSegment::Name(last)) = import.path.last() else {
                return Ok(None);
            };
            let mut path = module_names(&import.scope.module_path, self.edition)?;
            path.push(MacroName::parse(
                alias.as_ref().unwrap_or(last),
                self.edition,
            )?);
            let destination = MacroPath::new(import.scope.crate_key, path)?;
            return Ok(match names.lookup_from(&destination, &import.scope)? {
                PathLookup::Defined(selected) if selected == *definition => Some(selected),
                _ => None,
            });
        }
        let Some(route) = self.imports.iter().find(|route| route.source == *import) else {
            return Ok(None);
        };
        let mut reference_scope = import.scope.clone();
        if self.edition == 2015
            && !import.absolute
            && matches!(import.path.first(), Some(ImportSegment::Name(_)))
        {
            reference_scope.module_path.clear();
        }
        let Some(target_path) = names.import_segments_path(
            &import.path,
            import.absolute,
            &reference_scope,
            self.edition,
        )?
        else {
            return Ok(None);
        };
        let PathLookup::Defined(target) = names.lookup_from(&target_path, &import.scope)? else {
            return Ok(None);
        };
        Ok(match names.lookup_from(&route.path, &import.scope)? {
            PathLookup::Defined(destination) if destination == target => Some(target),
            _ => None,
        })
    }
    pub(crate) fn discovery_graph_with_compiler_imports(
        catalogs: &[std::sync::Arc<Self>],
        imports: &[super::compile_input_catalog::CompilerInjectedImport],
    ) -> Result<MacroNamespaces, Diagnostic> {
        if catalogs.len() > 100_000 {
            return Err(error("source catalog count exceeded"));
        }
        let mut borrowed = Vec::new();
        borrowed
            .try_reserve_exact(catalogs.len())
            .map_err(|_| error("source catalogue borrow allocation failed"))?;
        borrowed.extend(catalogs.iter().map(|catalog| catalog.as_ref()));
        super::module_aliases::graph_with_compiler_imports(&borrowed, true, imports)
    }
    /// Partial candidate graph retains exact textual producer demands as pending buckets.
    pub(crate) fn discovery_graph(
        catalogs: &[std::sync::Arc<Self>],
    ) -> Result<MacroNamespaces, Diagnostic> {
        if catalogs.len() > 100_000 {
            return Err(error("source catalog count exceeded"));
        }
        let mut borrowed = Vec::new();
        borrowed
            .try_reserve_exact(catalogs.len())
            .map_err(|_| error("source catalogue borrow allocation failed"))?;
        borrowed.extend(catalogs.iter().map(|c| c.as_ref()));
        super::module_aliases::graph_with_producers(&borrowed, true)
    }
    /// Construct a fresh graph atomically. Its lookups are candidate facts;
    /// callers must supply producer/seal and access checks before execution.
    pub fn candidate_graph(catalogs: &[Self]) -> Result<MacroNamespaces, Diagnostic> {
        super::module_aliases::candidate_graph(catalogs)
    }
}
fn target_path(
    import: &SourceImport,
    outline: &Outline,
    dependencies: &BTreeMap<MacroName, Digest>,
) -> Result<MacroPath, Diagnostic> {
    let edition = outline.profile.edition;
    let mut unit = import.scope.crate_key;
    let mut names = module_names(&import.scope.module_path, edition)?;
    let mut segments = import.path.iter().peekable();
    if import.absolute && edition == 2015 {
        names.clear();
    } else if import.absolute {
        let Some(ImportSegment::Name(alias)) = segments.next() else {
            return Err(error("absolute import needs an external crate name"));
        };
        let alias = MacroName::parse(alias, edition)?;
        unit = *dependencies
            .get(&alias)
            .ok_or_else(|| error("unknown absolute dependency alias"))?;
        names.clear();
    } else {
        if edition == 2015 && matches!(segments.peek(), Some(ImportSegment::Name(_))) {
            names.clear();
        }
        match segments.peek() {
            Some(ImportSegment::Crate) => {
                names.clear();
                segments.next();
            }
            Some(ImportSegment::SelfModule) => {
                segments.next();
            }
            Some(ImportSegment::Super) => {}
            Some(ImportSegment::Name(first)) => {
                let name = MacroName::parse(first, edition)?;
                let mut local = import.scope.module_path.clone();
                local.push(name.as_str().into());
                let local_module = outline
                    .scopes
                    .keys()
                    .any(|s| s.crate_key == unit && s.owner.is_none() && s.module_path == local);
                if !local_module && let Some(provider) = dependencies.get(&name) {
                    unit = *provider;
                    names.clear();
                    segments.next();
                }
            }
            None => return Err(error("empty source import")),
        }
    }
    let mut named = false;
    for segment in segments {
        match segment {
            ImportSegment::Super if !named => {
                names
                    .pop()
                    .ok_or_else(|| error("super import escapes crate root"))?;
            }
            ImportSegment::Name(name) => {
                named = true;
                if names.len() >= 256 {
                    return Err(error("macro route depth exceeded"));
                }
                names
                    .try_reserve(1)
                    .map_err(|_| error("macro route allocation failed"))?;
                names.push(MacroName::parse(name, edition)?);
            }
            _ => return Err(error("invalid source import qualifier position")),
        }
    }
    MacroPath::new(unit, names)
}
