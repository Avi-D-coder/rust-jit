//! A receipt for the actual checked-in embedded source set, not trait/header
//! semantic parity or a complete ImplCatalog certificate.
use super::PreparedLiveImplCatalogSources;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    db::key::QueryKey,
    source::{
        changes::PreparedSourceView, embedded_crates::standard_facade_crates, snapshot::CfgSet,
    },
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct CapturedTraitFacadeSources {
    sources: Vec<PreparedSourceView>,
    profile: ProfileIdentity,
    fingerprint: Digest,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-IMPL-CATALOG-FACADE", message.into())
}
impl CapturedTraitFacadeSources {
    /// Cold source preparation only. Facade views come from the selected
    /// publication, separate from the explicit live dependency closure. Compare exact
    /// source identity/bytes/cfg against the owning embedded source manifest;
    /// a reserved unit digest or name by itself is not a source receipt.
    pub(super) fn capture(
        live: &PreparedLiveImplCatalogSources,
        publication: &[PreparedSourceView],
    ) -> Result<Option<Self>, Diagnostic> {
        // The ordinary standalone capture producer installs the root's
        // facade instance; consumer editions do not create new copies of the
        // dependency's nominal types. Configured unit/header mapping remains
        // separate preparation work, never inferred from this source receipt.
        let profile = &live.selected_root().outline.parse.profile;
        let manifests = standard_facade_crates(profile);
        let units: BTreeSet<_> = manifests.iter().map(|krate| krate.unit).collect();
        let mut available = BTreeMap::new();
        for source in publication {
            let selected = source.selection();
            if !units.contains(&selected.outline.crate_key) {
                continue;
            }
            if !source.same_publication(&live.sources()[0]) {
                return Err(fault(
                    "facade source belongs to another captured publication",
                ));
            }
            if available
                .insert(
                    (selected.outline.crate_key, &selected.outline.module_path),
                    source,
                )
                .is_some()
            {
                return Err(fault("actual facade source module attachment is ambiguous"));
            }
        }
        let cfg = CfgSet::guest_default().fingerprint();
        let mut expected = BTreeSet::new();
        let mut sources = Vec::new();
        for krate in manifests {
            for file in krate.files {
                let key = (krate.unit, file.module_path);
                if !expected.insert(key.clone()) {
                    return Err(fault("embedded facade manifest repeats a module"));
                }
                let Some(source) = available.get(&(key.0, &key.1)).copied() else {
                    return Ok(None);
                };
                let selected = source.selection();
                let content = Digest::of_bytes(file.text.as_bytes());
                if selected.path.as_str() != file.path
                    || &selected.outline.parse.profile != profile
                    || selected.outline.cfg != cfg
                    || selected.outline.parse.content != content
                    || source.captured_macro_blob()?.content() != content
                {
                    return Err(fault(
                        "actual embedded facade source differs from its owning manifest",
                    ));
                }
                sources.push(source.share());
            }
        }
        for source in publication {
            let selected = source.selection();
            if units.contains(&selected.outline.crate_key)
                && !expected.contains(&(
                    selected.outline.crate_key,
                    selected.outline.module_path.clone(),
                ))
            {
                return Err(fault(
                    "actual embedded facade has an undeclared source module",
                ));
            }
        }
        sources.sort_by(|a, b| {
            a.selection()
                .outline
                .crate_key
                .cmp(&b.selection().outline.crate_key)
                .then_with(|| {
                    a.selection()
                        .outline
                        .module_path
                        .cmp(&b.selection().outline.module_path)
                })
        });
        let mut encoded = StableEncoder::new("lrr/v1/captured-trait-facade-sources");
        // This is source-set identity, not a world certificate. Unrelated
        // user sources must not change it; the retained handles still belong
        // to their actual publication. Header/name dependencies join later.
        encoded.record(profile);
        encoded.sequence(&sources, |encoded, source| {
            encoded.string(source.selection().path.as_str());
            encoded.record(&QueryKey::Outline(source.selection().outline.clone()));
        });
        Ok(Some(Self {
            sources,
            profile: profile.clone(),
            fingerprint: encoded.finish(),
        }))
    }
    pub(crate) fn sources(&self) -> &[PreparedSourceView] {
        &self.sources
    }
    pub(crate) fn owning_profile(&self) -> &ProfileIdentity {
        &self.profile
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
}
