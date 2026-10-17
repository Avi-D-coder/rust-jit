//! Currentness/accounting controls, not a source or execution authority grant.
use super::*;
use crate as lrr;
use crate::{
    core::profile::ProfileIdentity,
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::{Provided, Provider},
        value::QueryValue,
    },
    engine::{install::InstalledRecipes, telemetry::CacheBudgets},
    ir::pc::{NodeId, Phase},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::parse::ParsedFile,
};
use std::{collections::BTreeMap, sync::Arc};
include!("../../../../xtask/tests/support/constant_key.rs");
include!("../../../../xtask/tests/support/constant_value.rs");
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
fn raw_recipe() -> VerifiedRecipe {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"recipe-input-authority-source-control"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let body = parsed.index_body(handle, Default::default()).unwrap();
    let pc = body.pc(NodeId(0), Phase::Entry).unwrap();
    parsed.initial_thunk(handle, &body, &pc).unwrap()
}
fn key() -> QueryKey {
    QueryKey::Constant(fixture_key(
        Digest::of_bytes(b"recipe-input-query-control"),
        vec![],
        Digest::default(),
        ProfileIdentity::current(),
    ))
}
fn positive() -> InputKey {
    InputKey::CompileEnv("recipe-relevant-present".into())
}
fn negative() -> InputKey {
    InputKey::CompileEnv("recipe-relevant-absent".into())
}
fn inputs(
    value: &str,
    unrelated: bool,
    absent_now_present: bool,
) -> BTreeMap<InputKey, InputValue> {
    let mut inputs = BTreeMap::from([(positive(), InputValue::CompileEnv(value.into()))]);
    if unrelated {
        inputs.insert(
            InputKey::CompileEnv("recipe-unrelated".into()),
            InputValue::CompileEnv("other".into()),
        );
    }
    if absent_now_present {
        inputs.insert(negative(), InputValue::CompileEnv("now-present".into()));
    }
    inputs
}
struct Reads;
impl Provider for Reads {
    fn execute(&self, db: &mut Database, requested: &QueryKey) -> QueryPoll<Provided> {
        db.input(&positive());
        db.input(&negative());
        Provided::pure(fixture_value(requested, 1))
    }
}
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("recipe input control error: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("recipe input control demand: {input:?}"),
    }
}
fn capture(db: &mut Database) -> PreparedInputGuard {
    let outcome = ready(db.fetch(&Reads, key()));
    ready(db.prepare_input_guard(&Reads, &key(), outcome.fingerprint))
}
#[test]
fn recipe_guard_checks_relevant_inputs_without_queries_after_memo_eviction() {
    let mut db = Database::default();
    db.trace.detailed = true;
    db.publish_inputs(inputs("v1", false, false)).unwrap();
    let raw = raw_recipe();
    let guard = capture(&mut db);
    let expected_bytes =
        raw.metadata_bytes() + guard.metadata_bytes() + 2 * std::mem::size_of::<usize>();
    let shared = Arc::new(guard);
    let other_phase = raw
        .clone()
        .with_shared_prepared_inputs(Arc::clone(&shared))
        .unwrap();
    let recipe = raw
        .with_shared_prepared_inputs(Arc::clone(&shared))
        .unwrap();
    assert!(Arc::ptr_eq(
        other_phase.prepared_inputs.as_ref().unwrap(),
        &shared
    ));
    assert!(Arc::ptr_eq(
        recipe.prepared_inputs.as_ref().unwrap(),
        &shared
    ));
    assert_eq!(recipe.metadata_bytes(), expected_bytes);
    assert_eq!(other_phase.metadata_bytes(), expected_bytes);
    drop(other_phase);
    drop(shared);
    let clone = recipe.clone();
    assert!(Arc::ptr_eq(
        recipe.prepared_inputs.as_ref().unwrap(),
        clone.prepared_inputs.as_ref().unwrap()
    ));
    assert_eq!(recipe.metadata_bytes(), clone.metadata_bytes());
    let mut installed = InstalledRecipes::new(db.source_revision(), 8 * 1024 * 1024).unwrap();
    let id = installed
        .install_prepared(&db, recipe.recipe().key.clone(), &recipe)
        .unwrap();
    let before = db.trace.events.len();
    installed.get_current(&db, id).unwrap();
    assert_eq!(db.trace.events.len(), before);
    drop(installed);
    // Whole-universe replacement preserves the actual positive observation and
    // absence. Installed source_revision is a separate, coarser held guard;
    // this probe exercises only relevant recipe currentness, not that bypass.
    db.publish_inputs(inputs("v1", true, false)).unwrap();
    let floor = db.cache_usage().unwrap().pinned_memo_bytes;
    db.evict_between_runs(CacheBudgets {
        query_bytes: floor,
        ..CacheBudgets::default()
    })
    .unwrap();
    assert!(db.memo(&key()).is_none());
    let before = db.trace.events.len();
    recipe.revalidate_namespace(&db).unwrap();
    clone.revalidate_namespace(&db).unwrap();
    assert_eq!(
        db.trace.events.len(),
        before,
        "current-use validation re-entered a query"
    );
    db.publish_inputs(inputs("v1", true, true)).unwrap();
    let before = db.trace.events.len();
    assert_eq!(
        recipe.revalidate_namespace(&db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
    assert_eq!(db.trace.events.len(), before);
    db.publish_inputs(inputs("v2", true, false)).unwrap();
    assert_eq!(
        clone.revalidate_namespace(&db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
}
#[test]
fn recipe_envelope_distinguishes_new_stamps_and_cannot_replace_attachment() {
    let mut db = Database::default();
    db.publish_inputs(inputs("v1", false, false)).unwrap();
    let raw = raw_recipe();
    let raw_identity = raw.recipe().fingerprint();
    let legacy = raw.clone().with_namespace_witness(&db).unwrap();
    let legacy_witness = Arc::clone(legacy.namespace_witness.as_ref().unwrap());
    let recipe = legacy.with_prepared_inputs(capture(&mut db)).unwrap();
    assert!(Arc::ptr_eq(
        recipe.namespace_witness.as_ref().unwrap(),
        &legacy_witness
    ));
    recipe.revalidate_namespace(&db).unwrap();
    assert_eq!(
        recipe.recipe().fingerprint(),
        raw_identity,
        "currentness changed raw IR wire semantics"
    );
    assert!(
        recipe
            .clone()
            .with_prepared_inputs(capture(&mut db))
            .is_err(),
        "duplicate attachment refreshed a recipe"
    );
    let first = QueryValue::Thunk(Box::new(recipe));
    db.publish_inputs(inputs("v2", false, false)).unwrap();
    let next = raw.clone().with_prepared_inputs(capture(&mut db)).unwrap();
    assert!(first.matches(&QueryKey::Thunk(next.recipe().key.clone())));
    assert_eq!(next.recipe().fingerprint(), raw_identity);
    assert_ne!(
        first.fingerprint(),
        QueryValue::Thunk(Box::new(next)).fingerprint(),
        "new dependency envelope hidden by equal operations"
    );
}
struct RecipeProvider {
    recipe: VerifiedRecipe,
}
impl Provider for RecipeProvider {
    fn execute(&self, db: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
        if db.input(&positive()).is_none() {
            return Provided::error("LRR-E-NAME-MISSING", "actual relevant input revoked");
        }
        db.input(&negative());
        Provided::pure(QueryValue::Thunk(Box::new(self.recipe.clone())))
    }
}
#[test]
fn retained_recipe_envelope_is_charged_after_outer_memo_replacement_until_release() {
    let mut db = Database::default();
    db.publish_inputs(inputs("v1", false, false)).unwrap();
    let raw = raw_recipe();
    let recipe = raw.with_prepared_inputs(capture(&mut db)).unwrap();
    let allocation = Arc::downgrade(recipe.prepared_inputs.as_ref().unwrap());
    let charged = recipe.metadata_bytes();
    let requested = QueryKey::Thunk(recipe.recipe().key.clone());
    let provider = RecipeProvider { recipe };
    let outcome = ready(db.fetch(&provider, requested.clone()));
    let retained = outcome.value;
    db.publish_inputs(BTreeMap::new()).unwrap();
    match db.fetch(&provider, requested) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-NAME-MISSING"),
        other => panic!("revoked actual query did not replace its memo: {other:?}"),
    }
    drop(provider);
    assert!(allocation.upgrade().is_some());
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= charged);
    assert_eq!(
        db.evict_between_runs(CacheBudgets {
            query_bytes: 0,
            parse_bytes: 0,
            ..CacheBudgets::default()
        })
        .unwrap_err()
        .code,
        "LRR-E-METADATA-LIMIT"
    );
    let QueryValue::Thunk(recipe) = retained.as_ref() else {
        panic!("retained actual recipe variant differs")
    };
    assert_eq!(
        recipe.revalidate_namespace(&db).unwrap_err().code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
    drop(retained);
    assert!(allocation.upgrade().is_none());
    drop(allocation);
    db.evict_between_runs(CacheBudgets {
        query_bytes: 0,
        parse_bytes: 0,
        ..CacheBudgets::default()
    })
    .unwrap();
    assert_eq!(db.cache_usage().unwrap().memo_bytes, 0);
    assert_eq!(db.cache_usage().unwrap().pinned_memo_bytes, 0);
}
