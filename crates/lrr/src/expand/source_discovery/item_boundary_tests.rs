//! LRR discovery authority controls around the unchanged official shadowing
//! root. No execution of main, Foo::foo, or native/compiler reference is claimed.
//! Registered as a child of original_inert_tests to reuse actual source capture.
use super::*;
#[path = "item_impl_child_tests.rs"]
mod item_impl_child_tests;
use crate::expand::{
    body_macro_boundary::CompletedEmptyBoundary,
    discovery::{DiscoveryJob, DiscoveryKind, DiscoveryStep},
    source_discovery::item_boundaries::PendingItemBoundary,
};
use ra_ap_syntax::{AstNode, ast};

const STOCK: &str = include_str!(
    "../../../../../fixtures/upstream/rustc-checkout/tests/ui/resolve/type-param-local-var-shadowing.rs"
);

// Same pinned AST adapter as run_stock_local_call_tests::emitted_shadowing:
// actual inner attrs/docs stay at crate scope, all other bytes enter emit.
fn emitted(edition: u16) -> String {
    let edition = match edition {
        2015 => ra_ap_syntax::Edition::Edition2015,
        2018 => ra_ap_syntax::Edition::Edition2018,
        2021 => ra_ap_syntax::Edition::Edition2021,
        2024 => ra_ap_syntax::Edition::Edition2024,
        _ => panic!("unsupported boundary-control edition"),
    };
    let parsed = ra_ap_syntax::SourceFile::parse(STOCK, edition);
    assert!(parsed.errors().is_empty());
    let mut attributes = String::new();
    let mut items = String::new();
    let mut previous = 0;
    for attribute in parsed
        .tree()
        .syntax()
        .children()
        .filter_map(ast::AnyAttr::cast)
        .filter(|attribute| attribute.kind() == ast::AttrKind::Inner)
    {
        let range = crate::syntax::ast_adapter::range(attribute.syntax().text_range());
        let start = range.start as usize;
        let end = range.end as usize;
        items.push_str(&STOCK[previous..start]);
        attributes.push_str(&STOCK[start..end]);
        attributes.push('\n');
        previous = end;
    }
    items.push_str(&STOCK[previous..]);
    format!("{attributes}macro_rules! emit{{()=>{{{items}}}}}emit!();")
}

fn pending(edition: u16) -> (SourceDiscovery, String, Digest) {
    let text = emitted(edition);
    let (_, view) = prepare(&text, edition, CfgSet::guest_default(), &[]);
    let mut d = discovery(&view, Namespace::Value, true);
    for _ in 0..256 {
        match d
            .poll()
            .expect("actual output discovery must reach its pending boundary")
        {
            SourceDiscoveryStep::NeedExpansion { request } => {
                d.admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
                if !d.pending_item_boundaries.is_empty() {
                    assert_eq!(d.pending_item_boundaries.len(), 1);
                    let identity = *d.pending_item_boundaries.keys().next().unwrap();
                    return (d, text, identity);
                }
            }
            SourceDiscoveryStep::Incomplete { diagnostic, .. } => panic!(
                "actual output failed before pending receipt: {} {}",
                diagnostic.code, diagnostic.message
            ),
            _ => panic!("actual output did not expose its no-catalog pending receipt"),
        }
    }
    panic!("bounded actual output admission did not settle");
}

#[test]
fn actual_item_boundary_waits_for_declaration_jobs_then_finalizes_once_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut d, _, identity) = pending(edition);
        assert_eq!(
            d.item_impl_jobs.len(),
            1,
            "actual generic impl needs closure"
        );
        assert!(!d.nested_jobs.is_empty());
        assert!(d.seal.is_none());
        assert!(!d.completed_empty_boundaries.contains_key(&identity));
        assert!(!d.complete_ready_item_boundaries().unwrap());
        assert!(d.pending_item_boundaries.contains_key(&identity));
        let actual_record = Arc::clone(d.records.get(&identity).unwrap());
        assert!(
            d.declarative.as_ref().unwrap().2[0]
                .macro_index()
                .producer_kind(actual_record.key().invocation)
                .is_some()
        );
        for frontier in d.item_impl_jobs.keys() {
            assert!(
                !d.work
                    .completed_job(DiscoveryKind::ScanNestedDeclarations, *frontier)
            );
        }
        let expansions = d.accounted_expansion_count().unwrap();
        let sealed = seal(&mut d);
        assert_eq!(d.seal.as_ref().unwrap().key(), sealed.key());
        assert!(d.pending_item_boundaries.is_empty());
        let completed = d.completed_empty_boundaries.get(&identity).unwrap();
        assert!(completed.record.same_record(&actual_record));
        assert_eq!(completed.source_ordinal, 0);
        assert_eq!(completed.call.item_input, actual_record.key().invocation);
        assert!(
            d.declarative.as_ref().unwrap().2[0]
                .macro_index()
                .producer_kind(actual_record.key().invocation)
                .is_none()
        );
        assert_eq!(d.accounted_expansion_count().unwrap(), expansions);
        assert!(!d.complete_ready_item_boundaries().unwrap());
        assert_eq!(d.completed_empty_boundaries.len(), 1);
    }
}

#[test]
fn actual_pending_boundary_refuses_epoch_and_source_publication_changes_before_completion() {
    for change in [0, 1, 2, 3] {
        let (mut d, text, identity) = pending(2024);
        match change {
            0 => d.epoch = d.epoch.checked_add(1).unwrap(),
            1 => {
                // Same selection/content, independently captured publication.
                let (_, source) = prepare(&text, 2024, CfgSet::guest_default(), &[]);
                assert_eq!(source.selection(), d.sources[0].selection());
                assert!(!source.same_publication(&d.sources[0]));
                d.sources[0] = source;
            }
            2 => {
                let changed = format!("{text}\n// LRR source-authority control\n");
                let (_, source) = prepare(&changed, 2024, CfgSet::guest_default(), &[]);
                assert_ne!(source.selection(), d.sources[0].selection());
                d.sources[0] = source;
            }
            3 => {
                let mut cfg = CfgSet::guest_default();
                cfg.atoms.insert("lrr_boundary_changed_cfg".into());
                let (_, source) = prepare(&text, 2024, cfg, &[]);
                d.sources[0] = source;
            }
            _ => unreachable!(),
        }
        let error = d.complete_ready_item_boundaries().unwrap_err();
        assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
        assert_eq!(
            error.message,
            "pending item boundary source/cfg/epoch changed"
        );
        assert!(d.completed_empty_boundaries.get(&identity).is_none());
        assert!(d.pending_item_boundaries.contains_key(&identity));
        assert!(d.seal.is_none());
        let repeated = d
            .poll()
            .err()
            .expect("refused coordinator must remain poisoned");
        assert_eq!(repeated.code, error.code);
        assert_eq!(repeated.message, error.message);
    }
}

#[test]
fn wrong_typed_prerequisite_cannot_hide_pending_boundary_behind_a_worklist_seal() {
    let (mut d, _, identity) = pending(2024);
    let actual = match d.work.poll().unwrap() {
        DiscoveryStep::Run(ticket) => ticket.job().clone(),
        _ => panic!("actual admitted output must have unfinished declaration work"),
    };
    let wrong = DiscoveryJob {
        kind: if actual.kind == DiscoveryKind::ResolveMacro {
            DiscoveryKind::ResolveImport
        } else {
            DiscoveryKind::ResolveMacro
        },
        ..actual.clone()
    };
    let wrong_kind = wrong.kind;
    let wrong_identity = wrong.identity;
    assert!(!d.work.completed_job(wrong_kind, wrong_identity));
    let record = Arc::clone(d.records.get(&identity).unwrap());
    let call = d.sources[0]
        .outline()
        .item_invocations
        .iter()
        .find(|call| call.item_input == record.key().invocation)
        .unwrap()
        .clone();
    let corrupted = PendingItemBoundary::prepare(
        CompletedEmptyBoundary {
            record,
            source_ordinal: 0,
            call,
        },
        &d.sources[0],
        d.epoch,
        &[wrong],
    )
    .unwrap();
    d.pending_item_boundaries.insert(identity, corrupted);
    assert!(!d.complete_ready_item_boundaries().unwrap());
    // Let every genuine job execute through ordinary declaration preparation;
    // never mark a job complete just to obtain this negative-control state.
    let error = d
        .poll()
        .err()
        .expect("unowned prerequisite must forbid sealing");
    assert_eq!(error.code, "LRR-E-CATALOG-INCOMPLETE");
    assert_eq!(error.message, "scope seal retains pending item boundaries");
    assert!(d.pending_item_boundaries.contains_key(&identity));
    assert!(d.completed_empty_boundaries.get(&identity).is_none());
    assert!(d.seal.is_none());
    assert!(d.work.completed_job(actual.kind, actual.identity));
    assert!(!d.work.completed_job(wrong_kind, wrong_identity));
    let repeated = d
        .poll()
        .err()
        .expect("malformed pending receipt must stay poisoned");
    assert_eq!(repeated.code, error.code);
    assert_eq!(repeated.message, error.message);
}
