use lrr::db::poll::QueryPoll;
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        provider::{Provided, Provider, SyntaxProvider},
    },
};
use std::{cell::RefCell, collections::BTreeMap};
fn key(n: u8) -> QueryKey {
    QueryKey::Constant(fixture_key(
        Digest::of_bytes(&[n]),
        vec![],
        Digest::default(),
        ProfileIdentity::current(),
    ))
}
struct Graph {
    counts: RefCell<BTreeMap<u8, u32>>,
}
impl Provider for Graph {
    fn execute(&self, db: &mut Database, requested: &QueryKey) -> QueryPoll<Provided> {
        let n = (0..4).find(|n| key(*n) == *requested).unwrap();
        *self.counts.borrow_mut().entry(n).or_default() += 1;
        let route = db.input(&InputKey::CompileEnv(format!("route{n}")));
        let Some(InputValue::CompileEnv(route)) = route.as_deref() else {
            return Provided::error("LRR-E-NAME-MISSING", "route missing");
        };
        let mut values = vec![];
        for part in route.split(',').filter(|s| !s.is_empty()) {
            let result = match db.fetch(self, key(part.parse().unwrap())) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            };
            values.push(fixture_number(result.value.as_ref()));
        }
        let value = if n == 0 {
            match db.input(&InputKey::CompileEnv("value".into())).as_deref() {
                Some(InputValue::CompileEnv(v)) => v.parse().unwrap(),
                _ => return Provided::error("LRR-E-NAME-MISSING", "value missing"),
            }
        } else if n == 3 {
            values.iter().sum()
        } else {
            values[0] % 2
        };
        Provided::pure(fixture_value(requested, value))
    }
}
fn inputs(value: &str, route1: &str) -> BTreeMap<InputKey, InputValue> {
    [
        ("route0", ""),
        ("route1", route1),
        ("route2", "0"),
        ("route3", "1,2"),
        ("value", value),
    ]
    .into_iter()
    .map(|(k, v)| {
        (
            InputKey::CompileEnv(k.into()),
            InputValue::CompileEnv(v.into()),
        )
    })
    .collect()
}
fn number(db: &mut Database, g: &Graph, n: u8) -> i128 {
    fixture_number(
        db.fetch(g, key(n))
            .into_ready()
            .unwrap()
            .unwrap()
            .value
            .as_ref(),
    )
}

#[test]
fn diamond_recomputes_once_and_unchanged_output_reuses_downstream() {
    let g = Graph {
        counts: RefCell::new(BTreeMap::new()),
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    db.publish_inputs(inputs("1", "0")).unwrap();
    assert_eq!(number(&mut db, &g, 3), 2);
    assert_eq!(
        *g.counts.borrow(),
        BTreeMap::from([(0, 1), (1, 1), (2, 1), (3, 1)])
    );
    assert_eq!(number(&mut db, &g, 3), 2);
    assert_eq!(g.counts.borrow()[&0], 1);
    db.publish_inputs(inputs("2", "0")).unwrap();
    assert_eq!(number(&mut db, &g, 3), 0);
    assert!(g.counts.borrow().values().all(|n| *n == 2));
    db.publish_inputs(inputs("4", "0")).unwrap();
    assert_eq!(number(&mut db, &g, 3), 0);
    assert_eq!(
        *g.counts.borrow(),
        BTreeMap::from([(0, 3), (1, 3), (2, 3), (3, 2)])
    );
    assert!(
        db.trace
            .events
            .iter()
            .any(|e| e.action == lrr::db::trace::Action::ValidatedReuse)
    );
}
#[test]
fn active_cycle_is_not_cached_and_edit_breaks_it_immediately() {
    let g = Graph {
        counts: RefCell::new(BTreeMap::new()),
    };
    let mut db = Database::default();
    db.publish_inputs(inputs("1", "3")).unwrap();
    let e = db.fetch(&g, key(3)).into_ready().unwrap().unwrap_err();
    assert_eq!(e.code, "LRR-E-CONST-CYCLE");
    assert!(e.message.contains(&key(3).fingerprint().hex()));
    assert!(db.memo(&key(3)).is_none());
    assert!(db.memo(&key(1)).is_none());
    db.publish_inputs(inputs("1", "0")).unwrap();
    assert_eq!(number(&mut db, &g, 3), 2);
    assert!(db.memo(&key(3)).is_some());
}
#[test]
fn deterministic_missing_error_is_cached_and_new_input_invalidates_it() {
    let mut db = Database::default();
    let key = QueryKey::Signature(Digest::of_bytes(b"missing"));
    let first = db
        .fetch(&SyntaxProvider, key.clone())
        .into_ready()
        .unwrap()
        .unwrap_err();
    let value = db.memo(&key).unwrap().value.clone();
    db.publish_inputs(BTreeMap::new()).unwrap();
    let second = db
        .fetch(&SyntaxProvider, key.clone())
        .into_ready()
        .unwrap()
        .unwrap_err();
    assert_eq!(first.code, second.code);
    assert_eq!(first.message, second.message);
    assert!(std::sync::Arc::ptr_eq(
        &value,
        &db.memo(&key).unwrap().value
    ));
    assert_eq!(db.trace.counts["Signature:ProviderRun"], 1);
}
#[test]
fn result_variant_mismatch_is_non_cacheable_and_stack_is_cleaned() {
    struct Bad;
    impl Provider for Bad {
        fn execute(&self, _: &mut Database, requested: &QueryKey) -> QueryPoll<Provided> {
            Provided::pure(fixture_value(requested, 0))
        }
    }
    let mut db = Database::default();
    let k = QueryKey::Signature(Digest::default());
    let e = db.fetch(&Bad, k.clone()).into_ready().unwrap().unwrap_err();
    assert_eq!(e.code, "LRR-E-INTERNAL-SNAPSHOT");
    assert!(db.memo(&k).is_none());
    assert!(db.publish_inputs(BTreeMap::new()).is_ok());
}

include!("../../../xtask/tests/support/constant_key.rs");
include!("../../../xtask/tests/support/constant_value.rs");
include!("../../../xtask/tests/support/constant_number.rs");
