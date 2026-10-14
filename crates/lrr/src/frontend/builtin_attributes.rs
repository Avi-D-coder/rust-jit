//! Owned compiler-facet definitions. No spelling-based call-site certification.
use super::{
    item_attributes::CustomAttributeDecision,
    macro_names::MacroPath,
    source_macro_routes::{DefinitionRoute, SourceMacroRoutes},
};
use crate::expand::builtin_attributes::BuiltinAttributeDescriptor;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    expand::scopes::{MacroDefinition, MacroName},
    syntax::{outline::Visibility, visibility::VisibilityScope},
};
use std::collections::{BTreeMap, BTreeSet};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
pub struct BuiltinAttributeCatalog {
    definitions: BTreeMap<Digest, (MacroDefinition, BuiltinAttributeDescriptor)>,
}
pub enum AttributeDecision {
    Custom(CustomAttributeDecision),
    Builtin(BuiltinAttributeDescriptor),
}
impl BuiltinAttributeCatalog {
    /// Caller supplies the original compiler-owned defining unit/edition facet.
    /// Reexports must preserve these definitions rather than construct new ones.
    pub fn new(
        unit: Digest,
        edition: u16,
        descriptors: impl IntoIterator<Item = BuiltinAttributeDescriptor>,
    ) -> Result<Self, Diagnostic> {
        let mut definitions = BTreeMap::new();
        let mut count = 0usize;
        for descriptor in descriptors {
            count += 1;
            if count > 64 {
                return Err(fault("builtin attribute descriptor count limit"));
            }
            let mut e = StableEncoder::new("lrr/v2/builtin-attribute-definition");
            e.digest(unit);
            e.u32(u32::from(edition));
            e.bytes(&descriptor.definition_identity().0);
            let definition = MacroDefinition {
                identity: e.finish(),
                defining_unit: unit,
                definition_edition: edition,
            };
            definition.validate()?;
            definitions.insert(definition.identity, (definition, descriptor));
        }
        Ok(Self { definitions })
    }
    pub fn definitions(&self) -> impl Iterator<Item = &MacroDefinition> {
        self.definitions.values().map(|(d, _)| d)
    }
    pub fn select(
        &self,
        definition: &MacroDefinition,
    ) -> Result<Option<&BuiltinAttributeDescriptor>, Diagnostic> {
        let Some((expected, descriptor)) = self.definitions.get(&definition.identity) else {
            return Ok(None);
        };
        if expected != definition {
            return Err(fault("builtin attribute definition provenance differs"));
        }
        Ok(Some(descriptor))
    }
    /// Resolve only a retained opaque definition. Procedural/helper outcomes remain intact.
    pub fn classify(
        &self,
        decision: CustomAttributeDecision,
    ) -> Result<AttributeDecision, Diagnostic> {
        if let CustomAttributeDecision::NeedDefinition(definition) = &decision
            && let Some(descriptor) = self.select(definition)?
        {
            return Ok(AttributeDecision::Builtin(descriptor.clone()));
        }
        Ok(AttributeDecision::Custom(decision))
    }
    /// Expose a compiler facet at its declared module path for the existing source graph.
    pub fn source_routes(
        &self,
        prefix: &[MacroName],
    ) -> Result<Vec<SourceMacroRoutes>, Diagnostic> {
        if prefix.len() > 255 {
            return Err(fault("builtin attribute module depth limit"));
        }
        let mut bytes = prefix
            .len()
            .checked_mul(256)
            .ok_or_else(|| fault("builtin route size overflow"))?;
        for (index, name) in prefix.iter().enumerate() {
            let copies = (prefix.len() - index)
                .checked_mul(2)
                .and_then(|n| n.checked_add(self.definitions.len()))
                .ok_or_else(|| fault("builtin route size overflow"))?;
            let amount = name
                .as_str()
                .len()
                .checked_mul(3)
                .and_then(|n| n.checked_add(std::mem::size_of::<MacroName>()))
                .and_then(|n| n.checked_mul(copies))
                .ok_or_else(|| fault("builtin route size overflow"))?;
            bytes = bytes
                .checked_add(amount)
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("builtin route metadata limit"))?;
        }
        let mut units: BTreeMap<Digest, SourceMacroRoutes> = BTreeMap::new();
        for (definition, descriptor) in self.definitions.values() {
            let routes =
                units
                    .entry(definition.defining_unit)
                    .or_insert_with(|| SourceMacroRoutes {
                        extern_aliases: vec![],
                        export_pending: std::collections::BTreeSet::new(),
                        body_owner_pending: std::collections::BTreeMap::new(),
                        binding_origin: None,
                        definitions: vec![],
                        imports: vec![],
                        deferred: vec![],
                        module_access: BTreeMap::new(),
                        edition: definition.definition_edition,
                        modules: BTreeSet::new(),
                        dependencies: BTreeMap::new(),
                        source_imports: vec![],
                        textual_import_definitions: Default::default(),
                        implicit_disabled: BTreeSet::new(),
                    });
            for depth in 0..=prefix.len() {
                let key = (definition.defining_unit, prefix[..depth].to_vec());
                routes.modules.insert(key.clone());
                routes.module_access.insert(key, VisibilityScope::Public);
            }
            let mut names = prefix.to_vec();
            names.push(MacroName::parse(
                descriptor.name(),
                definition.definition_edition,
            )?);
            routes.definitions.push(DefinitionRoute {
                path: MacroPath::new(definition.defining_unit, names)?,
                definition: definition.clone(),
                visibility: Visibility::Public,
            });
        }
        Ok(units.into_values().collect())
    }
}
