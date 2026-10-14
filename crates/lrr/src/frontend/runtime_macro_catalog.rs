//! Compiler-owned runtime macro facets in the existing source namespace.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    expand::scopes::{MacroDefinition, MacroName},
    facade::macros::RuntimeMacroKind,
    frontend::{
        macro_names::MacroPath,
        source_macro_routes::{DefinitionRoute, SourceMacroRoutes},
    },
    syntax::{outline::Visibility, visibility::VisibilityScope},
};
use std::collections::{BTreeMap, BTreeSet};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-RUNTIME-MACRO", message.into())
}
/// An immutable compiler facet in one exact source world. There is no API to
/// register a guest/user definition as a runtime intrinsic.
pub struct RuntimeMacroCatalog {
    unit: Digest,
    edition: u16,
    standard: Option<Digest>,
    allocation: Option<Digest>,
    definitions: BTreeMap<Digest, (MacroDefinition, RuntimeMacroKind)>,
    // Selection-only standard facets from actual other participating source
    // profiles. They are never exposed as this catalogue's local exports.
    foreign_definitions: BTreeMap<Digest, (MacroDefinition, RuntimeMacroKind)>,
}
impl RuntimeMacroCatalog {
    /// The pinned standard inventory used both during source preparation and
    /// during database/generated certificate revalidation. This confers no
    /// runtime authority and cannot accept a user definition by its spelling.
    pub fn standard(profile: &crate::core::profile::ProfileIdentity) -> Result<Self, Diagnostic> {
        profile.validate_source()?;
        Self::new(crate::facade::unit(profile, "core"), profile.edition)?
            .with_standard_unit(crate::facade::unit(profile, "std"))?
            .with_alloc_unit(crate::facade::unit(profile, "alloc"))
    }
    pub(crate) fn standard_for_profiles(
        owning: &crate::core::profile::ProfileIdentity,
        profiles: &BTreeSet<crate::core::profile::ProfileIdentity>,
    ) -> Result<Self, Diagnostic> {
        if profiles.len() > 4 {
            return Err(Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "runtime macro owning-profile count limit".into(),
            ));
        }
        if !profiles.contains(owning) {
            return Err(fault("runtime macro owning profile is absent"));
        }
        let mut result = Self::standard(owning)?;
        for profile in profiles {
            profile.validate_source()?;
            if profile.semantics != owning.semantics {
                return Err(fault("runtime macro source semantics differ"));
            }
            if profile == owning {
                continue;
            }
            // No API admits a caller-built definition or arbitrary unit here.
            // Canonical IDs still encode the actual unit and defining edition.
            for (identity, member) in Self::standard(profile)?.definitions {
                if result.definitions.contains_key(&identity)
                    || result
                        .foreign_definitions
                        .insert(identity, member)
                        .is_some()
                {
                    return Err(fault("runtime macro standard-world identity collision"));
                }
            }
        }
        Ok(result)
    }
    /// Reconstruct only the pinned standard facet in a resolved definition's
    /// world. Invocation syntax keeps its own profile; select still checks the
    /// full definition identity/unit/edition and cannot admit a user macro.
    pub(crate) fn standard_for_definition(
        source: &crate::core::profile::ProfileIdentity,
        definition: &MacroDefinition,
    ) -> Result<Self, Diagnostic> {
        source.validate_source()?;
        definition.validate()?;
        let profile = crate::core::profile::ProfileIdentity {
            edition: definition.definition_edition,
            semantics: source.semantics.clone(),
        };
        profile.validate_source()?;
        Self::standard(&profile)
    }
    pub fn new(unit: Digest, edition: u16) -> Result<Self, Diagnostic> {
        MacroName::parse("__lrr_runtime_macro_facet", edition)?;
        let mut definitions = BTreeMap::new();
        for kind in [
            RuntimeMacroKind::Panic,
            RuntimeMacroKind::Assert,
            RuntimeMacroKind::AssertEq,
            RuntimeMacroKind::AssertNe,
            RuntimeMacroKind::Unreachable,
            RuntimeMacroKind::Unimplemented,
        ] {
            let mut e = StableEncoder::new("lrr/v2/runtime-macro-definition");
            e.digest(unit);
            e.u32(u32::from(edition));
            e.u32(kind.tag());
            let definition = MacroDefinition {
                identity: e.finish(),
                defining_unit: unit,
                definition_edition: edition,
            };
            definition.validate()?;
            if definitions
                .insert(definition.identity, (definition, kind))
                .is_some()
            {
                return Err(fault("runtime macro definition identity collision"));
            }
        }
        Ok(Self {
            unit,
            edition,
            standard: None,
            allocation: None,
            definitions,
            foreign_definitions: BTreeMap::new(),
        })
    }
    /// Std-owned definitions remain distinct from the core runtime facet. This
    /// registers source identity and syntax effects, never execution authority.
    pub(crate) fn with_standard_unit(mut self, standard: Digest) -> Result<Self, Diagnostic> {
        if standard == self.unit {
            return Err(fault("standard runtime macro unit aliases core"));
        }
        if self.standard.is_some() || self.allocation == Some(standard) {
            return Err(fault(
                "standard runtime macro facet already registered or aliases alloc",
            ));
        }
        self.standard = Some(standard);
        let kind = RuntimeMacroKind::Println;
        let mut e = StableEncoder::new("lrr/v2/runtime-macro-definition");
        e.digest(standard);
        e.u32(u32::from(self.edition));
        e.u32(kind.tag());
        let definition = MacroDefinition {
            identity: e.finish(),
            defining_unit: standard,
            definition_edition: self.edition,
        };
        definition.validate()?;
        if self
            .definitions
            .insert(definition.identity, (definition, kind))
            .is_some()
        {
            return Err(fault("standard runtime macro identity already registered"));
        }
        Ok(self)
    }
    /// Pinned alloc owns vec; std may reexport that exact definition. No extern alias is added.
    pub(crate) fn with_alloc_unit(mut self, allocation: Digest) -> Result<Self, Diagnostic> {
        if allocation == self.unit || self.standard == Some(allocation) || self.allocation.is_some()
        {
            return Err(fault(
                "alloc runtime macro unit aliases another facet or is already registered",
            ));
        }
        let kind = RuntimeMacroKind::Vec;
        let mut e = StableEncoder::new("lrr/v2/runtime-macro-definition");
        e.digest(allocation);
        e.u32(u32::from(self.edition));
        e.u32(kind.tag());
        let definition = MacroDefinition {
            identity: e.finish(),
            defining_unit: allocation,
            definition_edition: self.edition,
        };
        definition.validate()?;
        if self
            .definitions
            .insert(definition.identity, (definition, kind))
            .is_some()
        {
            return Err(fault("alloc runtime macro identity already registered"));
        }
        self.allocation = Some(allocation);
        Ok(self)
    }
    pub(crate) fn source_exports(
        &self,
        exporting_unit: Digest,
    ) -> Result<SourceMacroRoutes, Diagnostic> {
        let mut routes = self.source_routes()?;
        if self.allocation == Some(exporting_unit) {
            routes.definitions.clear();
        }
        routes.modules = BTreeSet::from([(exporting_unit, vec![])]);
        routes.module_access =
            BTreeMap::from([((exporting_unit, vec![]), VisibilityScope::Public)]);
        for route in &mut routes.definitions {
            route.path = MacroPath::new(exporting_unit, route.path.components().to_vec())?;
        }
        for (definition, kind) in self.definitions.values() {
            if definition.defining_unit == exporting_unit && definition.defining_unit != self.unit
                || *kind == RuntimeMacroKind::Vec && self.standard == Some(exporting_unit)
            {
                routes.definitions.push(DefinitionRoute {
                    path: MacroPath::new(
                        exporting_unit,
                        vec![MacroName::parse(kind.name(), self.edition)?],
                    )?,
                    definition: definition.clone(),
                    visibility: Visibility::Public,
                });
            }
        }
        Ok(routes)
    }
    pub fn definitions(&self) -> impl Iterator<Item = &MacroDefinition> {
        self.definitions.values().map(|(definition, _)| definition)
    }
    /// Called after the existing lexical/path/hygiene resolver selects a definition.
    /// An ordinary user definition remains ordinary even if its name is `panic`.
    pub fn select(
        &self,
        definition: &MacroDefinition,
    ) -> Result<Option<RuntimeMacroKind>, Diagnostic> {
        definition.validate()?;
        let Some((expected, kind)) = self
            .definitions
            .get(&definition.identity)
            .or_else(|| self.foreign_definitions.get(&definition.identity))
        else {
            return Ok(None);
        };
        if definition != expected {
            return Err(fault("runtime macro definition provenance differs"));
        }
        Ok(Some(*kind))
    }
    /// Root exports can be renamed/reexported by the ordinary source namespace.
    pub fn source_routes(&self) -> Result<SourceMacroRoutes, Diagnostic> {
        let root = (self.unit, vec![]);
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
        for (definition, kind) in self.definitions.values() {
            if definition.defining_unit != self.unit {
                continue;
            }
            routes.definitions.push(DefinitionRoute {
                path: MacroPath::new(
                    self.unit,
                    vec![MacroName::parse(kind.name(), self.edition)?],
                )?,
                definition: definition.clone(),
                visibility: Visibility::Public,
            });
        }
        Ok(routes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::profile::ProfileIdentity;
    fn profile(edition: u16) -> ProfileIdentity {
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        }
    }
    #[test]
    fn owning_runtime_worlds_select_exact_foreign_facets_without_exporting_them_locally() {
        let profiles = [2015, 2018, 2021, 2024]
            .into_iter()
            .map(profile)
            .collect::<BTreeSet<_>>();
        for owning in &profiles {
            let catalog = RuntimeMacroCatalog::standard_for_profiles(owning, &profiles).unwrap();
            let own = RuntimeMacroCatalog::standard(owning).unwrap();
            assert_eq!(
                catalog.definitions().collect::<Vec<_>>(),
                own.definitions().collect::<Vec<_>>()
            );
            assert_eq!(
                catalog.source_routes().unwrap().definitions().len(),
                own.source_routes().unwrap().definitions().len()
            );
            let standard = crate::facade::unit(owning, "std");
            let exports = catalog.source_exports(standard).unwrap();
            assert_eq!(
                exports.definitions().len(),
                own.source_exports(standard).unwrap().definitions().len()
            );
            for foreign in &profiles {
                let exact = RuntimeMacroCatalog::standard(foreign).unwrap();
                for definition in exact.definitions() {
                    assert_eq!(
                        catalog.select(definition).unwrap(),
                        exact.select(definition).unwrap()
                    );
                    let mut forged = definition.clone();
                    forged.defining_unit = Digest([226; 32]);
                    assert_eq!(
                        catalog.select(&forged).unwrap_err().code,
                        "LRR-E-RUNTIME-MACRO"
                    );
                }
            }
            let user = MacroDefinition {
                identity: Digest([227; 32]),
                defining_unit: catalog.unit,
                definition_edition: owning.edition,
            };
            assert!(catalog.select(&user).unwrap().is_none());
        }
    }
    #[test]
    fn unrepresented_runtime_worlds_and_missing_owners_never_gain_intrinsic_identity() {
        let own = profile(2024);
        let profiles = BTreeSet::from([own.clone()]);
        let catalog = RuntimeMacroCatalog::standard_for_profiles(&own, &profiles).unwrap();
        for definition in RuntimeMacroCatalog::standard(&profile(2015))
            .unwrap()
            .definitions()
        {
            assert!(catalog.select(definition).unwrap().is_none());
        }
        assert!(RuntimeMacroCatalog::standard_for_profiles(&profile(2015), &profiles).is_err());
        let mut too_many = profiles.clone();
        for edition in [2015, 2018, 2021, 2099] {
            too_many.insert(profile(edition));
        }
        assert_eq!(
            RuntimeMacroCatalog::standard_for_profiles(&own, &too_many)
                .err()
                .unwrap()
                .code,
            "LRR-E-METADATA-LIMIT"
        );
    }
}
