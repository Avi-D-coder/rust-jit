//! Retained replay controls share the existing unchanged official root fixture
//! and actual compiler/source discovery helpers. No runtime fixture claim.
use super::*;
use crate::source::compiler_replay::CompilerReplaySource;

#[test]
fn unchanged_root_prefix_replays_with_independent_current_scan_and_captured_set() {
    for edition in [2015, 2018, 2021, 2024] {
        let (_, view) = prepare(ORIGINAL, edition, CfgSet::guest_default(), &[]);
        let mut d = discovery(&view, Namespace::Value, true);
        seal(&mut d);
        let providers = &d.declarative.as_ref().unwrap().2;
        let names = SourceDiscovery::install_macro_routes_with_modules(
            &mut d.graph,
            providers,
            &d.module_routes,
            &d.nested_sources,
            &d.nested_inert_proofs,
        )
        .unwrap();
        let actual_scan = view.declaration_inventory(Default::default()).unwrap();
        let current = CompilerReplaySource::prepare(&view, &actual_scan).unwrap();
        let captured = original_inert::capture(&view, &actual_scan, 32 * 1024 * 1024).unwrap();
        assert_eq!(captured, d.original_prefix_jobs);
        assert_eq!(captured.len(), 1);
        for proof in d.nested_inert_proofs.values() {
            assert!(!Arc::ptr_eq(proof.scan(), &actual_scan));
            original_inert::revalidate(
                &current,
                &providers[0],
                proof,
                &names,
                &d.original_prefix_jobs,
            )
            .unwrap();
            // A current scan and compiler facet cannot invent prefix membership.
            assert!(
                original_inert::revalidate(
                    &current,
                    &providers[0],
                    proof,
                    &names,
                    &Default::default()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn retained_prefix_refuses_current_cfg_scope_and_changed_attribute_binding() {
    let (_, view) = prepare(ORIGINAL, 2024, CfgSet::guest_default(), &[]);
    let mut d = discovery(&view, Namespace::Type, true);
    seal(&mut d);
    let providers = &d.declarative.as_ref().unwrap().2;
    let names = SourceDiscovery::install_macro_routes_with_modules(
        &mut d.graph,
        providers,
        &d.module_routes,
        &d.nested_sources,
        &d.nested_inert_proofs,
    )
    .unwrap();
    let proof = d.nested_inert_proofs.values().next().unwrap();
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("changed_retained_prefix_cfg".into());
    let (_, changed) = prepare(ORIGINAL, 2024, cfg, &[]);
    let changed_scan = changed.declaration_inventory(Default::default()).unwrap();
    let changed = CompilerReplaySource::prepare(&changed, &changed_scan).unwrap();
    assert!(
        original_inert::revalidate(
            &changed,
            &providers[0],
            proof,
            &names,
            &d.original_prefix_jobs
        )
        .is_err()
    );
    let (_, scoped) = prepare(ORIGINAL, 2024, CfgSet::guest_default(), &["owned"]);
    let scoped_scan = scoped.declaration_inventory(Default::default()).unwrap();
    let scoped = CompilerReplaySource::prepare(&scoped, &scoped_scan).unwrap();
    assert!(
        original_inert::revalidate(
            &scoped,
            &providers[0],
            proof,
            &names,
            &d.original_prefix_jobs
        )
        .is_err()
    );
    let actual_scan = view.declaration_inventory(Default::default()).unwrap();
    let current = CompilerReplaySource::prepare(&view, &actual_scan).unwrap();
    let compiler = providers[0].compiler.as_ref().unwrap();
    let attributes = compiler.attribute_catalog().unwrap();
    let test = attributes
        .definitions()
        .find(|definition| attributes.select(definition).unwrap().unwrap().name() == "test")
        .unwrap()
        .clone();
    let mut changed_names = crate::frontend::macro_names::MacroNamespaces::default();
    changed_names
        .define(
            crate::frontend::macro_names::MacroPath::new(
                view.outline().root_scope.crate_key,
                vec![crate::expand::scopes::MacroName::parse("allow", 2024).unwrap()],
            )
            .unwrap(),
            test,
        )
        .unwrap();
    let error = original_inert::revalidate(
        &current,
        &providers[0],
        proof,
        &changed_names,
        &d.original_prefix_jobs,
    )
    .unwrap_err();
    assert_eq!(error.code, "LRR-E-MACRO-AMBIGUOUS");
}
