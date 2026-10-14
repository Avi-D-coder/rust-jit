use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn source(text: &str) -> PreparedSourceView {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([241; 32]);
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    view
}

fn discover(v: &PreparedSourceView) -> SourceDiscovery {
    SourceDiscovery::new(
        vec![v.share()],
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        Digest([245; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap()
}
#[test]
fn discovery_domains_share_one_budget_without_recharging_the_same_record() {
    use lrr::{expand::budget::SharedExpansionBudget, source::manifest::Namespace};
    let view = source(
        "macro_rules! one{()=>{fn made(){}}}macro_rules! two{()=>{fn other(){}}}one!();two!();",
    );
    let request_id = Digest([246; 32]);
    let budget = SharedExpansionBudget::new(request_id, Digest([245; 32]), 1, 8).unwrap();
    let mut first = discover(&view)
        .with_expansion_budget(request_id, budget.clone())
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = first.poll().unwrap() else {
        panic!("first domain demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    first.admit_expansion(record.clone()).unwrap();
    let mut second = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Type,
        Digest([245; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap()
    .with_expansion_budget(request_id, budget.clone())
    .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = second.poll().unwrap() else {
        panic!("second domain demand absent")
    };
    assert_eq!(request.key(), record.key());
    second.admit_expansion(record.clone()).unwrap();
    assert_eq!(budget.totals().unwrap().tokens, record.token_count());
    assert_eq!(budget.totals().unwrap().outputs, 1);
    let SourceDiscoveryStep::NeedExpansion { request } = second.poll().unwrap() else {
        panic!("distinct later output absent")
    };
    let later = std::sync::Arc::new(request.expand().unwrap());
    assert_eq!(
        second.admit_expansion(later.clone()).unwrap_err().code,
        "LRR-E-DISCOVERY-LIMIT"
    );
    assert!(second.expansion_record(later.key()).is_none());
    assert_eq!(budget.totals().unwrap().outputs, 1);
    assert!(first.sealed_expansions().is_err());
    assert!(second.sealed_expansions().is_err());
}
#[test]
fn discovery_shared_budget_rejects_foreign_request_graph_epoch_and_late_reconfiguration() {
    use lrr::expand::budget::SharedExpansionBudget;
    let view = source("macro_rules! one{()=>{}}one!();");
    let request = Digest([246; 32]);
    for budget in [
        SharedExpansionBudget::new(Digest([247; 32]), Digest([245; 32]), 1, 8).unwrap(),
        SharedExpansionBudget::new(request, Digest([247; 32]), 1, 8).unwrap(),
        SharedExpansionBudget::new(request, Digest([245; 32]), 2, 8).unwrap(),
    ] {
        assert!(
            discover(&view)
                .with_expansion_budget(request, budget)
                .is_err()
        );
    }
    let budget = SharedExpansionBudget::new(request, Digest([245; 32]), 1, 8).unwrap();
    let mut first = discover(&view)
        .with_expansion_budget(request, budget.clone())
        .unwrap();
    assert!(matches!(
        first.poll().unwrap(),
        SourceDiscoveryStep::NeedExpansion { .. }
    ));
    let second = discover(&view)
        .with_expansion_budget(request, budget)
        .unwrap();
    assert!(second.with_generated_token_limit(7).is_err());
}
#[test]
fn shared_expansion_budget_serializes_competing_output_charges() {
    use lrr::expand::budget::SharedExpansionBudget;
    let mut first = discover(&source("macro_rules! one{()=>{fn first(){}}}one!();"));
    let mut second = discover(&source("macro_rules! two{()=>{fn other(){}}}two!();"));
    let SourceDiscoveryStep::NeedExpansion { request } = first.poll().unwrap() else {
        panic!("first pure demand absent")
    };
    let first = std::sync::Arc::new(request.expand().unwrap());
    let SourceDiscoveryStep::NeedExpansion { request } = second.poll().unwrap() else {
        panic!("second pure demand absent")
    };
    let second = std::sync::Arc::new(request.expand().unwrap());
    assert_ne!(first.key(), second.key());
    assert_eq!(first.token_count(), second.token_count());
    let limit = first.token_count();
    let budget =
        SharedExpansionBudget::new(Digest([246; 32]), Digest([245; 32]), 1, limit).unwrap();
    let a = budget.clone();
    let b = budget.clone();
    let first = std::thread::spawn(move || a.record_output(&first));
    let second = std::thread::spawn(move || b.record_output(&second));
    let results = [first.join().unwrap(), second.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    let error = results.into_iter().find_map(Result::err).unwrap();
    assert_eq!(error.code, "LRR-E-DISCOVERY-LIMIT");
    assert_eq!(error.message, "cumulative generated token limit");
    assert_eq!(budget.totals().unwrap().outputs, 1);
    assert_eq!(budget.totals().unwrap().tokens, limit);
}
#[test]
fn shared_expansion_deadline_is_frozen_and_expires_before_late_output_admission() {
    use lrr::expand::budget::SharedExpansionBudget;
    use std::time::Duration;
    let view = source("macro_rules! one{()=>{fn made(){}}}one!();");
    let request_id = Digest([246; 32]);
    let budget = SharedExpansionBudget::new(request_id, Digest([245; 32]), 1, 2_000_000)
        .unwrap()
        .with_deadlines(Duration::from_secs(1), Duration::from_millis(5))
        .unwrap();
    let mut work = discover(&view)
        .with_expansion_budget(request_id, budget.clone())
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("deadline source demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    std::thread::sleep(Duration::from_millis(20));
    let error = work.admit_expansion(record.clone()).unwrap_err();
    assert_eq!(error.code, "LRR-E-MACRO-TIMEOUT");
    assert!(work.expansion_record(record.key()).is_none());
    assert_eq!(budget.totals().unwrap().outputs, 0);
    assert!(work.poll().is_err());
    assert!(
        budget
            .with_deadlines(Duration::from_secs(1), Duration::from_secs(2))
            .is_err()
    );
}
#[test]
fn shared_expansion_deadlines_cannot_exceed_specification_maximums() {
    use lrr::expand::budget::SharedExpansionBudget;
    use std::time::Duration;
    for (invocation, cumulative) in [
        (Duration::ZERO, Duration::from_secs(1)),
        (Duration::from_secs(11), Duration::from_secs(60)),
        (Duration::from_secs(10), Duration::from_secs(61)),
    ] {
        assert!(
            SharedExpansionBudget::new(Digest([246; 32]), Digest([245; 32]), 1, 2_000_000)
                .unwrap()
                .with_deadlines(invocation, cumulative)
                .is_err()
        );
    }
}
#[test]
fn discovery_cumulative_token_limit_refuses_before_integrating_later_output() {
    let view = source("macro_rules! one{()=>{fn made(){}}}one!();one!();");
    let mut work = discover(&view).with_generated_token_limit(8).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("first demand absent")
    };
    let first = std::sync::Arc::new(request.expand().unwrap());
    let first_tokens = first.token_count();
    assert!(first_tokens <= 8 && first_tokens > 0);
    work.admit_expansion(first.clone()).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("second demand absent")
    };
    let second = std::sync::Arc::new(request.expand().unwrap());
    assert!(first_tokens + second.token_count() > 8);
    let error = work.admit_expansion(second.clone()).unwrap_err();
    assert_eq!(error.code, "LRR-E-DISCOVERY-LIMIT");
    assert_eq!(error.message, "cumulative generated token limit");
    assert!(work.expansion_record(first.key()).is_some());
    assert!(work.expansion_record(second.key()).is_none());
    assert!(work.sealed_expansions().is_err());
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::NeedExpansion { .. }
    ));
}
#[test]
fn discovery_token_limit_configuration_is_bounded_and_frozen_before_polling() {
    let view = source("macro_rules! one{()=>{}}one!();");
    assert!(
        discover(&view)
            .with_generated_token_limit(2_000_001)
            .is_err()
    );
    let mut work = discover(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::NeedExpansion { .. }
    ));
    assert!(work.with_generated_token_limit(1).is_err());
}
#[test]
fn discovery_generated_child_output_shares_parent_token_budget() {
    let view =
        source("macro_rules! one{()=>{macro_rules! next{()=>{fn made(){}}}next!();}}one!();");
    let mut preview = discover(&view);
    let SourceDiscoveryStep::NeedExpansion { request } = preview.poll().unwrap() else {
        panic!("preview demand absent")
    };
    let parent = std::sync::Arc::new(request.expand().unwrap());
    let mut work = discover(&view)
        .with_generated_token_limit(parent.token_count())
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("parent demand absent")
    };
    assert_eq!(request.key(), parent.key());
    work.admit_expansion(parent.clone()).unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = work.poll().unwrap() else {
        panic!("child demand absent")
    };
    let child = std::sync::Arc::new(request.expand().unwrap());
    assert!(child.token_count() > 0);
    assert_eq!(
        work.admit_expansion(child.clone()).unwrap_err().message,
        "cumulative generated token limit"
    );
    assert!(work.expansion_record(parent.key()).is_some());
    assert!(work.expansion_record(child.key()).is_none());
    assert!(work.sealed_expansions().is_err());
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::NeedGeneratedExpansion { .. }
    ));
}
#[test]
fn discovery_token_budget_counts_empty_transport_group_and_preserves_seal_error() {
    let view = source("macro_rules! one{()=>{}}one!();");
    let mut work = discover(&view).with_generated_token_limit(0).unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("empty demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    assert_eq!(record.token_count(), 1);
    assert_eq!(
        work.admit_expansion(record.clone()).unwrap_err().code,
        "LRR-E-DISCOVERY-LIMIT"
    );
    assert!(work.expansion_record(record.key()).is_none());
    let mut work = discover(&view)
        .with_generated_token_limit(record.token_count())
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = work.poll().unwrap() else {
        panic!("reduced-limit demand absent")
    };
    assert_eq!(request.key(), record.key());
    work.admit_expansion(record.clone()).unwrap();
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    assert_eq!(
        work.admit_expansion(record).unwrap_err().code,
        "LRR-E-EXPANSION-SEAL-VIOLATION"
    );
}
#[test]
fn source_discovery_schedules_generated_leaf_and_binds_both_record_inputs() {
    let v = source(
        "use made as selected;macro_rules! seed{()=>{macro_rules! leaf{()=>{pub fn made(){7_i64;}}}leaf!();}}seed!();",
    );
    let mut w = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request: outer } = w.poll().unwrap() else {
        panic!("outer demand absent")
    };
    let parent = std::sync::Arc::new(outer.expand().unwrap());
    w.admit_expansion(parent.clone()).unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = w.poll().unwrap() else {
        panic!("generated demand absent")
    };
    assert_ne!(request.key().parent_context, outer.key().parent_context);
    let SourceDiscoveryStep::NeedGeneratedExpansion { request: again } = w.poll().unwrap() else {
        panic!("generated demand lost")
    };
    assert!(std::sync::Arc::ptr_eq(&request, &again));
    assert!(w.lookup("selected").is_err());
    let record = std::sync::Arc::new(request.expand().unwrap());
    let items =
        lrr::expand::generated_items::GeneratedItems::admit(request.key(), record.clone(), 1)
            .unwrap();
    let identity = items.items()[0].key.fingerprint();
    let mut wrong = request.key().clone();
    wrong.invocation = Digest([250; 32]);
    let bad = std::sync::Arc::new(
        lrr::expand::record::ExpansionRecord::from_tokens(wrong, &record.tokens().unwrap())
            .unwrap(),
    );
    assert_eq!(
        w.admit_expansion(bad).unwrap_err().code,
        "LRR-E-EXPANSION-INPUT"
    );
    w.admit_expansion(record.clone()).unwrap();
    let SourceDiscoveryStep::Sealed(seal) = w.poll().unwrap() else {
        panic!("completed fragment did not seal")
    };
    let lrr::frontend::names::NameLookup::Defined(d) = w.lookup("selected").unwrap() else {
        panic!("generated leaf import absent")
    };
    assert_eq!(d.identity, identity);
    assert_eq!(w.sealed_expansions().unwrap().count(), 2);
    for record in [parent, record] {
        assert!(
            seal.input_dependencies()
                .binary_search(&(
                    record.key().fingerprint(),
                    record.descriptor().fingerprint()
                ))
                .is_ok()
        );
    }
}
#[test]
fn source_discovery_generated_calls_close_before_outward_original_use() {
    let v = source("macro_rules! seed{()=>{macro_rules! next{()=>{}}next!();}}seed!();next!();");
    let mut w = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("outer demand absent")
    };
    w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = w.poll().unwrap() else {
        panic!("generated call not ordered before outward use")
    };
    w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("outward binding not published")
    };
    w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    assert_eq!(w.sealed_expansions().unwrap().count(), 3);
}
#[test]
fn source_discovery_generated_cfg_and_unresolved_order_preserve_incompleteness() {
    let v = source(
        "macro_rules! seed{()=>{#[cfg(any())]unknown!();macro_rules! leaf{()=>{}}leaf!();leaf!();}}seed!();",
    );
    let mut w = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("outer demand absent")
    };
    w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for _ in 0..2 {
        let SourceDiscoveryStep::NeedGeneratedExpansion { request } = w.poll().unwrap() else {
            panic!("active generated call absent")
        };
        assert!(ids.insert(request.key().invocation));
        w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            .unwrap();
    }
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    let v = source("macro_rules! seed{()=>{unknown!();macro_rules! leaf{()=>{}}leaf!();}}seed!();");
    let mut w = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("outer demand absent")
    };
    w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = w.poll().unwrap() else {
        panic!("ready leaf was hidden behind the unresolved producer")
    };
    w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(
        w.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(w.sealed_expansions().is_err());
}

#[test]
fn source_discovery_recursive_generated_output_retains_unresolved_child_frontier() {
    let v = source(
        "macro_rules! seed{()=>{macro_rules! leaf{()=>{unknown!();pub fn provisional(){}}}leaf!();}}seed!();",
    );
    let mut w = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("outer demand absent")
    };
    w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = w.poll().unwrap() else {
        panic!("generated demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    w.admit_expansion(record.clone()).unwrap();
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = w.poll().unwrap()
    else {
        panic!("unresolved recursive producer certified complete")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    let SourceDiscoveryStep::Incomplete { blocked: again, .. } = w.poll().unwrap() else {
        panic!("unresolved child frontier lost")
    };
    assert_eq!(blocked, again);
    assert!(std::sync::Arc::ptr_eq(
        &w.expansion_record(record.key()).unwrap(),
        &record
    ));
    assert!(w.sealed_expansions().is_err());
    assert!(w.lookup("provisional").is_err());
}
