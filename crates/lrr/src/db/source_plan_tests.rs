//! Upstream-derived pure-plan reuse/currentness and retained-allocation controls.
use super::*;
use crate::{
    core::{digest::StableEncode, profile::ProfileIdentity},
    engine::telemetry::CacheBudgets,
    source::{
        changes::{PreparedSourceInputs, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{closed_reference_signatures::ReferenceHeaderPlan, parse::ParsedFile},
};
const PARAMETERS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-call.rs"
);
const RESULT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-addr-of-ret.rs"
);
fn world(text: &str, name: &str, edition: u16) -> (Database, Digest) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let unit = SourceUnitInput {
        path: SourcePath::new("upstream-reference.rs").unwrap(),
        crate_key: Digest([191; 32]),
        module_path: vec![],
        profile: profile.clone(),
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
    };
    let parsed = ParsedFile::parse(&unit.blob, profile).unwrap();
    let outline = parsed.outline(unit.crate_key, vec![], &unit.cfg).unwrap();
    let definition = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
        .def_key
        .fingerprint();
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        vec![unit],
        Default::default(),
        vec![],
    )
    .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.trace.detailed = true;
    (db, definition)
}
fn header(db: &mut Database, definition: Digest) -> Arc<ReferenceHeaderPlan> {
    let outcome = db
        .fetch(
            &super::super::provider::SyntaxProvider,
            QueryKey::ReferenceHeader(definition),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::ReferenceHeader(plan) = outcome.value.as_ref() else {
        panic!("header plan absent")
    };
    Arc::clone(plan)
}
fn universe(db: &Database) -> BTreeMap<InputKey, InputValue> {
    db.inputs
        .iter()
        .map(|(key, record)| (key.clone(), record.value.as_ref().clone()))
        .collect()
}
#[test]
fn unchanged_reference_headers_share_one_source_plan_without_entering_any_body() {
    for edition in [2015, 2018, 2021, 2024] {
        for (text, name, parameters, has_result) in
            [(PARAMETERS, "takes_two", 2, false), (RESULT, "f", 1, true)]
        {
            let (mut db, definition) = world(text, name, edition);
            let one = header(&mut db, definition);
            let two = header(&mut db, definition);
            assert!(Arc::ptr_eq(&one, &two));
            assert_eq!(one.input_references(), parameters);
            assert_eq!(one.result().is_some(), has_result);
            assert!(one.parameter(0).is_some());
            assert_eq!(db.trace.counts.get("ReferenceHeader:ProviderRun"), Some(&1));
            assert!(!db.trace.events.iter().any(|event| matches!(
                event.key,
                QueryKey::BodyStructure(_) | QueryKey::Thunk(_) | QueryKey::PatternPlan(_)
            )));
            let mut inputs = universe(&db);
            let scope = match inputs
                .get(&InputKey::DeclarationContext(definition))
                .unwrap()
            {
                InputValue::DeclarationContext(context) => context.scope.clone(),
                _ => panic!(),
            };
            inputs.insert(
                InputKey::NameCandidates(crate::db::key::BucketKey {
                    scope,
                    namespace: crate::source::manifest::Namespace::Type,
                    name: "unrelated_type".into(),
                }),
                InputValue::NameCandidates(vec![Digest([192; 32])]),
            );
            db.publish_inputs(inputs).unwrap();
            assert!(Arc::ptr_eq(&one, &header(&mut db, definition)));
            assert_eq!(db.trace.counts.get("ReferenceHeader:ProviderRun"), Some(&1));
            assert_eq!(
                db.trace.counts.get("ReferenceHeader:ValidatedReuse"),
                Some(&1)
            );
            // These source facts deliberately do not admit a primitive name or
            // a caller region/ticket. Current Type admission remains separate.
            assert!(
                !db.memo(&QueryKey::ReferenceHeader(definition))
                    .unwrap()
                    .dependencies
                    .iter()
                    .any(|dep| matches!(
                        dep,
                        DependencyStamp::Input {
                            key: InputKey::NameCandidates(_)
                                | InputKey::ScopeMembers(_)
                                | InputKey::ScopeBlockers(_),
                            ..
                        } | DependencyStamp::Query {
                            key: QueryKey::SealedName(_) | QueryKey::ScopeBucket(_),
                            ..
                        }
                    ))
            );
        }
    }
}
#[test]
fn changed_source_header_context_and_body_cannot_replay_a_cached_reference_header() {
    for changed in 0..4 {
        let (mut db, definition) = world(RESULT, "f", 2024);
        let retained = header(&mut db, definition);
        let before = universe(&db);
        let mut next = before.clone();
        let key = match changed {
            0 => InputKey::DefinitionSource(definition),
            1 => InputKey::DefinitionHeader(definition),
            2 => InputKey::DeclarationContext(definition),
            _ => InputKey::DefinitionBody(definition),
        };
        match next.get_mut(&key).unwrap() {
            InputValue::DefinitionSource(source) => source.profile.edition = 2018,
            InputValue::DefinitionHeader(header) => header.profile.edition = 2018,
            InputValue::DeclarationContext(context) => context.range.start += 1,
            InputValue::DefinitionBody(body) => body.range.start += 1,
            _ => panic!(),
        }
        db.publish_inputs(next).unwrap();
        assert!(
            db.fetch(
                &super::super::provider::SyntaxProvider,
                QueryKey::ReferenceHeader(definition)
            )
            .into_ready()
            .unwrap()
            .is_err()
        );
        db.publish_inputs(before).unwrap();
        let restored = header(&mut db, definition);
        assert!(!Arc::ptr_eq(&retained, &restored));
        assert_eq!(retained.fingerprint(), restored.fingerprint());
        assert!(
            db.trace
                .counts
                .get("ReferenceHeader:Invalidated")
                .copied()
                .unwrap_or(0)
                >= 1
        );
    }
}
#[test]
fn unsupported_reference_shape_is_retained_without_lowering_a_cold_body() {
    let (mut db, definition) = world(
        "fn f(x:&[isize])->&[isize]{unsupported!()}fn main(){}",
        "f",
        2024,
    );
    let one = header(&mut db, definition);
    assert!(one.parameter(0).is_none());
    assert!(one.result().is_none());
    assert_eq!(one.input_references(), 1);
    assert!(Arc::ptr_eq(&one, &header(&mut db, definition)));
    assert!(
        !db.trace
            .events
            .iter()
            .any(|event| matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
    );
}
#[test]
fn retained_reference_plan_remains_charged_after_memo_retirement() {
    let (mut db, definition) = world(RESULT, "f", 2024);
    let plan = header(&mut db, definition);
    let key = QueryKey::ReferenceHeader(definition);
    let bytes = plan.metadata_bytes() + 2 * std::mem::size_of::<usize>();
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= bytes);
    db.memos.remove(&key);
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= bytes);
    let weak = Arc::downgrade(&plan);
    drop(plan);
    assert!(weak.upgrade().is_none());
    db.evict_between_runs(CacheBudgets {
        query_bytes: 0,
        parse_bytes: 0,
        ..CacheBudgets::default()
    })
    .unwrap();
    assert_eq!(db.cache_usage().unwrap().memo_bytes, 0);
}

fn pattern_world() -> (
    Database,
    Arc<crate::syntax::pattern_plan::PatternSourcePlan>,
) {
    let (mut db, definition) = world(
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/expr-match.rs"),
        "test_basic",
        2024,
    );
    let InputValue::DefinitionBody(body) = db.inputs[&InputKey::DefinitionBody(definition)]
        .value
        .as_ref()
    else {
        panic!()
    };
    let body = body.key();
    let outcome = db
        .fetch(
            &super::super::provider::SyntaxProvider,
            QueryKey::BodyStructure(body),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = outcome.value.as_ref() else {
        panic!()
    };
    let node = index
        .nodes()
        .iter()
        .find(|node| {
            node.role
                == crate::syntax::body_index::NodeRole::Expression(
                    crate::syntax::body_index::ExpressionKind::Match,
                )
        })
        .unwrap();
    let owner = index.pc(node.id, crate::ir::pc::Phase::Entry).unwrap();
    drop(outcome);
    let outcome = db
        .fetch(
            &super::super::provider::SyntaxProvider,
            QueryKey::PatternPlan(owner),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::PatternPlan(plan) = outcome.value.as_ref() else {
        panic!()
    };
    (db, Arc::clone(plan))
}
#[test]
fn retained_pattern_plan_accounts_its_parser_and_index_after_all_wrappers_retire() {
    let (mut db, plan) = pattern_world();
    let plan_bytes = plan.metadata_bytes() + 2 * std::mem::size_of::<usize>();
    let body_bytes = plan.index().estimated_bytes() + 2 * std::mem::size_of::<usize>();
    let parse_bytes = plan.parsed().estimated_bytes();
    db.memos.clear();
    let usage = db.cache_usage().unwrap();
    assert!(usage.pinned_memo_bytes >= plan_bytes + body_bytes);
    assert!(usage.pinned_parse_bytes >= parse_bytes);
    assert!(
        db.evict_between_runs(CacheBudgets {
            query_bytes: 0,
            parse_bytes: 0,
            ..CacheBudgets::default()
        })
        .is_err()
    );
    let parsed = Arc::downgrade(plan.parsed());
    let index = Arc::downgrade(plan.index());
    drop(plan);
    assert!(parsed.upgrade().is_none());
    assert!(index.upgrade().is_none());
    let eviction = db
        .evict_between_runs(CacheBudgets {
            query_bytes: 0,
            parse_bytes: 0,
            ..CacheBudgets::default()
        })
        .unwrap();
    assert_eq!(eviction.after.memo_bytes, 0);
    assert_eq!(eviction.after.parse_bytes, 0);
}
#[test]
fn cached_pattern_plan_accounts_nested_roots_without_dependency_wrappers_or_weak_rows() {
    let (mut db, plan) = pattern_world();
    let plan_bytes = plan.metadata_bytes() + 2 * std::mem::size_of::<usize>();
    let body_bytes = plan.index().estimated_bytes() + 2 * std::mem::size_of::<usize>();
    let parse_bytes = plan.parsed().estimated_bytes();
    let owner = plan.owner().clone();
    drop(plan);
    // Model ordinary memo eviction pruning unneeded weak rows while the
    // independently memoized structural plan still owns its immutable roots.
    db.memos
        .retain(|key, _| *key == QueryKey::PatternPlan(owner.clone()));
    db.query_pins.clear();
    let usage = db.cache_usage().unwrap();
    assert!(usage.memo_bytes >= plan_bytes + body_bytes);
    assert!(usage.parse_bytes >= parse_bytes);
    assert_eq!(usage.pinned_parse_bytes, 0);
    let eviction = db
        .evict_between_runs(CacheBudgets {
            query_bytes: 0,
            parse_bytes: 0,
            ..CacheBudgets::default()
        })
        .unwrap();
    assert_eq!(eviction.after.memo_bytes, 0);
    assert_eq!(eviction.after.parse_bytes, 0);
    assert_eq!(db.cache_usage().unwrap().memo_bytes, 0);
    assert_eq!(db.cache_usage().unwrap().parse_bytes, 0);
}

#[test]
fn source_plan_values_do_not_authorize_another_definition_owner_or_phase() {
    let (mut db, definition) = world(RESULT, "f", 2024);
    let plan = header(&mut db, definition);
    let value = QueryValue::ReferenceHeader(plan);
    assert!(value.matches(&QueryKey::ReferenceHeader(definition)));
    assert!(!value.matches(&QueryKey::ReferenceHeader(Digest([194; 32]))));
    let (mut db, plan) = pattern_world();
    let mut other = plan.owner().clone();
    other.phase = crate::ir::pc::Phase::Commit.code().unwrap();
    let value = QueryValue::PatternPlan(Arc::clone(&plan));
    assert!(!value.matches(&QueryKey::PatternPlan(other.clone())));
    let parse_runs = db.trace.counts.get("ParseFile:ProviderRun").copied();
    let body_runs = db.trace.counts.get("BodyStructure:ProviderRun").copied();
    assert_eq!(
        db.fetch(
            &super::super::provider::SyntaxProvider,
            QueryKey::PatternPlan(other)
        )
        .into_ready()
        .unwrap()
        .unwrap_err()
        .code,
        "LRR-E-QUERY-REQUEST"
    );
    assert_eq!(
        db.trace.counts.get("ParseFile:ProviderRun").copied(),
        parse_runs
    );
    assert_eq!(
        db.trace.counts.get("BodyStructure:ProviderRun").copied(),
        body_runs
    );
    let reference = QueryKey::ReferenceHeader(definition);
    let pattern = QueryKey::PatternPlan(plan.owner().clone());
    for key in [reference.clone(), pattern.clone()] {
        let encoded = serde_json::to_vec(&key).unwrap();
        let decoded: QueryKey = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(key, decoded);
        assert_eq!(key.fingerprint(), decoded.fingerprint());
    }
    assert_ne!(reference.fingerprint(), pattern.fingerprint());
}
