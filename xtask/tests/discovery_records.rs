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
fn discovery_retains_exact_record_after_external_owner_is_dropped() {
    let v = source("macro_rules! make{()=>{pub fn made(){7_i64;}}}make!();unknown!();");
    let mut w = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("demand absent")
    };
    let key = request.key().clone();
    let record = std::sync::Arc::new(request.expand().unwrap());
    let descriptor = record.descriptor().fingerprint();
    let weak = std::sync::Arc::downgrade(&record);
    w.admit_expansion(record).unwrap();
    let retained = w.expansion_record(&key).unwrap();
    assert!(std::sync::Arc::ptr_eq(&retained, &weak.upgrade().unwrap()));
    assert_eq!(retained.descriptor().fingerprint(), descriptor);
    assert!(w.sealed_expansions().is_err());
    assert!(matches!(
        w.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(w.expansion_record(&key).is_some());
    assert!(w.sealed_expansions().is_err());
}
#[test]
fn discovery_refused_reply_does_not_enter_immutable_record_index() {
    let v = source("macro_rules! make{()=>{}}make!();");
    let mut w = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("demand absent")
    };
    let record = request.expand().unwrap();
    let mut wrong = request.key().clone();
    wrong.engine_version = Digest([249; 32]);
    let bad = std::sync::Arc::new(
        lrr::expand::record::ExpansionRecord::from_tokens(wrong.clone(), &record.tokens().unwrap())
            .unwrap(),
    );
    assert_eq!(
        w.admit_expansion(bad).unwrap_err().code,
        "LRR-E-EXPANSION-INPUT"
    );
    assert!(w.expansion_record(&wrong).is_none());
    assert!(w.expansion_record(request.key()).is_none());
    w.admit_expansion(std::sync::Arc::new(record)).unwrap();
    assert!(w.expansion_record(request.key()).is_some());
}
#[test]
fn sealed_record_inputs_are_sorted_and_bound_to_certificate_dependencies() {
    let v = source("macro_rules! noop{()=>{}}noop!();noop!();");
    let mut w = discover(&v);
    let mut expected = Vec::new();
    for _ in 0..2 {
        let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
            panic!("demand absent")
        };
        let record = std::sync::Arc::new(request.expand().unwrap());
        expected.push((
            record.key().fingerprint(),
            record.descriptor().fingerprint(),
        ));
        w.admit_expansion(record).unwrap();
    }
    let SourceDiscoveryStep::Sealed(seal) = w.poll().unwrap() else {
        panic!("seal absent")
    };
    expected.sort();
    let actual = w
        .sealed_expansions()
        .unwrap()
        .map(|r| (r.key().fingerprint(), r.descriptor().fingerprint()))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert!(
        expected
            .iter()
            .all(|p| seal.input_dependencies().binary_search(p).is_ok())
    );
    let record = w.sealed_expansions().unwrap().next().unwrap().clone();
    assert_eq!(
        w.admit_expansion(record).unwrap_err().code,
        "LRR-E-EXPANSION-SEAL-VIOLATION"
    );
    assert_eq!(w.sealed_expansions().unwrap().count(), 2);
}
