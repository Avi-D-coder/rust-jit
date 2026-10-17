//! Composition controls for genuine successful pure-memo input captures.
//! Private malformed guards are used only to exercise defensive refusals.
use super::*;
use crate as lrr;
use crate::{
    core::profile::ProfileIdentity,
    db::{dependency::InputValue, provider::Provided},
};

include!("../../../../xtask/tests/support/constant_key.rs");
include!("../../../../xtask/tests/support/constant_value.rs");

fn key(n: u8) -> QueryKey {
    QueryKey::Constant(fixture_key(
        Digest::of_bytes(&[n]),
        vec![],
        Digest::default(),
        ProfileIdentity::current(),
    ))
}
fn positive() -> InputKey {
    InputKey::CompileEnv("join-positive".into())
}
fn second() -> InputKey {
    InputKey::CompileEnv("join-second".into())
}
fn absent() -> InputKey {
    InputKey::CompileEnv("join-absence".into())
}

struct ActualInputs;
impl Provider for ActualInputs {
    fn execute(&self, db: &mut Database, requested: &QueryKey) -> QueryPoll<Provided> {
        assert!(requested == &key(1) || requested == &key(2));
        db.input(&positive());
        let other = if requested == &key(1) {
            absent()
        } else {
            second()
        };
        db.input(&other);
        Provided::pure(fixture_value(requested, 1))
    }
}
fn publish(db: &mut Database, version: &str, missing_now_present: bool, unrelated: bool) {
    let mut rows = BTreeMap::from([
        (positive(), InputValue::CompileEnv(version.into())),
        (second(), InputValue::CompileEnv("second-v1".into())),
    ]);
    if missing_now_present {
        rows.insert(absent(), InputValue::CompileEnv("now-present".into()));
    }
    if unrelated {
        rows.insert(
            InputKey::CompileEnv("join-unrelated".into()),
            InputValue::CompileEnv("other".into()),
        );
    }
    db.publish_inputs(rows).unwrap();
}
fn captured(db: &mut Database, n: u8) -> PreparedInputGuard {
    let result = db
        .fetch(&ActualInputs, key(n))
        .into_ready()
        .unwrap()
        .unwrap();
    db.capture_prepared_inputs(&key(n), result.fingerprint)
        .unwrap()
}
fn world() -> (Database, PreparedInputGuard, PreparedInputGuard) {
    let mut db = Database::default();
    db.trace.detailed = true;
    publish(&mut db, "v1", false, false);
    let first = captured(&mut db, 1);
    let second = captured(&mut db, 2);
    assert_eq!(first.inputs.len(), 2);
    assert_eq!(second.inputs.len(), 2);
    (db, first, second)
}

#[test]
fn actual_guard_join_is_deterministic_and_deduplicates_without_queries() {
    let (db, first, second) = world();
    let first_inputs = first.inputs.clone();
    let second_inputs = second.inputs.clone();
    let events = db.trace.events.len();
    let joined = PreparedInputGuard::join(&[&first, &second, &first]).unwrap();
    let reordered = PreparedInputGuard::join(&[&second, &first]).unwrap();
    let nested = PreparedInputGuard::join(&[&joined, &first]).unwrap();
    assert_eq!(joined.inputs.len(), 3);
    assert!(joined.inputs.windows(2).all(|rows| rows[0] < rows[1]));
    assert_eq!(joined.inputs, reordered.inputs);
    assert_eq!(joined.fingerprint(), reordered.fingerprint());
    assert_eq!(joined.fingerprint(), nested.fingerprint());
    assert_eq!(first.inputs, first_inputs);
    assert_eq!(second.inputs, second_inputs);
    assert!(
        joined.metadata_bytes()
            >= std::mem::size_of::<PreparedInputGuard>()
                + 3 * std::mem::size_of::<DependencyStamp>()
    );
    for guard in [&joined, &reordered, &nested, &first, &second] {
        guard.require_current(&db).unwrap();
    }
    assert_eq!(
        db.trace.events.len(),
        events,
        "join/currentness must never fetch or plan"
    );
}

#[test]
fn actual_old_and_new_input_captures_cannot_be_joined_to_refresh_stamps() {
    let (mut db, old, _) = world();
    publish(&mut db, "v2", false, false);
    let current = captured(&mut db, 1);
    assert_ne!(old.fingerprint(), current.fingerprint());
    let events = db.trace.events.len();
    for guards in [[&old, &current], [&current, &old]] {
        let error = PreparedInputGuard::join(&guards).unwrap_err();
        assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
        assert_eq!(error.message, "prepared input join stamps conflict");
    }
    assert_eq!(
        old.require_current(&db).unwrap_err().message,
        "prepared operation input changed"
    );
    current.require_current(&db).unwrap();
    assert_eq!(db.trace.events.len(), events);
}

#[test]
fn joined_actual_guard_ignores_unrelated_input_but_refuses_positive_or_absence_changes() {
    for change in [0, 1] {
        let (mut db, first, second) = world();
        let joined = PreparedInputGuard::join(&[&first, &second]).unwrap();
        publish(&mut db, "v1", false, true);
        let events = db.trace.events.len();
        joined.require_current(&db).unwrap();
        assert_eq!(db.trace.events.len(), events);
        publish(
            &mut db,
            if change == 0 { "v2" } else { "v1" },
            change == 1,
            true,
        );
        let events = db.trace.events.len();
        let error = joined.require_current(&db).unwrap_err();
        assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
        assert_eq!(error.message, "prepared operation input changed");
        assert_eq!(
            db.trace.events.len(),
            events,
            "revocation must not repair or refresh a guard"
        );
    }
}

#[test]
fn malformed_query_stamp_is_refused_even_when_its_actual_memo_exists() {
    let (db, first, _) = world();
    // Production capture flattens this genuine derived memo into input stamps.
    // An internal malformed receipt must not turn it back into a live query.
    let inputs = vec![DependencyStamp::Query {
        key: key(1),
        fingerprint: db.memo(&key(1)).unwrap().fingerprint,
    }];
    let malformed = PreparedInputGuard {
        fingerprint: guard_fingerprint(&inputs),
        inputs,
        bytes: std::mem::size_of::<PreparedInputGuard>(),
        checked: Default::default(),
    };
    let events = db.trace.events.len();
    let error = PreparedInputGuard::join(&[&first, &malformed]).unwrap_err();
    assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
    assert_eq!(
        error.message,
        "prepared input join contains a derived demand"
    );
    first.require_current(&db).unwrap();
    assert_eq!(db.trace.events.len(), events);
}

