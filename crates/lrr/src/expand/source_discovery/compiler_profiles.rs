//! Immutable compiler-facet preparation for actually selected owning profiles.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
        profile::ProfileIdentity,
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    source::changes::{PreparedSourceView, SourceSelection},
};
use std::{collections::BTreeMap, sync::Arc};
struct Entry {
    source: PreparedSourceView,
    selection: SourceSelection,
    catalog: Arc<CompileInputCatalog>,
    identity: Digest,
}
pub(super) struct PreparedSourceCompilerFacets {
    entries: Vec<Entry>,
    bytes: usize,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-COMPILE-INPUT", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "owning-profile compiler facet budget".into(),
    )
}
impl PreparedSourceCompilerFacets {
    pub(super) fn prepare(
        providers: &[super::super::source_declarative::SourceDeclarative],
        supplied: Arc<CompileInputCatalog>,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        if providers.is_empty()
            || providers.len() > 100_000
            || byte_limit > 64 * 1024 * 1024
            || byte_limit < std::mem::size_of::<Self>() + 256
        {
            return Err(limit());
        }
        let canonical = providers.iter().try_fold(false, |matched, p| {
            Ok::<_, Diagnostic>(
                matched
                    || supplied
                        .canonical_standalone_member(
                            &p.module_source().selection().outline.parse.profile,
                        )?
                        .is_some(),
            )
        })?;
        if !canonical && supplied.has_standard_facets() {
            return Err(fault(
                "supplied compiler facet is not the actual standalone catalogue of any selected owning profile",
            ));
        }
        let mut profiles: BTreeMap<ProfileIdentity, Arc<CompileInputCatalog>> = BTreeMap::new();
        let mut entries = Vec::new();
        let mut bytes = providers
            .len()
            .checked_mul(std::mem::size_of::<Entry>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() + 256))
            .filter(|n| *n <= byte_limit)
            .ok_or_else(limit)?;
        entries
            .try_reserve_exact(providers.len())
            .map_err(|_| limit())?;
        for provider in providers {
            let source = provider.module_source();
            let profile = &source.selection().outline.parse.profile;
            let catalog = if canonical {
                source.require_declaration_syntax()?;
                if let Some(catalog) = profiles.get(profile) {
                    catalog.clone()
                } else {
                    if profiles.len() >= 256 {
                        return Err(limit());
                    }
                    let catalog = if supplied.canonical_standalone_member(profile)?.is_some() {
                        supplied.clone()
                    } else {
                        source.require_original_hygiene()?;
                        Arc::new(CompileInputCatalog::standalone(profile)?)
                    };
                    if catalog.canonical_standalone_member(profile)?.is_none() {
                        return Err(fault(
                            "prepared compiler facet is not an exact owning-profile standalone member",
                        ));
                    }
                    bytes = bytes
                        .checked_add(catalog.owning_profile_metadata_bytes()?)
                        .and_then(|n| n.checked_add(std::mem::size_of::<ProfileIdentity>()))
                        .and_then(|n| n.checked_add(profile.semantics.capacity()))
                        .and_then(|n| n.checked_add(128))
                        .filter(|n| *n <= byte_limit)
                        .ok_or_else(limit)?;
                    profiles.insert(profile.clone(), catalog.clone());
                    catalog
                }
            } else {
                // Preserve explicit custom compile-input data without upgrading
                // it into standard Attribute/prelude authority.
                if entries.is_empty() {
                    bytes = bytes
                        .checked_add(supplied.owning_profile_metadata_bytes()?)
                        .filter(|n| *n <= byte_limit)
                        .ok_or_else(limit)?;
                }
                supplied.clone()
            };
            bytes = bytes
                .checked_add(128)
                .and_then(|n| n.checked_add(source.selection().metadata_bytes().ok()?))
                .filter(|n| *n <= byte_limit)
                .ok_or_else(limit)?;
            entries.push(Entry {
                source: source.share(),
                selection: source.selection().clone(),
                identity: catalog.owning_profile_identity()?,
                catalog,
            });
        }
        Ok(Self { entries, bytes })
    }
    pub(super) fn install(
        &self,
        providers: &mut [super::super::source_declarative::SourceDeclarative],
    ) -> Result<(), Diagnostic> {
        if providers.len() != self.entries.len() {
            return Err(fault("compiler facet source membership differs"));
        }
        for (provider, entry) in providers.iter().zip(&self.entries) {
            if provider.compiler.is_some()
                || !provider.module_source().same_publication(&entry.source)
                || provider.module_source().selection() != &entry.selection
            {
                return Err(fault(
                    "compiler facet source owner/publication changed or already installed",
                ));
            }
        }
        for (provider, entry) in providers.iter_mut().zip(&self.entries) {
            provider.compiler = Some(entry.catalog.clone());
        }
        Ok(())
    }
    pub(super) fn revalidate(
        &self,
        providers: &[super::super::source_declarative::SourceDeclarative],
    ) -> Result<(), Diagnostic> {
        if providers.len() != self.entries.len() {
            return Err(fault("prepared compiler facet source membership changed"));
        }
        for (provider, entry) in providers.iter().zip(&self.entries) {
            if !provider.module_source().same_publication(&entry.source)
                || provider.module_source().selection() != &entry.selection
            {
                return Err(fault(
                    "prepared compiler facet source/profile/current catalogue differs",
                ));
            }
            let catalog = provider
                .compiler
                .as_ref()
                .ok_or_else(|| fault("prepared compiler facet absent"))?;
            if catalog.owning_profile_identity()? != entry.identity {
                return Err(fault("prepared compiler facet current identity differs"));
            }
        }
        Ok(())
    }
    pub(super) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(super) fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v1/owning-source-compiler-facets");
        e.u64(self.entries.len() as u64);
        for entry in &self.entries {
            e.digest(entry.selection.location_identity());
            e.record(&entry.selection.outline.parse);
            e.digest(entry.selection.outline.cfg);
            e.digest(entry.identity);
        }
        e.finish()
    }
}
