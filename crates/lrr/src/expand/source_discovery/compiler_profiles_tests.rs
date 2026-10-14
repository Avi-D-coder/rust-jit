//! Actual owning-source compiler catalogue facts; no full fixture execution.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::{
        dependency::{InputKey, InputValue},
        key::OutlineKey,
        memo::Database,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;
const ORIGINAL: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-fn-const-eval.rs"
);
const CROSS: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/cross-crate-pat-span.rs"
);
const AUX: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/auxiliary/foreign-crate-macro-pat.rs"
);
fn unit(text: &str, path: &str, key: Digest, edition: u16) -> SourceUnitInput {
    SourceUnitInput {
        path: SourcePath::new(path).unwrap(),
        crate_key: key,
        module_path: vec![],
        profile: ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
    }
}
fn world(
    units: Vec<SourceUnitInput>,
    edges: Vec<SourceDependency>,
) -> (Database, Vec<PreparedSourceView>) {
    let selections: Vec<_> = units
        .iter()
        .map(|u| SourceSelection {
            path: u.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: u.blob.content(),
                    profile: u.profile.clone(),
                },
                crate_key: u.crate_key,
                module_path: u.module_path.clone(),
                cfg: u.cfg.fingerprint(),
            },
        })
        .collect();
    let inputs =
        PreparedSourceInputs::refresh_with_dependencies(None, units, Default::default(), edges)
            .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let views = selections
        .iter()
        .map(|s| {
            let SourceSelectionOutcome::Selected(v) = inputs.select(s).unwrap() else {
                panic!()
            };
            v
        })
        .collect();
    (db, views)
}
fn providers(
    views: &[PreparedSourceView],
) -> Vec<crate::expand::source_declarative::SourceDeclarative> {
    views
        .iter()
        .map(|v| crate::expand::source_declarative::SourceDeclarative::new(v.share()).unwrap())
        .collect()
}
fn supplied(edition: u16) -> Arc<crate::frontend::compile_input_catalog::CompileInputCatalog> {
    Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(&ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        })
        .unwrap(),
    )
}
fn seal(d: &mut SourceDiscovery) -> Seal {
    loop {
        match d.poll().unwrap() {
            SourceDiscoveryStep::Sealed(s) => return s,
            SourceDiscoveryStep::NeedExpansion { request } => d
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => d
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Incomplete { diagnostic, .. } => panic!(
                "actual preparation incomplete: {} {}",
                diagnostic.code, diagnostic.message
            ),
            _ => panic!("actual producer requires unestablished authority"),
        }
    }
}
#[test]
fn unchanged_cross_crate_2021_root_2018_aux_get_their_actual_owning_facets() {
    let root = Digest([11; 32]);
    let aux = Digest([12; 32]);
    let (_, views) = world(
        vec![
            unit(CROSS, "cross.rs", root, 2021),
            unit(AUX, "aux.rs", aux, 2018),
        ],
        vec![SourceDependency {
            consumer: root,
            alias: "foreign_crate_macro_pat".into(),
            provider: aux,
        }],
    );
    let mut ps = providers(&views);
    let plan = compiler_profiles::PreparedSourceCompilerFacets::prepare(
        &ps,
        supplied(2021),
        64 * 1024 * 1024,
    )
    .unwrap();
    plan.install(&mut ps).unwrap();
    plan.revalidate(&ps).unwrap();
    for (p, view) in ps.iter().zip(&views) {
        let c = p.compiler.as_ref().unwrap();
        let profile = &view.selection().outline.parse.profile;
        assert!(c.canonical_standalone_member(profile).unwrap().is_some());
        assert_eq!(
            c.runtime_prelude_unit(false),
            crate::facade::unit(profile, "core")
        );
        assert!(
            c.attribute_catalog()
                .unwrap()
                .definitions()
                .all(|d| d.definition_edition == profile.edition)
        );
    }
    assert_ne!(
        ps[0]
            .compiler
            .as_ref()
            .unwrap()
            .owning_profile_identity()
            .unwrap(),
        ps[1]
            .compiler
            .as_ref()
            .unwrap()
            .owning_profile_identity()
            .unwrap()
    );
    // No original custom_matches! expansion or whole main is asserted.
}
#[test]
fn unchanged_array_original_mixed_profiles_publish_actual_prefix_proofs() {
    for (root_edition, aux_edition) in [(2015, 2024), (2024, 2015), (2018, 2021), (2021, 2018)] {
        let root = Digest([13; 32]);
        let aux = Digest([14; 32]);
        let (mut db, views) = world(
            vec![
                unit(ORIGINAL, "root.rs", root, root_edition),
                unit(ORIGINAL, "aux.rs", aux, aux_edition),
            ],
            vec![SourceDependency {
                consumer: root,
                alias: "aux".into(),
                provider: aux,
            }],
        );
        for target in &views {
            for ns in [Namespace::Type, Namespace::Value] {
                let mut d = SourceDiscovery::new(
                    views.iter().map(|v| v.share()).collect(),
                    target.outline().root_scope.clone(),
                    ns,
                    Digest([15; 32]),
                    1,
                )
                .unwrap()
                .with_declarative_compiler(
                    Digest([16; 32]),
                    crate::expand::source_declarative::engine_version(),
                    supplied(root_edition),
                )
                .unwrap();
                let sealed = seal(&mut d);
                d.publish(&mut db).unwrap();
                let Some((InputValue::DiscoverySeal(input), _)) =
                    db.published_input(&InputKey::DiscoverySeal(sealed.key()))
                else {
                    panic!()
                };
                assert_eq!(input.nested_inert_proofs.len(), 2);
                assert!(
                    input
                        .nested_inert_proofs
                        .iter()
                        .all(|p| p.attribute_facet().is_some())
                );
                for view in &views {
                    assert!(
                        input
                            .nested_inert_proofs
                            .iter()
                            .any(|p| p.scan().parse() == &view.selection().outline.parse
                                && p.scan().path() == &view.selection().path)
                    );
                }
                assert!(input.records.is_empty());
            }
        }
        // Both exact originals are configured source components; ARR/add are
        // never evaluated and neither original is a full fixture pass.
    }
}
#[test]
fn source_order_does_not_copy_the_first_profile_catalogue() {
    let (_, views) = world(
        vec![
            unit(ORIGINAL, "a.rs", Digest([17; 32]), 2015),
            unit(ORIGINAL, "b.rs", Digest([18; 32]), 2024),
        ],
        vec![],
    );
    for order in [[0, 1], [1, 0]] {
        let mut ps: Vec<_> = order
            .iter()
            .map(|i| {
                crate::expand::source_declarative::SourceDeclarative::new(views[*i].share())
                    .unwrap()
            })
            .collect();
        let p = compiler_profiles::PreparedSourceCompilerFacets::prepare(
            &ps,
            supplied(2024),
            64 * 1024 * 1024,
        )
        .unwrap();
        p.install(&mut ps).unwrap();
        p.revalidate(&ps).unwrap();
        for source in &ps {
            assert!(
                source
                    .compiler
                    .as_ref()
                    .unwrap()
                    .canonical_standalone_member(
                        &source.module_source().selection().outline.parse.profile
                    )
                    .unwrap()
                    .is_some()
            );
        }
    }
}
#[test]
fn all_four_actual_profiles_have_distinct_core_facet_owners() {
    let (_, views) = world(
        [2015, 2018, 2021, 2024]
            .into_iter()
            .enumerate()
            .map(|(i, e)| unit(ORIGINAL, &format!("p{i}.rs"), Digest([20 + i as u8; 32]), e))
            .collect(),
        vec![],
    );
    let mut ps = providers(&views);
    let plan = compiler_profiles::PreparedSourceCompilerFacets::prepare(
        &ps,
        supplied(2024),
        64 * 1024 * 1024,
    )
    .unwrap();
    plan.install(&mut ps).unwrap();
    let ids: BTreeSet<_> = ps
        .iter()
        .map(|p| {
            p.compiler
                .as_ref()
                .unwrap()
                .owning_profile_identity()
                .unwrap()
        })
        .collect();
    assert_eq!(ids.len(), 4);
    plan.revalidate(&ps).unwrap();
}
#[test]
fn foreign_standard_profile_is_not_accepted_when_no_selected_source_owns_it() {
    let (_, views) = world(vec![unit(ORIGINAL, "a.rs", Digest([30; 32]), 2015)], vec![]);
    let ps = providers(&views);
    let e = compiler_profiles::PreparedSourceCompilerFacets::prepare(
        &ps,
        supplied(2024),
        64 * 1024 * 1024,
    )
    .err()
    .unwrap();
    assert_eq!(e.code, "LRR-E-COMPILE-INPUT");
    assert!(ps.iter().all(|p| p.compiler.is_none()));
}
#[test]
fn explicit_custom_compile_input_data_is_not_upgraded_to_attribute_authority() {
    let (_, views) = world(
        vec![
            unit(ORIGINAL, "a.rs", Digest([31; 32]), 2015),
            unit(ORIGINAL, "b.rs", Digest([32; 32]), 2024),
        ],
        vec![],
    );
    let mut ps = providers(&views);
    let custom = Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::new(
            Digest([33; 32]),
            2024,
            [crate::expand::compile_input_identity::descriptor("concat")
                .unwrap()
                .unwrap()],
        )
        .unwrap(),
    );
    let id = custom.owning_profile_identity().unwrap();
    let p = compiler_profiles::PreparedSourceCompilerFacets::prepare(&ps, custom, 64 * 1024 * 1024)
        .unwrap();
    p.install(&mut ps).unwrap();
    for source in &ps {
        let c = source.compiler.as_ref().unwrap();
        assert_eq!(c.owning_profile_identity().unwrap(), id);
        assert!(c.attribute_catalog().is_none());
        assert!(
            original_inert::matched_attribute_facet(source.module_source(), source)
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn copied_publication_changed_profile_and_replaced_catalogue_refuse_replay() {
    let (_, views) = world(
        vec![
            unit(ORIGINAL, "a.rs", Digest([34; 32]), 2015),
            unit(ORIGINAL, "b.rs", Digest([35; 32]), 2024),
        ],
        vec![],
    );
    let mut ps = providers(&views);
    let plan = compiler_profiles::PreparedSourceCompilerFacets::prepare(
        &ps,
        supplied(2024),
        64 * 1024 * 1024,
    )
    .unwrap();
    plan.install(&mut ps).unwrap();
    ps[0].compiler = Some(supplied(2024));
    assert!(plan.revalidate(&ps).is_err());
    let (_, copied) = world(
        vec![
            unit(ORIGINAL, "a.rs", Digest([34; 32]), 2015),
            unit(ORIGINAL, "b.rs", Digest([35; 32]), 2024),
        ],
        vec![],
    );
    let mut other = providers(&copied);
    let q = compiler_profiles::PreparedSourceCompilerFacets::prepare(
        &other,
        supplied(2024),
        64 * 1024 * 1024,
    )
    .unwrap();
    q.install(&mut other).unwrap();
    assert!(plan.revalidate(&other).is_err());
}
#[test]
fn quota_and_already_installed_facets_refuse_before_mutating_any_owner() {
    let (_, views) = world(
        vec![
            unit(ORIGINAL, "a.rs", Digest([36; 32]), 2015),
            unit(ORIGINAL, "b.rs", Digest([37; 32]), 2024),
        ],
        vec![],
    );
    let mut ps = providers(&views);
    assert!(
        compiler_profiles::PreparedSourceCompilerFacets::prepare(&ps, supplied(2024), 1).is_err()
    );
    assert!(ps.iter().all(|p| p.compiler.is_none()));
    let p = compiler_profiles::PreparedSourceCompilerFacets::prepare(
        &ps,
        supplied(2024),
        64 * 1024 * 1024,
    )
    .unwrap();
    ps[1].compiler = Some(supplied(2024));
    assert!(p.install(&mut ps).is_err());
    assert!(ps[0].compiler.is_none());
}

#[test]
fn exact_owning_cfg_no_std_and_disabled_prelude_policy_survive_facet_preparation() {
    let text = "#![cfg_attr(owning_core,no_std)]#![no_implicit_prelude]const X:usize=1;fn main(){}";
    let mut root = unit(text, "cfg-root.rs", Digest([40; 32]), 2015);
    root.cfg.atoms.insert("owning_core".into());
    let aux = unit(text, "cfg-aux.rs", Digest([41; 32]), 2024);
    let (_, views) = world(vec![root, aux], vec![]);
    let mut ps = providers(&views);
    let plan = compiler_profiles::PreparedSourceCompilerFacets::prepare(
        &ps,
        supplied(2015),
        64 * 1024 * 1024,
    )
    .unwrap();
    plan.install(&mut ps).unwrap();
    plan.revalidate(&ps).unwrap();
    for (i, p) in ps.iter().enumerate() {
        let policy = p.module_source().macro_prelude_policy().unwrap();
        assert_eq!(policy.standard, i == 1);
        assert_eq!(policy.disabled, vec![Vec::<String>::new()]);
        let profile = &p.module_source().selection().outline.parse.profile;
        assert_eq!(
            p.compiler
                .as_ref()
                .unwrap()
                .runtime_prelude_unit(policy.standard),
            crate::facade::unit(profile, if i == 1 { "std" } else { "core" })
        );
        assert_eq!(
            p.module_source().selection().outline.cfg,
            views[i].selection().outline.cfg
        );
    }
    assert_ne!(
        views[0].selection().outline.cfg,
        views[1].selection().outline.cfg
    );
    // Facet ownership never overrides actual source cfg or disableable prelude policy.
}
#[test]
fn mixed_owner_prefix_publication_refuses_a_new_epoch_without_source_advance() {
    let (mut db, views) = world(
        vec![
            unit(ORIGINAL, "epoch-root.rs", Digest([42; 32]), 2015),
            unit(ORIGINAL, "epoch-aux.rs", Digest([43; 32]), 2024),
        ],
        vec![],
    );
    let mut d = SourceDiscovery::new(
        views.iter().map(|v| v.share()).collect(),
        views[0].outline().root_scope.clone(),
        Namespace::Type,
        Digest([44; 32]),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest([45; 32]),
        crate::expand::source_declarative::engine_version(),
        supplied(2015),
    )
    .unwrap();
    seal(&mut d);
    assert_eq!(d.nested_inert_proofs.len(), 2);
    let revision = db.source_revision();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(db.source_revision(), revision);
    assert!(d.publish(&mut db).is_err());
}
