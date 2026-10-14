use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database, poll::QueryPoll},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{BodyIndex, BodyIndexLimits},
        parse::{ParseKey, ParsedFile},
    },
};
fn prepare(text: &str) -> (PreparedSourceInputs, PreparedSourceView, BodyIndex) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([220; 32]);
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
    let parsed = ParsedFile::parse(&blob, profile.clone()).unwrap();
    let outline = parsed.outline(unit, vec![], &cfg).unwrap();
    let body = outline.items[0].body.as_ref().unwrap();
    let index = parsed.index_body(body, BodyIndexLimits::default()).unwrap();
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
    (inputs, view, index)
}
fn run(text: &str) -> lrr::engine::run::RunPoll {
    use lrr::engine::run::RunMachine;
    let (inputs, view, index) = prepare(text);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_run().unwrap();
    let QueryPoll::Ready(Ok(mut run)) =
        RunMachine::start_with_sources(&mut db, index.body().clone(), 4000, vec![view.share()])
    else {
        panic!("run absent")
    };
    let outcome = run.poll(&mut db);
    db.finish_run().unwrap();
    outcome
}
fn value(text: &str, wanted: &str) {
    use lrr::engine::run::{RunPoll, ValueSummary};
    match run(text) {
        RunPoll::Completed(report) => {
            assert!(matches!(report.value,ValueSummary::Signed(ref n) if n==wanted))
        }
        RunPoll::Fault(e) => panic!("condition binding fault: {e:?}"),
        _ => panic!("condition binding did not complete"),
    }
}
#[test]
fn reached_scalar_condition_binding_modes_preserve_shadowing_and_cold_reference_patterns() {
    value(
        "fn case()->i64{let x=9_i64;if let mut x=2_i64{x=x+1_i64;x}else{missing()}}",
        "3",
    );
    value(
        "fn case()->i64{if false && let ref x=missing(){x}else{4_i64}}",
        "4",
    );
    let lrr::engine::run::RunPoll::Fault(error) =
        run("fn case()->i64{if let ref x=2_i64{0_i64}else{1_i64}}")
    else {
        panic!("reference execution incorrectly admitted")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert_eq!(
        error.message,
        "condition binding mode awaits pattern integration"
    );
}

use lrr::vm::pattern::{
    BindingModifier as B, DefaultBindingMode as M, PatternContext, ReferenceKind as R,
};
#[test]
fn implicit_pattern_reference_peeling_never_upgrades_a_shared_binding_mode() {
    let c = PatternContext::new(2024);
    let c = c.unwrap();
    assert_eq!(c.mode(), M::Move);
    let mutable = c.peel_implicit(R::Mutable);
    assert_eq!(mutable.mode(), M::RefMut);
    assert_eq!(mutable.peel_implicit(R::Mutable).mode(), M::RefMut);
    let shared = mutable.peel_implicit(R::Shared);
    assert_eq!(shared.mode(), M::Ref);
    assert_eq!(shared.peel_implicit(R::Mutable).mode(), M::Ref);
    assert_eq!(c.peel_implicit(R::Shared).mode(), M::Ref);
    assert_eq!(c.mode(), M::Move);
}
#[test]
fn reached_edition2024_explicit_pattern_modifiers_require_move_mode() {
    let c = PatternContext::new(2024).unwrap();
    for (modifier, wanted) in [
        (B::Plain, M::Move),
        (B::Mut, M::Move),
        (B::Ref, M::Ref),
        (B::RefMut, M::RefMut),
    ] {
        assert_eq!(c.binding_mode(modifier).unwrap(), wanted);
    }
    assert_eq!(c.explicit_reference().unwrap().mode(), M::Move);
    for reference in [R::Shared, R::Mutable] {
        let inherited = c.peel_implicit(reference);
        assert_eq!(inherited.binding_mode(B::Plain).unwrap(), inherited.mode());
        for modifier in [B::Mut, B::Ref, B::RefMut] {
            assert_eq!(
                inherited.binding_mode(modifier).unwrap_err().code,
                "LRR-E-EDITION-PATTERN"
            );
        }
        assert_eq!(
            inherited.explicit_reference().unwrap_err().code,
            "LRR-E-EDITION-PATTERN"
        );
    }
}
#[test]
fn older_pattern_editions_preserve_mut_reset_and_explicit_reference_behavior() {
    for edition in [2015, 2018, 2021] {
        let c = PatternContext::new(edition)
            .unwrap()
            .peel_implicit(R::Shared);
        assert_eq!(c.binding_mode(B::Plain).unwrap(), M::Ref);
        assert_eq!(c.binding_mode(B::Mut).unwrap(), M::Move);
        assert_eq!(c.binding_mode(B::Ref).unwrap(), M::Ref);
        assert_eq!(c.binding_mode(B::RefMut).unwrap(), M::RefMut);
        assert_eq!(c.explicit_reference().unwrap().mode(), M::Move);
    }
    assert_eq!(
        PatternContext::new(2027).unwrap_err().code,
        "LRR-E-TOOLCHAIN-CAPABILITY"
    );
}
#[test]
fn pattern_context_identity_separates_edition_and_inherited_binding_mode() {
    let mut identities = std::collections::BTreeSet::new();
    for edition in [2015, 2018, 2021, 2024] {
        let c = PatternContext::new(edition).unwrap();
        for c in [c, c.peel_implicit(R::Shared), c.peel_implicit(R::Mutable)] {
            assert!(identities.insert(c.fingerprint()));
            assert_eq!(c.fingerprint(), c.fingerprint());
        }
    }
    assert_eq!(identities.len(), 12);
}
