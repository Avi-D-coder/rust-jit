//! Standard library macro definitions resolved on demand (E-R5-006 decision).
//!
//! The unchanged `core`/`alloc`/`std` source files that define the library's
//! macros are parsed once per owning profile. Their ordinary declaration routes
//! (exported `macro_rules!`, `macro` 2.0 items and the crate-root re-exports of
//! those macros) join the guest's macro namespace, so `assert_eq!`, `panic!`,
//! `println!` and `vec!` select the real library definitions and expand
//! through the ordinary declarative pipeline.
//!
//! Library modules that are not loaded are simply absent: a program rustc
//! accepts cannot have a macro-generated item that changes an answer found
//! among the original library definitions, so no library-wide seal is needed.
use super::{
    macro_names::MacroPath,
    source_macro_routes::{DefinitionRoute, SourceMacroRoutes},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::{
        scopes::{MacroDefinition, MacroName},
        source_declarative::SourceDeclarative,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{imports::ImportSegment, parse::ParseKey},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock},
};

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-LIBRARY-MACROS", message.into())
}

/// Edition declared by the library crates' Cargo manifests.
const LIBRARY_EDITION: u16 = 2024;

struct LibraryFile {
    krate: &'static str,
    module_path: &'static [&'static str],
    path: &'static str,
    bytes: &'static [u8],
}

macro_rules! library_file {
    ($krate:literal, [$($module:literal),*], $path:literal) => {
        LibraryFile {
            krate: $krate,
            module_path: &[$($module),*],
            path: $path,
            bytes: include_bytes!(concat!(
                "../../../../fixtures/upstream/rustc-checkout/library/",
                $path
            )),
        }
    };
}

/// The unchanged upstream files that define or re-export library macros.
const FILES: &[LibraryFile] = &[
    library_file!("core", [], "core/src/lib.rs"),
    library_file!("core", ["macros"], "core/src/macros/mod.rs"),
    library_file!("core", ["panic"], "core/src/panic.rs"),
    // `#[rustc_builtin_macro]` derive definitions (`pub macro Clone($item:item)`)
    // and their module re-exports (`core::fmt::Debug`, `core::hash::Hash`).
    library_file!("core", ["clone"], "core/src/clone.rs"),
    library_file!("core", ["marker"], "core/src/marker.rs"),
    library_file!("core", ["cmp"], "core/src/cmp.rs"),
    library_file!("core", ["default"], "core/src/default.rs"),
    library_file!("core", ["hash"], "core/src/hash/mod.rs"),
    library_file!("core", ["fmt"], "core/src/fmt/mod.rs"),
    library_file!("alloc", [], "alloc/src/lib.rs"),
    library_file!("alloc", ["macros"], "alloc/src/macros.rs"),
    library_file!("std", [], "std/src/lib.rs"),
    library_file!("std", ["macros"], "std/src/macros.rs"),
    library_file!("std", ["panic"], "std/src/panic.rs"),
];

pub(crate) struct LibraryMacros {
    _prepared: PreparedSourceInputs,
    providers: Vec<SourceDeclarative>,
    routes: Vec<Arc<SourceMacroRoutes>>,
    units: BTreeMap<&'static str, Digest>,
    /// Definitions marked `#[rustc_builtin_macro]`, by the builtin's name.
    builtins: BTreeMap<Digest, String>,
    /// The library's own macro graph, built on first path resolution.
    graph: std::sync::OnceLock<Result<super::macro_names::MacroNamespaces, Diagnostic>>,
}

impl std::fmt::Debug for LibraryMacros {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryMacros")
            .field("units", &self.units)
            .finish_non_exhaustive()
    }
}

impl LibraryMacros {
    /// Shared per owning profile; parsing the library files happens once.
    pub(crate) fn for_profile(profile: &ProfileIdentity) -> Result<Arc<Self>, Diagnostic> {
        static CACHE: OnceLock<Mutex<BTreeMap<ProfileIdentity, Arc<LibraryMacros>>>> =
            OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
        if let Some(found) = cache
            .lock()
            .map_err(|_| fault("library macro cache poisoned"))?
            .get(profile)
        {
            return Ok(Arc::clone(found));
        }
        let built = Arc::new(Self::build(profile)?);
        Ok(Arc::clone(
            cache
                .lock()
                .map_err(|_| fault("library macro cache poisoned"))?
                .entry(profile.clone())
                .or_insert(built),
        ))
    }

    fn build(owner: &ProfileIdentity) -> Result<Self, Diagnostic> {
        let units: BTreeMap<&'static str, Digest> = ["core", "alloc", "std"]
            .into_iter()
            .map(|name| (name, crate::facade::unit(owner, name)))
            .collect();
        let profile = ProfileIdentity {
            edition: LIBRARY_EDITION,
            ..ProfileIdentity::current()
        };
        let cfg = CfgSet::guest_default();
        let mut inputs = Vec::new();
        let mut selections = Vec::new();
        for file in FILES {
            let blob = SourceBlob::new(file.bytes.to_vec())?;
            let path = SourcePath::new(file.path)?;
            let unit = units[file.krate];
            let module_path: Vec<String> = file.module_path.iter().map(|m| (*m).into()).collect();
            selections.push(SourceSelection {
                path: path.clone(),
                outline: OutlineKey {
                    parse: ParseKey {
                        content: blob.content(),
                        profile: profile.clone(),
                    },
                    crate_key: unit,
                    module_path: module_path.clone(),
                    cfg: cfg.fingerprint(),
                },
            });
            inputs.push(SourceUnitInput {
                path,
                crate_key: unit,
                module_path,
                profile: profile.clone(),
                cfg: cfg.clone(),
                blob,
            });
        }
        let dependency = |consumer: &str, alias: &str, provider: &str| SourceDependency {
            consumer: units[consumer],
            alias: alias.into(),
            provider: units[provider],
        };
        let prepared = PreparedSourceInputs::refresh_with_dependencies(
            None,
            inputs,
            BTreeMap::new(),
            vec![
                dependency("alloc", "core", "core"),
                dependency("std", "core", "core"),
                dependency("std", "alloc", "alloc"),
            ],
        )?;
        let mut providers = Vec::new();
        for selection in &selections {
            let SourceSelectionOutcome::Selected(view) = prepared.select(selection)? else {
                return Err(fault("library macro source was not selected"));
            };
            providers.push(SourceDeclarative::new(view)?);
        }
        let mut routes = Vec::new();
        let mut builtins = BTreeMap::new();
        let mut names = BTreeSet::new();
        for (file, provider) in FILES.iter().zip(&providers) {
            let outline = provider.module_source().outline();
            let mut dependencies = BTreeMap::new();
            for (alias, target) in match file.krate {
                "alloc" => &[("core", "core")][..],
                "std" => &[("core", "core"), ("alloc", "alloc")][..],
                _ => &[][..],
            } {
                dependencies.insert(MacroName::parse(alias, LIBRARY_EDITION)?, units[target]);
            }
            // Other dependencies (`libc`, `unwind`, ...) are not loaded: their
            // units exist and contain nothing this namespace needs.
            for row in &outline.extern_crates {
                if row.dependency != "self" {
                    let name = MacroName::parse(&row.dependency, LIBRARY_EDITION)?;
                    dependencies
                        .entry(name)
                        .or_insert_with(|| crate::facade::unit(owner, &row.dependency));
                }
            }
            let route =
                SourceMacroRoutes::from_completed_index(outline, provider.macro_index(), &dependencies, &[])?;
            for definition in &route.definitions {
                if let Some(name) = definition.path.components().last() {
                    names.insert(name.as_str().to_owned());
                }
            }
            for item in &outline.items {
                let Some(attribute) = item
                    .header
                    .attributes
                    .iter()
                    .find(|attribute| attribute.name == "rustc_builtin_macro")
                else {
                    continue;
                };
                // `#[rustc_builtin_macro(core_panic)]` names its expander;
                // otherwise the builtin shares the macro's own name.
                let text: String = attribute.tokens.iter().map(|t| t.text.as_str()).collect();
                let name = text
                    .split_once('(')
                    .and_then(|(_, rest)| rest.split(')').next())
                    .map(|inner| inner.split(',').next().unwrap_or(inner).trim().to_owned())
                    .filter(|inner| !inner.is_empty())
                    .unwrap_or_else(|| item.def_key.declared_name.clone());
                builtins.insert(item.def_key.fingerprint(), name);
            }
            routes.push(route);
        }
        // Keep the ordinary routes, restricted to what the macro namespace needs
        // from these partially loaded crates: definitions, the re-exports of
        // library macros, and the module/extern structure they route through.
        // Unloaded modules are absent, never pending.
        for route in &mut routes {
            route.export_pending.clear();
            route.body_owner_pending.clear();
            route.imports.retain(|import| reexports_macro(&import.source, &names));
            route.deferred.retain(|deferred| reexports_macro(&deferred.source, &names));
            // The guest's own name graph never owns library imports; their
            // macro re-exports travel through `imports` in the macro graph.
            route.source_imports.clear();
        }
        Ok(Self {
            _prepared: prepared,
            providers,
            routes: routes.into_iter().map(Arc::new).collect(),
            units,
            builtins,
            graph: std::sync::OnceLock::new(),
        })
    }

    pub(crate) fn providers(&self) -> &[SourceDeclarative] {
        &self.providers
    }
    pub(crate) fn routes(&self) -> &[Arc<SourceMacroRoutes>] {
        &self.routes
    }
    pub(crate) fn unit(&self, name: &str) -> Option<Digest> {
        self.units.get(name).copied()
    }
    /// The builtin expander name for a `#[rustc_builtin_macro]` definition.
    pub(crate) fn builtin(&self, definition: &MacroDefinition) -> Option<&str> {
        self.builtins.get(&definition.identity).map(String::as_str)
    }
    /// The library's own definition exported at `krate::name`, if any.
    pub(crate) fn exported(&self, krate: &str, name: &str) -> Option<&DefinitionRoute> {
        let unit = self.unit(krate)?;
        self.routes.iter().flat_map(|route| route.definitions.iter()).find(|route| {
            route.path.unit == unit
                && route.path.components().len() == 1
                && route.path.components()[0].as_str() == name
        })
    }
    /// The definition `krate::name` names through the library's own routes,
    /// following its crate-root reexports (`pub use crate::macros::builtin::derive`).
    pub(crate) fn resolve(&self, krate: &str, name: &str) -> Option<MacroDefinition> {
        if let Some(route) = self.exported(krate, name) {
            return Some(route.definition.clone());
        }
        let unit = self.unit(krate)?;
        let graph = self
            .graph
            .get_or_init(|| SourceMacroRoutes::discovery_graph(&self.routes))
            .as_ref()
            .ok()?;
        let path = MacroPath::new(unit, vec![MacroName::parse(name, LIBRARY_EDITION).ok()?]).ok()?;
        let caller = crate::syntax::outline::ScopeKey {
            crate_key: unit,
            module_path: vec![],
            owner: None,
        };
        match graph.lookup_from(&path, &caller).ok()? {
            super::macro_names::PathLookup::Defined(definition) => Some(definition),
            _ => None,
        }
    }
    /// The definition `krate::a::b::name` names through the library's own
    /// routes (module re-exports such as `core::fmt::Debug` included).
    pub(crate) fn resolve_path(&self, krate: &str, segments: &[&str]) -> Option<MacroDefinition> {
        let unit = self.unit(krate)?;
        let graph = self
            .graph
            .get_or_init(|| SourceMacroRoutes::discovery_graph(&self.routes))
            .as_ref()
            .ok()?;
        let names = segments
            .iter()
            .map(|segment| MacroName::parse(segment, LIBRARY_EDITION).ok())
            .collect::<Option<Vec<_>>>()?;
        let path = MacroPath::new(unit, names).ok()?;
        let caller = crate::syntax::outline::ScopeKey {
            crate_key: unit,
            module_path: vec![],
            owner: None,
        };
        match graph.lookup_from(&path, &caller).ok()? {
            super::macro_names::PathLookup::Defined(definition) => Some(definition),
            _ => None,
        }
    }
    pub(crate) fn defines(&self, path: &MacroPath) -> bool {
        self.routes
            .iter()
            .flat_map(|route| route.definitions.iter())
            .any(|route| &route.path == path)
    }
}

fn reexports_macro(import: &crate::syntax::imports::SourceImport, names: &BTreeSet<String>) -> bool {
    matches!(import.path.last(), Some(ImportSegment::Name(name)) if names.contains(name))
}

#[cfg(test)]
#[path = "library_macros_tests.rs"]
mod tests;
