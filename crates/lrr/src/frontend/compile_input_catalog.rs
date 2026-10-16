//! Compiler-facet compile-input definitions in the existing source macro namespace.
use super::{
    macro_names::MacroPath,
    source_macro_routes::{DefinitionRoute, SourceMacroRoutes},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    expand::{
        compile_input_identity::CompileInputDescriptor,
        scopes::{MacroDefinition, MacroName},
    },
    syntax::{outline::Visibility, visibility::VisibilityScope},
};
use std::collections::{BTreeMap, BTreeSet};
#[path = "compile_input_modules.rs"]
mod ordinary_modules;
#[path = "compile_input_catalog/owning_profiles.rs"]
mod owning_profiles;
mod value_binding;
pub(crate) use ordinary_modules::CompilerInjectedImport;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-COMPILE-INPUT", message.into())
}
pub struct CompileInputCatalog {
    unit: Digest,
    edition: u16,
    macro_exports: Vec<
        std::sync::Arc<crate::expand::source_declarative::macro_exports::SourceMacroExportFacet>,
    >,
    facades: BTreeMap<String, Digest>,
    allocation: Option<Digest>,
    definitions: BTreeMap<Digest, (MacroDefinition, CompileInputDescriptor)>,
    attributes: Option<super::builtin_attributes::BuiltinAttributeCatalog>,
    value_binding: Option<value_binding::CompilerValueBindingFacet>,
    primitive_float_methods:
        Option<std::sync::Arc<crate::facade::primitive_float_methods::PrimitiveFloatMethods>>,
}
impl CompileInputCatalog {
    /// Caller supplies a compiler-owned facet. User macros cannot construct descriptors.
    pub fn new(
        unit: Digest,
        edition: u16,
        descriptors: impl IntoIterator<Item = CompileInputDescriptor>,
    ) -> Result<Self, Diagnostic> {
        let mut definitions = BTreeMap::new();
        for descriptor in descriptors {
            if definitions.len() >= 6 {
                return Err(fault("compile-input builtin descriptor count limit"));
            }
            let mut e = StableEncoder::new("lrr/v2/compile-input-definition");
            e.digest(unit);
            e.u32(u32::from(edition));
            e.digest(descriptor.definition_identity());
            let definition = MacroDefinition {
                identity: e.finish(),
                defining_unit: unit,
                definition_edition: edition,
            };
            definition.validate()?;
            if definitions
                .insert(definition.identity, (definition, descriptor))
                .is_some()
            {
                return Err(fault("duplicate compile-input builtin definition"));
            }
        }
        // Empty facets still require a valid source edition.
        MacroName::parse("__lrr_compile_input_facet", edition)?;
        Ok(Self {
            unit,
            edition,
            macro_exports: Vec::new(),
            facades: BTreeMap::new(),
            allocation: None,
            definitions,
            attributes: None,
            value_binding: None,
            primitive_float_methods: None,
        })
    }
    /// Retain actual configured declaration facets separately from compiler
    /// globals and runtime facade bodies. This does not grant source-seal or
    /// macro-use-prelude completion, and does not rewrite core/std identities.
    pub fn with_macro_export_facets(
        mut self,
        facets: impl IntoIterator<
            Item = std::sync::Arc<
                crate::expand::source_declarative::macro_exports::SourceMacroExportFacet,
            >,
        >,
    ) -> Result<Self, Diagnostic> {
        let mut bytes = self.macro_exports.iter().try_fold(0usize, |bytes, facet| {
            bytes
                .checked_add(facet.metadata_bytes())
                .ok_or_else(|| fault("macro-export facet accounting overflow"))
        })?;
        for facet in facets {
            if self
                .macro_exports
                .first()
                .is_some_and(|old| old.preparation() != facet.preparation())
            {
                return Err(fault(
                    "macro-export facets belong to different preparations",
                ));
            }
            if self.macro_exports.len() >= 256 {
                return Err(fault("macro-export facet count limit"));
            }
            if self.macro_exports.iter().any(|old| {
                old.source().selection().location_identity()
                    == facet.source().selection().location_identity()
            }) {
                return Err(fault("duplicate macro-export source facet"));
            }
            bytes = bytes
                .checked_add(facet.metadata_bytes())
                .filter(|n| *n <= 64 * 1024 * 1024)
                .ok_or_else(|| fault("macro-export facet retained budget"))?;
            self.macro_exports
                .try_reserve(1)
                .map_err(|_| fault("macro-export handle allocation"))?;
            self.macro_exports.push(facet);
        }
        self.macro_exports.sort_by_key(|facet| facet.identity());
        Ok(self)
    }
    pub fn macro_export_facets(
        &self,
    ) -> impl Iterator<
        Item = &std::sync::Arc<
            crate::expand::source_declarative::macro_exports::SourceMacroExportFacet,
        >,
    > {
        self.macro_exports.iter()
    }
    /// Construct the pinned compiler facets for a captured standalone profile.
    /// This supplies definition facts only, never expansion authority.
    pub fn standalone(profile: &crate::core::profile::ProfileIdentity) -> Result<Self, Diagnostic> {
        let mut descriptors = Vec::new();
        descriptors
            .try_reserve_exact(6)
            .map_err(|_| fault("compiler descriptor inventory allocation"))?;
        for name in [
            "env",
            "option_env",
            "concat",
            "include",
            "include_str",
            "include_bytes",
        ] {
            descriptors.push(
                crate::expand::compile_input_identity::descriptor(name)?
                    .ok_or_else(|| fault("pinned compiler inventory descriptor absent"))?,
            );
        }
        Self::new(
            crate::facade::unit(profile, "core"),
            profile.edition,
            descriptors,
        )?
        .with_standard_facades(profile)
    }
    pub(crate) fn with_standard_facades(
        mut self,
        profile: &crate::core::profile::ProfileIdentity,
    ) -> Result<Self, Diagnostic> {
        if profile.edition != self.edition || crate::facade::unit(profile, "core") != self.unit {
            return Err(fault("compiler facade owner/profile mismatch"));
        }
        self.allocation = Some(crate::facade::unit(profile, "alloc"));
        self.facades.insert("core".into(), self.unit);
        self.facades
            .insert("std".into(), crate::facade::unit(profile, "std"));
        let descriptors = ["derive", "test"]
            .into_iter()
            .map(|name| {
                crate::expand::builtin_attributes::descriptor(name)?
                    .ok_or_else(|| fault("compiler annotation inventory descriptor absent"))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        self.attributes = Some(super::builtin_attributes::BuiltinAttributeCatalog::new(
            self.unit,
            self.edition,
            descriptors,
        )?);
        self.value_binding = Some(value_binding::CompilerValueBindingFacet::for_profile(
            profile,
        )?);
        self.primitive_float_methods = Some(std::sync::Arc::new(
            crate::facade::primitive_float_methods::PrimitiveFloatMethods::for_profile(profile)?,
        ));
        Ok(self)
    }
    pub(crate) fn runtime_catalog(
        &self,
    ) -> Result<Option<super::runtime_macro_catalog::RuntimeMacroCatalog>, Diagnostic> {
        if self.facades.is_empty() {
            return Ok(None);
        }
        let runtime =
            super::runtime_macro_catalog::RuntimeMacroCatalog::new(self.unit, self.edition)?;
        let runtime = if let Some(standard) = self.facades.get("std") {
            runtime.with_standard_unit(*standard)?
        } else {
            runtime
        };
        let runtime = if let Some(allocation) = self.allocation {
            runtime.with_alloc_unit(allocation)?
        } else {
            runtime
        };
        let runtime = runtime.with_primitive_float_methods(self)?;
        Ok(Some(runtime))
    }
    /// Keep the exact existing foreign Macro inventory, but attach inherent
    /// methods only from this actual canonical owning compiler facet. A set of
    /// participating source profiles is not an inherent-member minting API.
    pub(crate) fn runtime_catalog_for_profiles(
        &self,
        profiles: &BTreeSet<crate::core::profile::ProfileIdentity>,
    ) -> Result<super::runtime_macro_catalog::RuntimeMacroCatalog, Diagnostic> {
        let facet = self
            .primitive_float_methods()?
            .ok_or_else(|| fault("owning primitive method compiler facet is absent"))?;
        super::runtime_macro_catalog::RuntimeMacroCatalog::standard_for_profiles(
            facet.profile(),
            profiles,
        )?
        .with_primitive_float_methods(self)
    }
    pub(super) fn primitive_float_methods(
        &self,
    ) -> Result<
        Option<&std::sync::Arc<crate::facade::primitive_float_methods::PrimitiveFloatMethods>>,
        Diagnostic,
    > {
        let Some(facet) = &self.primitive_float_methods else {
            return Ok(None);
        };
        if self.canonical_standalone_member(facet.profile())?.is_none() {
            return Err(fault("primitive method compiler membership differs"));
        }
        Ok(Some(facet))
    }
    pub(crate) fn runtime_prelude_unit(&self, standard: bool) -> Digest {
        if standard {
            self.facades.get("std").copied().unwrap_or(self.unit)
        } else {
            self.unit
        }
    }
    pub fn attribute_catalog(&self) -> Option<&super::builtin_attributes::BuiltinAttributeCatalog> {
        self.attributes.as_ref()
    }
    pub(crate) fn facade_aliases(&self) -> impl Iterator<Item = (&str, Digest)> {
        self.facades
            .iter()
            .map(|(name, unit)| (name.as_str(), *unit))
    }
    /// Compiler globals are distinct from the disableable macro-use prelude.
    /// Only an explicitly configured standard facade may supply these identities.
    pub(crate) fn compiler_globals(&self) -> Result<Vec<(MacroName, MacroDefinition)>, Diagnostic> {
        let mut globals = Vec::new();
        if self.facades.is_empty() {
            return Ok(globals);
        }
        globals
            .try_reserve_exact(10)
            .map_err(|_| fault("compiler global inventory allocation"))?;
        for (definition, descriptor) in self.definitions.values() {
            globals.push((
                MacroName::parse(descriptor.name(), self.edition)?,
                definition.clone(),
            ));
        }
        if let Some(attributes) = &self.attributes {
            for definition in attributes.definitions() {
                let descriptor = attributes
                    .select(definition)?
                    .ok_or_else(|| fault("compiler annotation inventory identity absent"))?;
                globals.push((
                    MacroName::parse(descriptor.name(), self.edition)?,
                    definition.clone(),
                ));
            }
        }
        let runtime =
            super::runtime_macro_catalog::RuntimeMacroCatalog::new(self.unit, self.edition)?;
        for definition in runtime.definitions() {
            if let Some(
                kind @ (crate::facade::macros::RuntimeMacroKind::Panic
                | crate::facade::macros::RuntimeMacroKind::Assert),
            ) = runtime.select(definition)?
            {
                globals.push((
                    MacroName::parse(kind.name(), self.edition)?,
                    definition.clone(),
                ));
            }
        }
        Ok(globals)
    }
    pub(crate) fn source_exports(&self, unit: Digest) -> Result<SourceMacroRoutes, Diagnostic> {
        self.source_routes_for(unit)
    }
    pub fn definitions(&self) -> impl Iterator<Item = &MacroDefinition> {
        self.definitions.values().map(|(d, _)| d)
    }
    pub fn select(
        &self,
        definition: &MacroDefinition,
    ) -> Result<Option<&CompileInputDescriptor>, Diagnostic> {
        let Some((expected, descriptor)) = self.definitions.get(&definition.identity) else {
            return Ok(None);
        };
        if expected != definition {
            return Err(fault("compile-input builtin definition provenance differs"));
        }
        Ok(Some(descriptor))
    }
    /// Original compiler root exports; aliases/reexports retain these exact definitions.
    pub fn source_routes(&self) -> Result<SourceMacroRoutes, Diagnostic> {
        self.source_routes_for(self.unit)
    }
    fn source_routes_for(&self, exporting_unit: Digest) -> Result<SourceMacroRoutes, Diagnostic> {
        let root = (exporting_unit, vec![]);
        let mut routes = SourceMacroRoutes {
            extern_aliases: vec![],
            export_pending: std::collections::BTreeSet::new(),
            body_owner_pending: std::collections::BTreeMap::new(),
            binding_origin: None,
            definitions: vec![],
            imports: vec![],
            deferred: vec![],
            module_access: BTreeMap::from([(root.clone(), VisibilityScope::Public)]),
            edition: self.edition,
            modules: BTreeSet::from([root]),
            dependencies: BTreeMap::new(),
            source_imports: vec![],
            textual_import_definitions: Default::default(),
            implicit_disabled: BTreeSet::new(),
        };
        for (definition, descriptor) in self.definitions.values() {
            routes.definitions.push(DefinitionRoute {
                path: MacroPath::new(
                    exporting_unit,
                    vec![MacroName::parse(descriptor.name(), self.edition)?],
                )?,
                definition: definition.clone(),
                visibility: Visibility::Public,
            });
        }
        if !self.facades.is_empty() {
            let runtime = self
                .runtime_catalog()?
                .ok_or_else(|| fault("standard runtime catalog absent"))?;
            let exports = runtime.source_exports(exporting_unit)?;
            routes
                .definitions
                .try_reserve_exact(exports.definitions.len())
                .map_err(|_| fault("SDK macro inventory allocation"))?;
            routes.definitions.extend(exports.definitions);
        }
        if let Some(attributes) = &self.attributes {
            // Paths reexport the original core definitions; std never acquires
            // fresh annotation identities. These are bounded compiler inventory
            // modules, not a declaration-completeness or execution grant.
            for prelude in [
                None,
                Some("v1"),
                Some("rust_2015"),
                Some("rust_2018"),
                Some("rust_2021"),
                Some("rust_2024"),
            ] {
                let prefix = match prelude {
                    Some(name) => vec![
                        MacroName::parse("prelude", self.edition)?,
                        MacroName::parse(name, self.edition)?,
                    ],
                    None => vec![],
                };
                for facet in attributes.source_routes(&prefix)? {
                    routes
                        .definitions
                        .try_reserve_exact(facet.definitions.len())
                        .map_err(|_| fault("compiler annotation route allocation"))?;
                    for definition in facet.definitions {
                        // `test` is exported by the edition preludes, not by
                        // the core/std crate roots. Compiler-global availability
                        // does not manufacture a public root export.
                        if prelude.is_none()
                            && definition
                                .path
                                .components()
                                .last()
                                .is_some_and(|name| name.as_str() == "test")
                        {
                            continue;
                        }
                        routes.definitions.push(DefinitionRoute {
                            path: MacroPath::new(
                                exporting_unit,
                                definition.path.components().to_vec(),
                            )?,
                            ..definition
                        });
                    }
                    for (_, path) in facet.modules {
                        routes.modules.insert((exporting_unit, path));
                    }
                    for ((_, path), visibility) in facet.module_access {
                        routes
                            .module_access
                            .insert((exporting_unit, path), visibility);
                    }
                }
            }
        }
        Ok(routes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_annotation_reexports_keep_exact_definition_across_core_and_std() {
        for edition in [2015, 2018, 2021, 2024] {
            let mut profile = crate::core::profile::ProfileIdentity::current();
            profile.edition = edition;
            let catalog = CompileInputCatalog::standalone(&profile).unwrap();
            let attributes = catalog.attribute_catalog().unwrap();
            let globals = catalog.compiler_globals().unwrap();
            for (_, unit) in catalog.facade_aliases() {
                let names =
                    SourceMacroRoutes::candidate_graph(&[catalog.source_exports(unit).unwrap()])
                        .unwrap();
                for definition in attributes.definitions() {
                    let descriptor = attributes.select(definition).unwrap().unwrap();
                    assert!(
                        globals
                            .iter()
                            .any(|(name, value)| name.as_str() == descriptor.name()
                                && value == definition)
                    );
                    for prelude in [
                        None,
                        Some("v1"),
                        Some("rust_2015"),
                        Some("rust_2018"),
                        Some("rust_2021"),
                        Some("rust_2024"),
                    ] {
                        let mut path = match prelude {
                            Some(name) => vec![
                                MacroName::parse("prelude", edition).unwrap(),
                                MacroName::parse(name, edition).unwrap(),
                            ],
                            None => vec![],
                        };
                        path.push(MacroName::parse(descriptor.name(), edition).unwrap());
                        let expected = if prelude.is_none() && descriptor.name() == "test" {
                            super::super::macro_names::PathLookup::Absent
                        } else {
                            super::super::macro_names::PathLookup::Defined(definition.clone())
                        };
                        assert_eq!(
                            names.lookup(&MacroPath::new(unit, path).unwrap()).unwrap(),
                            expected
                        );
                        assert_eq!(
                            definition.defining_unit,
                            crate::facade::unit(&profile, "core")
                        );
                    }
                }
            }
        }
    }
}
