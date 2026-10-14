//! Frozen prepared procedural exports in the existing macro namespace.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        identity::{MacroIdentity, ProceduralKind},
        scopes::{MacroDefinition, MacroName},
    },
};
use std::collections::BTreeMap;
use std::sync::Arc;
/// Immutable producer span facts captured by the authorized service. Consumers
/// only merge these facts; reading a catalog never opens files or starts tools.
#[derive(Clone)]
pub(crate) struct PreparedDefinitionSite {
    pub site: crate::syntax::token_wire::Key,
    pub registry: crate::syntax::token_wire::TokenForest,
    pub source: crate::syntax::token_wire::SourceIdentity,
    pub text: Arc<str>,
    pub bytes: usize,
    pub fingerprint: Digest,
    pub diagnostic_definitions:
        BTreeMap<ProceduralKind, BTreeMap<String, crate::syntax::spans::SourceRange>>,
}
impl std::fmt::Debug for PreparedDefinitionSite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedDefinitionSite")
            .field("site", &self.site)
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}
impl PreparedDefinitionSite {
    pub(crate) fn diagnostic_definition(
        &self,
        form: ProceduralKind,
        export: &str,
    ) -> Option<crate::syntax::spans::SourceRange> {
        self.diagnostic_definitions.get(&form)?.get(export).copied()
    }
    pub(crate) fn merge_into(
        &self,
        input: &crate::syntax::token_wire::TokenForest,
    ) -> Result<crate::syntax::token_wire::TokenForest, Diagnostic> {
        let mut contexts: BTreeMap<_, _> = input
            .contexts()
            .iter()
            .map(|row| (row.key(), row.clone()))
            .collect();
        for row in self.registry.contexts() {
            if let Some(old) = contexts.get(&row.key()) {
                if old != row {
                    return Err(fault("producer definition context conflicts"));
                }
            } else {
                contexts.insert(row.key(), row.clone());
            }
        }
        let mut spans: BTreeMap<_, _> = input
            .spans()
            .iter()
            .map(|row| (row.key(), row.clone()))
            .collect();
        for row in self.registry.spans() {
            if let Some(old) = spans.get(&row.key()) {
                if old != row {
                    return Err(fault("producer definition span conflicts"));
                }
            } else {
                spans.insert(row.key(), row.clone());
            }
        }
        let merged = crate::syntax::token_wire::TokenForest::new(
            input.roots().to_vec(),
            contexts.into_values().collect(),
            spans.into_values().collect(),
        )
        .map_err(|e| fault(&e.message))?;
        if merged.allocated_bytes().map_err(|e| fault(&e.message))? > 32 * 1024 * 1024 {
            return Err(fault("producer definition merged wire byte limit"));
        }
        Ok(merged)
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
#[derive(Debug, Clone)]
pub struct ProceduralExport {
    definition: MacroDefinition,
    name: MacroName,
    identity: MacroIdentity,
}
impl ProceduralExport {
    pub fn definition(&self) -> &MacroDefinition {
        &self.definition
    }
    pub fn name(&self) -> &MacroName {
        &self.name
    }
    pub fn identity(&self) -> &MacroIdentity {
        &self.identity
    }
}
pub(crate) struct PreparedProcBinding {
    pub consumer: Digest,
    pub graph: Digest,
    pub epoch: u64,
    pub environment: Digest,
    pub preparation: Digest,
    pub aliases: BTreeMap<MacroName, Digest>,
}
#[derive(Debug, Clone)]
pub struct PreparedProcCatalog {
    exports: BTreeMap<Digest, ProceduralExport>,
    consumer: Digest,
    graph: Digest,
    epoch: u64,
    environment: Digest,
    preparation: Digest,
    aliases: BTreeMap<MacroName, Digest>,
    definition_sites: BTreeMap<Digest, Arc<PreparedDefinitionSite>>,
}
impl PreparedProcCatalog {
    /// Only the authorized service supplies retained, exactly loaded export facts.
    pub(crate) fn from_exports<'a>(
        rows: impl IntoIterator<Item = (Digest, Digest, u16, &'a str, ProceduralKind)>,
        binding: PreparedProcBinding,
    ) -> Result<Self, Diagnostic> {
        let mut exports = BTreeMap::new();
        let mut bytes = 0usize;
        for (artifact, unit, edition, export, kind) in rows {
            if exports.len() >= 4096 {
                return Err(fault("prepared procedural export count limit"));
            }
            bytes = bytes
                .checked_add(
                    export
                        .len()
                        .checked_mul(3)
                        .ok_or_else(|| fault("prepared export size overflow"))?,
                )
                .and_then(|n| n.checked_add(std::mem::size_of::<ProceduralExport>() + 256))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| fault("prepared export metadata limit"))?;
            let name = MacroName::parse(&format!("r#{export}"), edition)?;
            let mut e = StableEncoder::new("lrr/v2/prepared-procedural-definition");
            e.digest(unit);
            e.digest(artifact);
            e.u32(u32::from(edition));
            e.string(export);
            e.u8(match kind {
                ProceduralKind::Function => 0,
                ProceduralKind::Attribute => 1,
                ProceduralKind::Derive => 2,
            });
            let definition = MacroDefinition {
                identity: e.finish(),
                defining_unit: unit,
                definition_edition: edition,
            };
            definition.validate()?;
            exports.insert(
                definition.identity,
                ProceduralExport {
                    definition,
                    name,
                    identity: MacroIdentity::Procedural {
                        artifact,
                        export: export.into(),
                        kind,
                    },
                },
            );
        }
        if binding.epoch == 0
            || binding.aliases.len() > 100_000
            || binding.aliases.values().any(|unit| {
                !exports
                    .values()
                    .any(|export| export.definition.defining_unit == *unit)
            })
        {
            return Err(fault("invalid prepared macro binding/alias"));
        }
        Ok(Self {
            exports,
            consumer: binding.consumer,
            graph: binding.graph,
            epoch: binding.epoch,
            environment: binding.environment,
            preparation: binding.preparation,
            aliases: binding.aliases,
            definition_sites: BTreeMap::new(),
        })
    }
    pub(crate) fn bind_definition_sites(
        &mut self,
        sites: BTreeMap<Digest, Arc<PreparedDefinitionSite>>,
    ) -> Result<(), Diagnostic> {
        if sites.len() > 4096 {
            return Err(fault("prepared definition site count limit"));
        }
        let mut producers = BTreeMap::new();
        for export in self.exports.values() {
            let MacroIdentity::Procedural { artifact, .. } = export.identity else {
                return Err(fault(
                    "definition site catalog contains a non-procedural identity",
                ));
            };
            let expected = (
                export.definition.defining_unit,
                export.definition.definition_edition,
            );
            if let Some(old) = producers.insert(artifact, expected)
                && old != expected
            {
                return Err(fault(
                    "definition site artifact has conflicting producer metadata",
                ));
            }
        }
        for (artifact, site) in &sites {
            site.registry.validate().map_err(|e| fault(&e.message))?;
            let span = site
                .registry
                .spans()
                .iter()
                .find(|span| span.key() == site.site)
                .ok_or_else(|| fault("prepared definition site span absent"))?;
            let expected = producers
                .get(artifact)
                .ok_or_else(|| fault("definition site artifact has no loaded export"))?;
            let edition = match span.edition {
                crate::syntax::token_wire::Edition::E2015 => 2015,
                crate::syntax::token_wire::Edition::E2018 => 2018,
                crate::syntax::token_wire::Edition::E2021 => 2021,
                crate::syntax::token_wire::Edition::E2024 => 2024,
            };
            for at in site
                .diagnostic_definitions
                .values()
                .flat_map(BTreeMap::values)
            {
                if at.start > at.end || site.text.get(at.start as usize..at.end as usize).is_none()
                {
                    return Err(fault(
                        "prepared procedural definition span exceeds captured source",
                    ));
                }
            }
            if span.source != site.source
                || expected.0.0 != span.source.unit.0
                || expected.1 != edition
            {
                return Err(fault("prepared definition site/export provenance differs"));
            }
        }
        sites
            .values()
            .try_fold(0usize, |n, site| n.checked_add(site.bytes))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| fault("prepared definition site byte limit"))?;
        self.definition_sites = sites;
        self.metadata_bytes()?;
        Ok(())
    }
    pub(crate) fn definition_site(&self, artifact: Digest) -> Option<&PreparedDefinitionSite> {
        self.definition_sites.get(&artifact).map(Arc::as_ref)
    }
    pub(crate) fn consumer(&self) -> Digest {
        self.consumer
    }
    pub(crate) fn graph(&self) -> Digest {
        self.graph
    }
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }
    pub(crate) fn environment(&self) -> Digest {
        self.environment
    }
    pub(crate) fn preparation(&self) -> Digest {
        self.preparation
    }
    pub(crate) fn aliases(&self) -> &BTreeMap<MacroName, Digest> {
        &self.aliases
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>();
        for site in self.definition_sites.values() {
            bytes = bytes
                .checked_add(site.bytes)
                .and_then(|n| n.checked_add(160))
                .ok_or_else(|| fault("prepared definition site metadata overflow"))?;
        }
        for name in self.aliases.keys() {
            bytes = bytes
                .checked_add(name.as_str().len())
                .and_then(|n| n.checked_add(160))
                .ok_or_else(|| fault("prepared alias metadata overflow"))?;
        }
        for export in self.exports.values() {
            let MacroIdentity::Procedural { export: name, .. } = &export.identity else {
                unreachable!("private procedural catalog");
            };
            bytes = bytes
                .checked_add(name.capacity())
                .and_then(|n| n.checked_add(export.name.as_str().len()))
                .and_then(|n| n.checked_add(384))
                .ok_or_else(|| fault("prepared export metadata overflow"))?;
        }
        if bytes > 64 * 1024 * 1024 {
            return Err(fault("prepared catalog metadata limit"));
        }
        Ok(bytes)
    }
    pub fn exports(&self) -> impl Iterator<Item = &ProceduralExport> {
        self.exports.values()
    }
    /// Resolve a namespace candidate back to its exact backend identity.
    /// A different namespace kind must never dispatch as an item transform.
    pub(crate) fn attribute_candidate(
        &self,
        definition: &MacroDefinition,
    ) -> Result<bool, Diagnostic> {
        let Some(export) = self.exports.get(&definition.identity) else {
            return Ok(false);
        };
        if &export.definition != definition {
            return Err(fault("procedural attribute definition provenance differs"));
        }
        Ok(matches!(
            export.identity,
            MacroIdentity::Procedural {
                kind: ProceduralKind::Attribute,
                ..
            }
        ))
    }
    pub fn select(
        &self,
        definition: &MacroDefinition,
        expected: ProceduralKind,
    ) -> Result<Option<&ProceduralExport>, Diagnostic> {
        let Some(export) = self.exports.get(&definition.identity) else {
            return Ok(None);
        };
        if &export.definition != definition {
            return Err(fault("procedural definition provenance differs"));
        }
        if !matches!(export.identity, MacroIdentity::Procedural { kind, .. } if kind == expected) {
            return Err(fault(
                "resolved procedural macro kind differs from invocation",
            ));
        }
        Ok(Some(export))
    }
}
impl StableEncode for PreparedProcCatalog {
    const DOMAIN: &'static str = "lrr/v2/prepared-procedural-catalog";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.consumer);
        e.digest(self.graph);
        e.u64(self.epoch);
        e.digest(self.environment);
        e.digest(self.preparation);
        e.u64(u64::try_from(self.definition_sites.len()).expect("bounded definition sites"));
        for (artifact, site) in &self.definition_sites {
            e.digest(*artifact);
            e.digest(site.fingerprint);
        }
        e.u64(u64::try_from(self.aliases.len()).expect("bounded macro aliases"));
        for (name, unit) in &self.aliases {
            e.string(name.as_str());
            e.digest(*unit);
        }
        e.u64(u64::try_from(self.exports.len()).expect("bounded exports"));
        for (id, export) in &self.exports {
            e.digest(*id);
            e.digest(export.definition.defining_unit);
            e.u16(export.definition.definition_edition);
            e.string(export.name.as_str());
            let MacroIdentity::Procedural {
                artifact,
                export: name,
                kind,
            } = &export.identity
            else {
                unreachable!("private catalog contains only procedural exports");
            };
            e.digest(*artifact);
            e.string(name);
            e.u8(match kind {
                ProceduralKind::Function => 0,
                ProceduralKind::Attribute => 1,
                ProceduralKind::Derive => 2,
            });
        }
    }
}

impl PreparedProcCatalog {
    /// Copy already admitted definition facts for an exactly selected request.
    /// This performs no source read, query, preparation or macro invocation.
    pub(crate) fn source_failure_origin(
        &self,
        definition: &MacroDefinition,
        key: &crate::expand::identity::ExpansionKey,
        invocation: crate::syntax::origin::SourceOrigin,
    ) -> Result<crate::syntax::origin::MacroOrigin, Diagnostic> {
        use crate::syntax::origin::{DefinitionOrigin, MacroOrigin, SourceOrigin};
        let MacroIdentity::Procedural {
            artifact,
            export,
            kind,
        } = &key.macro_identity
        else {
            return Err(fault("source failure origin requires a procedural request"));
        };
        let selected = self
            .select(definition, *kind)?
            .ok_or_else(|| fault("source failure origin selected export absent"))?;
        if selected.identity() != &key.macro_identity
            || key.consumer_compile_env != self.environment()
            || key.procedural_epoch != Some(self.epoch())
            || invocation.source.unit.0 != self.consumer().0
        {
            return Err(fault("source failure origin differs from captured catalog"));
        }
        let limit = || {
            Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "source procedural failure origin storage limit".into(),
            )
        };
        let native = self.definition_site(*artifact).and_then(|site| {
            site.diagnostic_definition(*kind, export)
                .map(|range| (site, range))
        });
        std::mem::size_of::<MacroOrigin>()
            .checked_add(invocation.source.path.capacity())
            .and_then(|bytes| bytes.checked_add(export.len()))
            .and_then(|bytes| {
                bytes.checked_add(native.map_or(0, |(site, _)| site.source.path.len()))
            })
            .filter(|bytes| *bytes <= 64 * 1024)
            .ok_or_else(limit)?;
        let definition = native
            .map(|(site, range)| {
                let mut name = String::new();
                name.try_reserve_exact(export.len()).map_err(|_| limit())?;
                name.push_str(export);
                let mut path = String::new();
                path.try_reserve_exact(site.source.path.len())
                    .map_err(|_| limit())?;
                path.push_str(&site.source.path);
                Ok::<_, Diagnostic>(DefinitionOrigin {
                    identity: definition.identity,
                    name,
                    location: SourceOrigin {
                        source: crate::syntax::token_wire::SourceIdentity {
                            unit: site.source.unit,
                            content: site.source.content,
                            path,
                        },
                        range,
                    },
                })
            })
            .transpose()?;
        Ok(MacroOrigin {
            invocation_identity: key.invocation,
            invocation: Some(invocation),
            definition,
            failure_kind: None,
            failure_span_key: None,
            failure_span: None,
            parents: Vec::new(),
        })
    }
}
