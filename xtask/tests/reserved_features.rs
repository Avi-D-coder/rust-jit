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
fn reserved_generator_syntax_remains_opaque_and_only_reached_execution_refuses() {
    let text = "fn case()->i64{gen {missing()};1_i64}";
    let (_, _, index) = prepare(text);
    let generator = index
        .nodes()
        .iter()
        .find(|n| {
            n.role
                == lrr::syntax::body_index::NodeRole::Expression(
                    lrr::syntax::body_index::ExpressionKind::Gen,
                )
        })
        .expect("generator syntax missing");
    assert!(generator.opaque);
    assert!(!index.nodes().iter().any(|n| n.role
        == lrr::syntax::body_index::NodeRole::Expression(
            lrr::syntax::body_index::ExpressionKind::Call
        )));
    let lrr::engine::run::RunPoll::Fault(error) = run(text) else {
        panic!("generator execution admitted")
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    value("fn case()->i64{if false {gen {missing()};}1_i64}", "1");
}
