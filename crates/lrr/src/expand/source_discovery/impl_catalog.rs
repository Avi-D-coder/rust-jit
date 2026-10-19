//! Captured live-unit and actual embedded-facade source membership. Neither
//! receipt establishes configured-world/header/producer catalog completion.
use super::SourceDiscovery;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    db::key::QueryKey,
    source::{
        changes::{PreparedSourceView, SourceSelection},
        dependencies::SourceDependencies,
    },
};
use std::{collections::BTreeSet, sync::Arc};
#[path = "trait_facade_sources.rs"]
mod trait_facade_sources;
pub(crate) use trait_facade_sources::CapturedTraitFacadeSources;

#[path = "configured_catalog.rs"]
mod configured_catalog;
pub(crate) use configured_catalog::ImplCatalogCompletion;
pub(super) use configured_catalog::ImplCatalogPlan;

/// Missing contributions are preparation demands, never complete empty catalogs.
/// No Ready/Sealed variant exists before configured-world, exact header parity
/// and the common nested producer closure are actually complete.
pub(crate) enum LiveImplCatalogPreparation {
    NeedLiveUnit {
        expected: SourceSelection,
    },
    NeedConfiguredWorld {
        captured: PreparedLiveImplCatalogSources,
        trait_facade: MandatoryTraitFacade,
    },
}
pub(crate) enum MandatoryTraitFacade {
    Missing,
    /// Real source files are retained; their header/impl compatibility and
    /// language-item translation must still be prepared before solving.
    NeedHeaders {
        captured: CapturedTraitFacadeSources,
    },
}

/// Retains actual source-publication handles, exact source selectors and the
/// existing dependency graph. It grants no name, impl, source-currentness or
/// executable-body authority and exposes no constructor from arbitrary rows.
pub(crate) struct PreparedLiveImplCatalogSources {
    selected: SourceSelection,
    unit_graph: Digest,
    epoch: u64,
    dependencies: Arc<SourceDependencies>,
    units: Vec<Digest>,
    sources: Vec<PreparedSourceView>,
    profiles: Vec<ProfileIdentity>,
    fingerprint: Digest,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-IMPL-CATALOG-SOURCES", message.into())
}
impl SourceDiscovery {
    /// Derive membership from the selected source's immutable publication,
    /// never from a caller-selected discovery vector or a scope certificate.
    pub(crate) fn prepare_live_impl_catalog_sources(
        &self,
    ) -> Result<LiveImplCatalogPreparation, Diagnostic> {
        self.key.validate()?;
        let mut owners = self
            .sources
            .iter()
            .filter(|source| source.selection().outline.crate_key == self.scope.crate_key);
        let owner = owners
            .next()
            .ok_or_else(|| fault("selected live unit has no captured publication"))?;
        if owners.any(|source| !source.same_publication(owner)) {
            return Err(fault("selected live unit has conflicting publications"));
        }
        PreparedLiveImplCatalogSources::capture(owner, self.key.unit_graph, self.key.epoch)
    }
}
impl PreparedLiveImplCatalogSources {
    /// Actual captured membership without first planning a throwaway scope
    /// discovery. Existing scope metadata and the global builder share this
    /// same source/graph/facade preparation route.
    pub(crate) fn capture(
        owner: &PreparedSourceView,
        unit_graph: Digest,
        epoch: u64,
    ) -> Result<LiveImplCatalogPreparation, Diagnostic> {
        if epoch == 0 {
            return Err(fault("source catalog expansion epoch must be nonzero"));
        }
        let dependencies = owner.source_dependencies()?;
        let selected = dependencies
            .root(owner.selection().outline.crate_key)
            .ok_or_else(|| fault("selected unit is absent from its actual source graph"))?;
        let mut sources = owner.captured_declaration_sources()?;
        if sources.iter().any(|s| !s.same_publication(owner)) {
            return Err(fault(
                "captured declaration publication count/owner differs",
            ));
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
                .then_with(|| a.selection().path.cmp(&b.selection().path))
        });
        if sources.windows(2).any(|pair| {
            let a = pair[0].selection();
            let b = pair[1].selection();
            a.outline.crate_key == b.outline.crate_key
                && a.outline.module_path == b.outline.module_path
                && (a.path == b.path || a.outline.module_path.is_empty())
        }) {
            return Err(fault("captured source/root ownership is duplicated"));
        }
        // The retained dependency graph already owns indexed adjacency. Visit
        // each reached unit once; no alias-map copies or resource counters.
        let mut reached = BTreeSet::from([owner.selection().outline.crate_key]);
        let mut pending = vec![owner.selection().outline.crate_key];
        while let Some(unit) = pending.pop() {
            let root = dependencies
                .root(unit)
                .ok_or_else(|| fault("source dependency endpoint has no configured root"))?;
            match sources.binary_search_by(|source| {
                source
                    .selection()
                    .outline
                    .crate_key
                    .cmp(&unit)
                    .then_with(|| source.selection().outline.module_path.as_slice().cmp(&[]))
            }) {
                Ok(ordinal) if sources[ordinal].selection() == root => {}
                Ok(_) => return Err(fault("captured live root differs from configured graph")),
                Err(_) => {
                    return Ok(LiveImplCatalogPreparation::NeedLiveUnit {
                        expected: root.clone(),
                    });
                }
            }
            for target in dependencies
                .aliases_for(unit, root.outline.parse.profile.edition)?
                .values()
            {
                if reached.insert(*target) {
                    pending.push(*target);
                }
            }
        }
        let units: Vec<_> = reached.into_iter().collect();
        // The mandatory synthetic facade is a separate catalog input: its
        // captured files need not be reachable through explicit extern edges.
        let publication = sources;
        let sources: Vec<_> = publication
            .iter()
            .filter(|source| {
                units
                    .binary_search(&source.selection().outline.crate_key)
                    .is_ok()
            })
            .map(PreparedSourceView::share)
            .collect();
        let mut unique_profiles = BTreeSet::new();
        for source in &sources {
            let profile = &source.selection().outline.parse.profile;
            profile.validate_source()?;
            unique_profiles.insert(profile);
        }
        let profiles: Vec<_> = unique_profiles.into_iter().cloned().collect();
        let fingerprint = encode_membership(
            selected,
            unit_graph,
            epoch,
            &dependencies,
            &units,
            &sources,
            &profiles,
        );
        let live = PreparedLiveImplCatalogSources {
            selected: selected.clone(),
            unit_graph: unit_graph,
            epoch: epoch,
            dependencies: Arc::clone(&dependencies),
            units,
            sources,
            profiles,
            fingerprint,
        };
        let trait_facade = match CapturedTraitFacadeSources::capture(&live, &publication)? {
            Some(captured) => MandatoryTraitFacade::NeedHeaders { captured },
            None => MandatoryTraitFacade::Missing,
        };
        Ok(LiveImplCatalogPreparation::NeedConfiguredWorld {
            captured: live,
            trait_facade,
        })
    }
    pub(crate) fn selected_root(&self) -> &SourceSelection {
        &self.selected
    }
    pub(crate) fn graph_epoch(&self) -> (Digest, u64) {
        (self.unit_graph, self.epoch)
    }
    pub(crate) fn source_dependencies(&self) -> &Arc<SourceDependencies> {
        &self.dependencies
    }
    pub(crate) fn sources(&self) -> &[PreparedSourceView] {
        &self.sources
    }
    pub(crate) fn live_units(&self) -> &[Digest] {
        &self.units
    }
    pub(crate) fn live_source_profiles(&self) -> &[ProfileIdentity] {
        &self.profiles
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
}
fn encode_membership(
    selected: &SourceSelection,
    unit_graph: Digest,
    epoch: u64,
    dependencies: &SourceDependencies,
    units: &[Digest],
    sources: &[PreparedSourceView],
    profiles: &[ProfileIdentity],
) -> Digest {
    let mut e = StableEncoder::new("lrr/v1/captured-live-impl-source-membership");
    e.string(selected.path.as_str());
    e.record(&QueryKey::Outline(selected.outline.clone()));
    e.digest(unit_graph);
    e.u64(epoch);
    e.digest(dependencies.fingerprint());
    e.sequence(units, |e, unit| e.digest(*unit));
    e.sequence(sources, |e, source| {
        e.string(source.selection().path.as_str());
        e.record(&QueryKey::Outline(source.selection().outline.clone()));
    });
    e.sequence(profiles, |e, profile| e.record(profile));
    e.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::key::OutlineKey,
        source::{
            changes::{PreparedSourceInputs, SourceSelectionOutcome, SourceUnitInput},
            dependencies::SourceDependency,
            manifest::Namespace,
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
    };
    const FIELDS: &str = include_str!(
        "../../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-to-subfield.rs"
    );
    const DESTRUCTORS: &str = include_str!(
        "../../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/destructor-run-for-let-ignore-6892.rs"
    );
    fn live(discovery: &SourceDiscovery) -> PreparedLiveImplCatalogSources {
        match discovery.prepare_live_impl_catalog_sources().unwrap() {
            LiveImplCatalogPreparation::NeedConfiguredWorld {
                captured,
                trait_facade: MandatoryTraitFacade::Missing,
            } => captured,
            LiveImplCatalogPreparation::NeedConfiguredWorld {
                trait_facade: MandatoryTraitFacade::NeedHeaders { .. },
                ..
            } => panic!("control unexpectedly contains actual standard facade sources"),
            LiveImplCatalogPreparation::NeedLiveUnit { expected } => {
                panic!("actual captured root was omitted: {expected:?}")
            }
        }
    }
    #[test]
    fn unchanged_cleanup_sources_never_prove_an_empty_trait_facade_all_editions() {
        for edition in [2015, 2018, 2021, 2024] {
            for text in [FIELDS, DESTRUCTORS] {
                let (provider, _, _) =
                    crate::expand::source_declarative::assembly::tests::world(text, edition);
                let source = provider.module_source();
                let discovery = SourceDiscovery::new(
                    vec![source.share()],
                    source.outline().root_scope.clone(),
                    Namespace::Value,
                    Digest::of_bytes(b"actual-cleanup-impl-membership-control"),
                    1,
                )
                .unwrap();
                let prepared = live(&discovery);
                assert_eq!(prepared.sources().len(), 1);
                assert_eq!(
                    prepared.sources()[0]
                        .captured_macro_blob()
                        .unwrap()
                        .text()
                        .unwrap(),
                    text
                );
                assert_eq!(prepared.live_source_profiles().len(), 1);
                assert_eq!(prepared.live_source_profiles()[0].edition, edition);
                assert_eq!(prepared.selected_root(), source.selection());
                assert_eq!(prepared.graph_epoch().1, 1);
            }
        }
    }

    fn input(
        unit: Digest,
        path: &str,
        modules: &[&str],
        edition: u16,
        text: &str,
    ) -> (SourceUnitInput, SourceSelection) {
        let mut profile = ProfileIdentity::current();
        profile.edition = edition;
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        let path = SourcePath::new(path).unwrap();
        let module_path: Vec<_> = modules.iter().map(|part| (*part).to_owned()).collect();
        let selection = SourceSelection {
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
        };
        (
            SourceUnitInput {
                path,
                crate_key: unit,
                module_path,
                profile,
                cfg,
                blob,
            },
            selection,
        )
    }
    #[test]
    fn actual_publication_includes_reachable_dependency_modules_not_unrelated_units() {
        let root = Digest::of_bytes(b"live-membership-root");
        let dependency = Digest::of_bytes(b"live-membership-dependency");
        let unrelated = Digest::of_bytes(b"live-membership-unrelated");
        let (root_input, root_selection) = input(root, "root.rs", &[], 2024, FIELDS);
        let (dependency_input, dependency_selection) = input(
            dependency,
            "dep.rs",
            &[],
            2018,
            "mod details; fn cold() {} ",
        );
        let (module_input, module_selection) = input(
            dependency,
            "details.rs",
            &["details"],
            2018,
            "trait Marker {} trait Bound {} impl<T> Marker for T where T: Bound {}",
        );
        let (unrelated_input, _) = input(unrelated, "unrelated.rs", &[], 2015, "fn unrelated() {}");
        let inputs = PreparedSourceInputs::refresh_with_dependencies(
            None,
            vec![root_input, dependency_input, module_input, unrelated_input],
            Default::default(),
            vec![SourceDependency {
                consumer: root,
                alias: "dep".into(),
                provider: dependency,
            }],
        )
        .unwrap();
        let select = |selection: &SourceSelection| {
            let SourceSelectionOutcome::Selected(source) = inputs.select(selection).unwrap() else {
                panic!("exact captured source absent")
            };
            source
        };
        let root_source = select(&root_selection);
        let dependency_source = select(&dependency_selection);
        // Deliberately omit the captured module from this scope demand. Actual
        // publication membership, rather than this vector, must still find it.
        let discovery = SourceDiscovery::new(
            vec![root_source.share(), dependency_source],
            root_source.outline().root_scope.clone(),
            Namespace::Value,
            Digest::of_bytes(b"live-membership-world-control"),
            1,
        )
        .unwrap();
        let prepared = live(&discovery);
        assert_eq!(prepared.live_units().len(), 2);
        assert!(prepared.live_units().contains(&root));
        assert!(prepared.live_units().contains(&dependency));
        assert!(!prepared.live_units().contains(&unrelated));
        assert_eq!(prepared.sources().len(), 3);
        assert!(
            prepared
                .sources()
                .iter()
                .any(|s| s.selection() == &module_selection)
        );
        assert_eq!(prepared.live_source_profiles().len(), 2);
        assert_eq!(prepared.fingerprint(), live(&discovery).fingerprint());
        assert_eq!(prepared.source_dependencies().root_count(), 3);
    }
    #[test]
    fn captured_membership_does_not_discharge_cold_or_disabled_producer_frontiers() {
        let text = "#[cfg(any())] fn disabled() { absent_disabled!(); } fn cold() { possible_impl!(); } fn main() {}";
        let (provider, _, _) =
            crate::expand::source_declarative::assembly::tests::world(text, 2024);
        let source = provider.module_source();
        let discovery = SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Value,
            Digest::of_bytes(b"cold-impl-producer-captured-membership"),
            1,
        )
        .unwrap();
        let prepared = live(&discovery);
        assert_eq!(prepared.sources().len(), 1);
        // The ordinary scanner owns cfg and grammar. Membership creates no
        // proof that the surviving unknown producer cannot emit an impl.
        assert_eq!(discovery.nested_jobs.len(), 1);
        assert!(discovery.nested_completed.is_empty());
        assert!(discovery.seal.is_none());
        assert!(!discovery.started);
    }
}
