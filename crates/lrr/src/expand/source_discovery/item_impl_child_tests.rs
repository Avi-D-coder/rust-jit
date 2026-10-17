//! Real stock wrapper declaration-completion authority, not guest execution.
//! Child of item_boundary_tests: reuse its pinned full-stock AST adapter and the
//! existing original_inert_tests captured source/discovery helpers.
use super::*;
use crate::expand::source_discovery::item_impl_children::ChildImplFrontier;

fn wrapped(edition: u16) -> String {
    let direct = emitted(edition);
    let prefix = direct
        .strip_suffix("emit!();")
        .expect("actual shared stock adapter has one final invocation");
    format!("{prefix}macro_rules! wrap{{()=>{{emit!();}}}}wrap!();")
}

fn pending_wrapper(edition: u16) -> SourceDiscovery {
    let text = wrapped(edition);
    let (_, view) = prepare(&text, edition, CfgSet::guest_default(), &[]);
    let mut d = discovery(&view, Namespace::Value, true);
    for _ in 0..256 {
        match d.poll().expect("real wrapper declaration discovery failed") {
            SourceDiscoveryStep::NeedExpansion { request } => d
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => d
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Incomplete { diagnostic, .. } => panic!(
                "real wrapper closure incomplete: {} {}",
                diagnostic.code, diagnostic.message
            ),
            _ => panic!("real wrapper did not expose child impl admission"),
        }
        if !d.child_impl_frontiers.is_empty() {
            assert_eq!(d.child_impl_frontiers.len(), 1);
            assert!(d.item_impl_completions.is_empty());
            assert!(d.seal.is_none());
            return d;
        }
    }
    panic!("bounded actual wrapper admission did not settle");
}

fn completed_wrapper(edition: u16) -> SourceDiscovery {
    let mut d = pending_wrapper(edition);
    seal(&mut d);
    assert!(d.child_impl_frontiers.is_empty());
    assert_eq!(d.item_impl_completions.len(), 1);
    d
}

fn actual_waiter(
    d: &SourceDiscovery,
    wrong_parent: bool,
    wrong_ancestor: bool,
) -> ChildImplFrontier {
    let (&ancestor, completion) = d.item_impl_completions.iter().next().unwrap();
    assert!(Arc::ptr_eq(
        completion.completed(),
        d.nested_completed.get(&ancestor).unwrap()
    ));
    let roots = &d.declarative.as_ref().unwrap().2[0];
    let mut matches = roots
        .completed_catalog_roots()
        .filter(|root| d.item_root_declarations.get(&root.record_identity()) == Some(&ancestor));
    let parent = matches
        .next()
        .expect("actual declaration root catalog absent");
    assert!(matches.next().is_none());
    let mut children = parent.selection_catalog_children();
    let child = children.next().expect("actual stock emit child absent");
    assert!(children.next().is_none());
    let plan = crate::frontend::generated_headers::GeneratedHeaderPlan::prepare_catalog(child)
        .expect("actual child retains its admitted common headers");
    assert!(plan.impl_frontier().is_some());
    let ancestor = if wrong_ancestor {
        let wrong = Digest::of_bytes(b"LRR wrong ancestor declaration job authority control");
        assert_ne!(wrong, ancestor);
        wrong
    } else {
        ancestor
    };
    ChildImplFrontier::prepare(
        ancestor,
        child.retained_items().clone(),
        if wrong_parent {
            // A genuine admitted output is still the wrong selected parent.
            child.retained_items().clone()
        } else {
            parent.retained_items().clone()
        },
        plan.retained_impl_projection(),
        child.owner().clone(),
        child.cfg().clone(),
        d.epoch,
    )
    .unwrap()
}

#[test]
fn actual_stock_child_requires_its_selected_parent_epoch_and_ancestor() {
    let d = completed_wrapper(2024);
    let completion = d.item_impl_completions.values().next().unwrap();
    let actual = actual_waiter(&d, false, false);
    let headers = actual.resolve(completion, d.epoch).unwrap();
    assert!(!headers.is_empty());
    assert!(
        headers
            .iter()
            .all(|header| completion.contains_header(*header))
    );

    let wrong_parent = actual_waiter(&d, true, false);
    let error = wrong_parent.resolve(completion, d.epoch).unwrap_err();
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(error.message, "child impl selected parent catalog differs");

    let error = actual
        .resolve(completion, d.epoch.checked_add(1).unwrap())
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(error.message, "child impl frontier epoch changed");

    let wrong_ancestor = actual_waiter(&d, false, true);
    let error = wrong_ancestor.resolve(completion, d.epoch).unwrap_err();
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(error.message, "child impl retained ancestor owner differs");
    assert_eq!(d.item_impl_completions.len(), 1);
}

#[test]
fn equal_source_metadata_cannot_substitute_another_actual_completed_arc_owner() {
    let original = completed_wrapper(2024);
    let (&ancestor, cached) = original.item_impl_completions.iter().next().unwrap();
    let mut current = pending_wrapper(2024);
    let (&frontier, waiter) = current.child_impl_frontiers.iter().next().unwrap();
    assert_eq!(waiter.ancestor_job(), ancestor);
    assert_eq!(
        original.sources[0].selection(),
        current.sources[0].selection()
    );
    assert!(!original.sources[0].same_publication(&current.sources[0]));
    cached
        .require_owner(ancestor, current.epoch, waiter.cfg_fingerprint())
        .unwrap();
    // This is the isolated authority mutation. All current work still executes
    // through real declaration discovery; no completion/job/body is fabricated.
    current
        .item_impl_completions
        .insert(ancestor, Arc::clone(cached));
    let error = current
        .poll()
        .err()
        .expect("equal metadata must not replace the actual completed owner");
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(error.message, "child impl actual completed owner changed");
    let actual = current.nested_completed.get(&ancestor).unwrap();
    assert!(!Arc::ptr_eq(actual, cached.completed()));
    assert_eq!(
        actual.index().output().record().descriptor().fingerprint(),
        cached
            .completed()
            .index()
            .output()
            .record()
            .descriptor()
            .fingerprint()
    );
    assert!(current.child_impl_frontiers.contains_key(&frontier));
    assert!(current.producers.contains(&frontier));
    assert!(
        !current
            .work
            .completed_job(DiscoveryKind::ScanNestedDeclarations, frontier)
    );
    assert!(current.seal.is_none());
    let repeated = current.poll().err().expect("wrong owner remains refused");
    assert_eq!(repeated.code, error.code);
    assert_eq!(repeated.message, error.message);
}
