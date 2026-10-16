//! Scalar dependencies for compile-time-only source facets. These inputs grant
//! no ordinary parse, declaration, body, native or macro execution authority.
use super::{Database, account_add, fault, input_metadata_bytes};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::dependency::{DependencyStamp, InputKey, InputRecord, InputValue},
    engine::telemetry::CacheBudgets,
    expand::source_declarative::macro_exports::SourceMacroExportFacet,
    source::changes::SourceSelection,
};
use std::{collections::BTreeMap, sync::Arc};

fn preparation(message: &str) -> Diagnostic {
    fault("LRR-E-MACRO-EXPORT-PUBLICATION", message)
}
/// Issued only from an opaque, configured source facet by the coordinator.
/// A receipt is a preparation dependency, not export/namespace completeness.
#[derive(Debug, Clone)]
pub struct MacroExportSourceReceipt {
    location: Digest,
    unit: Digest,
    facet: Digest,
    preparation: Digest,
    environment: Digest,
    source: SourceSelection,
    bytes: usize,
}
impl MacroExportSourceReceipt {
    fn capture(facet: &SourceMacroExportFacet) -> Result<Self, Diagnostic> {
        let source = facet.source().selection();
        if source.canonical()? != *source || source.outline.crate_key != facet.unit() {
            return Err(preparation("macro-export receipt source/unit differs"));
        }
        let bytes = std::mem::size_of::<Self>()
            .checked_add(source.metadata_bytes()?)
            .and_then(|n| n.checked_sub(std::mem::size_of::<SourceSelection>()))
            .filter(|n| *n <= 1024 * 1024)
            .ok_or_else(|| preparation("macro-export receipt metadata bound"))?;
        Ok(Self {
            location: source.location_identity(),
            unit: facet.unit(),
            facet: facet.identity(),
            preparation: facet.preparation(),
            environment: facet.environment(),
            source: source.clone(),
            bytes,
        })
    }
    pub fn location_identity(&self) -> Digest {
        self.location
    }
    pub fn unit(&self) -> Digest {
        self.unit
    }
    pub fn facet_identity(&self) -> Digest {
        self.facet
    }
    pub fn preparation(&self) -> Digest {
        self.preparation
    }
    pub fn environment(&self) -> Digest {
        self.environment
    }
    pub fn source(&self) -> &SourceSelection {
        &self.source
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    fn matches_facet(&self, facet: &SourceMacroExportFacet) -> bool {
        self.facet == facet.identity()
            && self.unit == facet.unit()
            && self.preparation == facet.preparation()
            && self.environment == facet.environment()
            && &self.source == facet.source().selection()
    }
}
impl StableEncode for MacroExportSourceReceipt {
    const DOMAIN: &'static str = "lrr/v1/macro-export-source-receipt";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.location);
        e.digest(self.unit);
        e.digest(self.facet);
        e.digest(self.preparation);
        e.digest(self.environment);
        e.string(self.source.path.as_str());
        e.record(&self.source.outline);
    }
}
impl Database {
    /// Explicit preparation replaces just this compile-time receipt set.
    /// Source storage stays with the preparation catalog, never the live-source
    /// registry. All validation/resource failures precede input publication.
    pub(crate) fn publish_macro_export_sources(
        &mut self,
        facets: &[Arc<SourceMacroExportFacet>],
    ) -> Result<(), Diagnostic> {
        self.require_coordinator()?;
        if self.run_active {
            return Err(preparation("macro-export publication during active run"));
        }
        if facets.len() > 256 {
            return Err(preparation("macro-export receipt count bound"));
        }
        let generation = facets.first().map(|facet| facet.preparation());
        let mut staged = BTreeMap::new();
        let mut additional = 0usize;
        for facet in facets {
            if Some(facet.preparation()) != generation {
                return Err(preparation(
                    "macro-export facets belong to different preparations",
                ));
            }
            let env_key = InputKey::UnitCompileEnv(facet.unit());
            if !matches!(self.published_input(&env_key), Some((InputValue::UnitCompileEnv(env), _))
                if env.unit() == facet.unit() && env.fingerprint() == facet.environment())
            {
                return Err(preparation(
                    "macro-export configured environment is absent or replaced",
                ));
            }
            let receipt = MacroExportSourceReceipt::capture(facet)?;
            let key = InputKey::MacroExportSource(receipt.location);
            let value = InputValue::MacroExportSource(Box::new(receipt));
            additional = account_add(additional, input_metadata_bytes(&key, &value)?)?;
            let record = InputRecord {
                fingerprint: value.fingerprint(),
                value: Arc::new(value),
            };
            if staged.insert(key, record).is_some() {
                return Err(preparation("duplicate macro-export receipt source"));
            }
        }
        let current: Vec<_> = self
            .inputs
            .iter()
            .filter(|(key, _)| matches!(key, InputKey::MacroExportSource(_)))
            .map(|(key, row)| (key, row.fingerprint))
            .collect();
        if current.len() == staged.len()
            && current.iter().all(|(key, fingerprint)| {
                staged
                    .get(*key)
                    .is_some_and(|row| row.fingerprint == *fingerprint)
            })
        {
            return Ok(());
        }
        // Conservative transient bound retains the old input owners while
        // admitting new scalar rows. No facet graph is charged as runtime data.
        let usage = self.cache_usage()?;
        if account_add(usage.memo_bytes, additional)? > CacheBudgets::default().query_bytes {
            return Err(preparation("macro-export receipt cache admission bound"));
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| fault("LRR-E-ID-EXHAUSTED", "query revision exhausted"))?;
        self.inputs
            .retain(|key, _| !matches!(key, InputKey::MacroExportSource(_)));
        self.inputs.extend(staged);
        self.revision = revision;
        Ok(())
    }

    /// Relevant current input reads only. The caller must still establish actual
    /// export/namespace/expansion authority through the common discovery route.
    pub(crate) fn macro_export_source_dependencies(
        &mut self,
        facet: &SourceMacroExportFacet,
    ) -> Result<[DependencyStamp; 2], Diagnostic> {
        let key = InputKey::MacroExportSource(facet.source().selection().location_identity());
        let source = self
            .input(&key)
            .ok_or_else(|| preparation("macro-export source is not published"))?;
        if !matches!(source.as_ref(), InputValue::MacroExportSource(receipt) if receipt.matches_facet(facet))
        {
            return Err(preparation("macro-export source preparation was replaced"));
        }
        let environment_key = InputKey::UnitCompileEnv(facet.unit());
        let environment = self
            .input(&environment_key)
            .ok_or_else(|| preparation("macro-export environment is not published"))?;
        if !matches!(environment.as_ref(), InputValue::UnitCompileEnv(env)
            if env.unit() == facet.unit() && env.fingerprint() == facet.environment())
        {
            return Err(preparation(
                "macro-export configured environment was replaced",
            ));
        }
        // Published rows already own checked stable stamps; do not rehash source
        // or configuration on this query/preparation path.
        let source_stamp = self.published_input(&key).expect("read current receipt").1;
        let environment_stamp = self
            .published_input(&environment_key)
            .expect("read current environment")
            .1;
        Ok([
            DependencyStamp::Input {
                key,
                fingerprint: source_stamp,
            },
            DependencyStamp::Input {
                key: environment_key,
                fingerprint: environment_stamp,
            },
        ])
    }
}
