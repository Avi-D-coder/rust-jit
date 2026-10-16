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
        // publish_inputs expands ScopeMembers into its granular rows. Its
        // complete input protocol must not receive those rows a second time.
        .filter(|(key, _)| match key {
            InputKey::NameCandidates(bucket) => !db
                .inputs
                .contains_key(&InputKey::ScopeMembers(bucket.scope.clone())),
            InputKey::ScopeBlockers(scope) => !db
                .inputs
                .contains_key(&InputKey::ScopeMembers(scope.clone())),
            _ => true,
        })
        .map(|(key, record)| (key.clone(), record.value.as_ref().clone()))
        .collect()
}

fn input_floor_budget(db: &Database) -> CacheBudgets {
    CacheBudgets {
        // Published source inputs are mandatory pins. A zero query cap cannot
        // retain this world, even after its last query result is reclaimed.
        query_bytes: db.cache_usage().unwrap().pinned_memo_bytes,
        parse_bytes: 0,
        ..CacheBudgets::default()
    }
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
            let InputValue::ScopeMembers(members) =
                inputs.get_mut(&InputKey::ScopeMembers(scope)).unwrap()
            else {
                panic!("owning scope members absent")
            };
            members.candidates.insert(
                (
                    crate::source::manifest::Namespace::Type,
                    "unrelated_type".into(),
                ),
                vec![Digest([192; 32])],
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
    for changed in 0..5 {
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
            InputValue::DefinitionBody(body) if changed == 3 => body.range.start += 1,
            InputValue::DefinitionBody(body) => body.range.end -= 1,
            _ => panic!(),
        }
        db.publish_inputs(next).unwrap();
        let rejected = db
            .fetch(
                &super::super::provider::SyntaxProvider,
                QueryKey::ReferenceHeader(definition),
            )
            .into_ready()
            .unwrap();
        // A pure query may retain its typed diagnostic as QueryValue::Error.
        // Neither that representation nor a transport diagnostic is a plan.
        let diagnostic = match rejected {
            Err(error) => error,
            Ok(outcome) => match outcome.value.as_ref() {
                QueryValue::Error(error) => error.clone(),
                other => panic!("changed input {changed} unexpectedly admitted {other:?}"),
            },
        };
        assert_eq!(
            diagnostic.code, "LRR-E-CALL-SIGNATURE",
            "changed input {changed}"
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
    let budget = input_floor_budget(&db);
    db.evict_between_runs(budget).unwrap();
    let after = db.cache_usage().unwrap();
    assert!(db.memos.is_empty());
    assert_eq!(after.memo_entries, 0);
    assert_eq!(after.memo_bytes, after.pinned_memo_bytes);
    assert_eq!(after.parse_bytes, 0);
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
    // Product retirement leases defer final index destruction to this boundary.
    assert!(index.upgrade().is_some());
    let budget = input_floor_budget(&db);
    let eviction = db.evict_between_runs(budget).unwrap();
    assert!(index.upgrade().is_none());
    assert!(db.memos.is_empty());
    assert_eq!(eviction.after.memo_entries, 0);
    assert_eq!(eviction.after.memo_bytes, eviction.after.pinned_memo_bytes);
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
    let budget = input_floor_budget(&db);
    let eviction = db.evict_between_runs(budget).unwrap();
    assert!(db.memos.is_empty());
    assert_eq!(eviction.after.memo_entries, 0);
    assert_eq!(eviction.after.memo_bytes, eviction.after.pinned_memo_bytes);
    assert_eq!(eviction.after.parse_bytes, 0);
    let after = db.cache_usage().unwrap();
    assert_eq!(after.memo_bytes, after.pinned_memo_bytes);
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

#[test]
fn externally_retained_product_pins_its_owner_without_cloning_the_index() {
    let (mut db, plan) = pattern_world();
    let key = QueryKey::PatternPlan(plan.owner().clone());
    drop(plan);
    let value = &db.memos[&key].value;
    let QueryValue::PatternPlan(plan) = value.as_ref() else {
        panic!()
    };
    let (outer, inner) = db.memo_owners();
    assert!(!QueryPin::new(value).external(&outer, &inner));
    let before = Arc::strong_count(plan.index());
    let product = Arc::clone(plan.products().next().expect("actual source product"));
    assert_eq!(Arc::strong_count(plan.index()), before);
    assert!(QueryPin::new(value).external(&outer, &inner));
    let weak = Arc::downgrade(&product);
    assert!(
        db.evict_between_runs(CacheBudgets {
            query_bytes: 0,
            parse_bytes: 0,
            ..CacheBudgets::default()
        })
        .is_err()
    );
    assert!(db.memos.contains_key(&key));
    drop(product);
    // The source-plan memo still retains the exact same allocation.
    assert!(weak.upgrade().is_some());
    let value = &db.memos[&key].value;
    let (outer, inner) = db.memo_owners();
    assert!(!QueryPin::new(value).external(&outer, &inner));
}

#[test]
fn product_only_lease_is_charged_after_outer_plan_and_body_memos_retire() {
    let (mut db, plan) = pattern_world();
    let product = Arc::clone(plan.products().next().expect("actual source product"));
    let product_bytes = product.metadata_bytes() + 2 * std::mem::size_of::<usize>();
    let index_bytes = product.index().estimated_bytes() + 2 * std::mem::size_of::<usize>();
    let weak = Arc::downgrade(&product);
    let index = Arc::downgrade(product.index());
    drop(plan);
    db.memos.clear();
    let usage = db.cache_usage().unwrap();
    assert!(usage.pinned_memo_bytes >= product_bytes + index_bytes);
    // Published-owner-only input pins and dead Signature/ParseFile weak rows
    // remain reclaimable until between-run pruning. Their registry overhead
    // need not be pinned; the actual retained graph and index must be.
    assert!(
        db.evict_between_runs(CacheBudgets {
            query_bytes: product_bytes + index_bytes - 1,
            ..CacheBudgets::default()
        })
        .is_err()
    );
    drop(product);
    // Retirement lease defers the final graph/index destructor until the
    // between-run coordinator, even after every source memo was invalidated.
    assert!(weak.upgrade().is_some());
    assert!(index.upgrade().is_some());
    let budget = input_floor_budget(&db);
    let eviction = db.evict_between_runs(budget).unwrap();
    assert!(weak.upgrade().is_none());
    assert!(index.upgrade().is_none());
    assert_eq!(
        eviction.after.memo_bytes,
        db.cache_usage().unwrap().memo_bytes
    );
}

#[test]
fn cached_product_roots_have_no_false_external_index_owner_and_evict_exactly() {
    let (mut db, plan) = pattern_world();
    let owner = plan.owner().clone();
    let products: Vec<_> = plan.products().map(Arc::downgrade).collect();
    assert!(!products.is_empty());
    drop(plan);
    db.memos
        .retain(|key, _| *key == QueryKey::PatternPlan(owner.clone()));
    let (outer, inner) = db.memo_owners();
    assert!(
        !QueryPin::new(&db.memos[&QueryKey::PatternPlan(owner)].value).external(&outer, &inner)
    );
    let budget = input_floor_budget(&db);
    let eviction = db.evict_between_runs(budget).unwrap();
    assert!(db.memos.is_empty());
    assert!(products.iter().all(|product| product.upgrade().is_none()));
    let actual = db.cache_usage().unwrap();
    assert_eq!(eviction.after.memo_bytes, actual.memo_bytes);
    assert_eq!(eviction.after.parse_bytes, actual.parse_bytes);
    assert_eq!(actual.memo_bytes, actual.pinned_memo_bytes);
}

#[test]
fn retirement_only_product_pins_an_independently_cached_body_allocation() {
    let (mut db, plan) = pattern_world();
    let key = QueryKey::BodyStructure(plan.index().body().clone());
    let body_bytes = plan.index().estimated_bytes() + 2 * std::mem::size_of::<usize>();
    let product_bytes: usize = plan
        .products()
        .map(|product| product.metadata_bytes() + 2 * std::mem::size_of::<usize>())
        .sum();
    let products: Vec<_> = plan.products().map(Arc::downgrade).collect();
    drop(plan);
    db.memos.retain(|candidate, _| *candidate == key);
    db.query_pins.clear();
    assert_eq!(db.memos.len(), 1);
    let usage = db.cache_usage().unwrap();
    assert!(usage.pinned_memo_bytes >= body_bytes + product_bytes);
    let budget = input_floor_budget(&db);
    let eviction = db.evict_between_runs(budget).unwrap();
    assert!(products.iter().all(|product| product.upgrade().is_none()));
    assert_eq!(
        eviction.after.memo_bytes,
        db.cache_usage().unwrap().memo_bytes
    );
}

#[test]
fn external_source_plan_keeps_graph_retirement_lease_across_quiescent_pruning() {
    let (mut db, plan) = pattern_world();
    let graph = plan
        .products()
        .next()
        .expect("actual retained source graph");
    let weak = Arc::downgrade(graph);
    // Retain the parent allocation only; do not clone its nested graph handle.
    db.evict_between_runs(CacheBudgets::default()).unwrap();
    db.memos.clear();
    db.begin_run().unwrap();
    drop(plan);
    assert!(
        weak.upgrade().is_some(),
        "graph retired during an active run"
    );
    db.finish_run().unwrap();
    db.evict_between_runs(CacheBudgets::default()).unwrap();
    assert!(
        weak.upgrade().is_none(),
        "quiescent retirement leaked its graph"
    );
}

fn string_literal_world(
    edition: u16,
) -> (
    Database,
    QueryKey,
    Arc<crate::syntax::string_literal::PreparedStringLiteral>,
) {
    let (mut db, definition) = world(
        include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/match/match-borrowed-str.rs"
        ),
        "compare",
        edition,
    );
    let InputValue::DefinitionBody(body) = db.inputs[&InputKey::DefinitionBody(definition)]
        .value
        .as_ref()
    else {
        panic!("upstream compare body absent")
    };
    let body = body.key();
    let indexed = db
        .fetch(
            &super::super::provider::SyntaxProvider,
            QueryKey::BodyStructure(body),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!("upstream compare structural index absent")
    };
    let literal = index
        .nodes()
        .iter()
        .find(|node| {
            node.role
                == crate::syntax::body_index::NodeRole::Expression(
                    crate::syntax::body_index::ExpressionKind::Literal,
                )
                // The earlier written string belongs to a match pattern.
                // Select the actual comparison operand, not that endpoint.
                && node.parent.is_some_and(|parent| index.nodes()[parent.0 as usize].role
                    == crate::syntax::body_index::NodeRole::Expression(
                        crate::syntax::body_index::ExpressionKind::Other))
        })
        .unwrap();
    let request = QueryKey::Thunk(index.pc(literal.id, crate::ir::pc::Phase::Entry).unwrap());
    drop(indexed);
    let prepared = db
        .fetch(&super::super::provider::SyntaxProvider, request.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::Thunk(recipe) = prepared.value.as_ref() else {
        panic!("upstream literal did not use an ordinary verified recipe")
    };
    let proof = Arc::clone(recipe.string_literal_roots().next().unwrap());
    assert!(recipe.string_literal_admission_bytes() >= proof.metadata_bytes().unwrap());
    drop(prepared);
    (db, request, proof)
}

#[test]
fn immutable_literal_inner_aliases_share_accounting_and_retire_between_runs_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, _, proof) = string_literal_world(edition);
        let before = db.cache_usage().unwrap();
        let alias = Arc::clone(&proof);
        assert_eq!(db.cache_usage().unwrap().memo_bytes, before.memo_bytes);
        assert_eq!(
            db.cache_usage().unwrap().pinned_memo_bytes,
            before.pinned_memo_bytes
        );
        drop(alias);
        let required = proof.metadata_bytes().unwrap();
        db.memos.clear();
        assert!(db.cache_usage().unwrap().pinned_memo_bytes >= required);
        let weak = Arc::downgrade(&proof);
        db.begin_run().unwrap();
        drop(proof);
        assert!(
            weak.upgrade().is_some(),
            "literal receipt retired in an active run"
        );
        db.finish_run().unwrap();
        db.evict_between_runs(CacheBudgets::default()).unwrap();
        assert!(
            weak.upgrade().is_none(),
            "literal retirement pool leaked its receipt"
        );
    }
}

#[test]
fn cached_only_literal_receipt_can_reclaim_to_mandatory_input_budget() {
    let (mut db, request, proof) = string_literal_world(2015);
    let weak = Arc::downgrade(&proof);
    drop(proof);
    let budget = input_floor_budget(&db);
    let report = db.evict_between_runs(budget).unwrap();
    assert!(report.victims.contains(&request));
    assert!(weak.upgrade().is_none());
    assert!(report.after.memo_bytes <= budget.query_bytes);
    assert_eq!(report.after.parse_bytes, 0);
}

#[test]
fn rejected_literal_retention_cannot_publish_a_successful_memo() {
    let (mut db, request, proof) = string_literal_world(2015);
    drop(proof);
    db.memos.clear();
    db.evict_between_runs(CacheBudgets::default()).unwrap();
    assert_eq!(db.source_plan_pins.registered_artifact_count(), 0);
    // Exercise the real registry preflight with a reduced count bound. Source
    // admission, the pinned parser and ordinary provider remain unchanged.
    db.source_plan_pins.set_test_artifact_limit(0);
    let failure = match db.fetch(&super::super::provider::SyntaxProvider, request.clone()) {
        QueryPoll::Ready(Err(error)) => error,
        _ => panic!("literal registry bound must reject admission"),
    };
    assert_eq!(failure.code, "LRR-E-METADATA-LIMIT");
    assert!(db.memo(&request).is_none(), "rejected recipe was published");
    assert_eq!(db.source_plan_pins.registered_artifact_count(), 0);

    db.source_plan_pins.set_test_artifact_limit(usize::MAX);
    let outcome = db
        .fetch(&super::super::provider::SyntaxProvider, request.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    assert!(matches!(outcome.value.as_ref(), QueryValue::Thunk(_)));
    assert!(db.memo(&request).is_some());
    assert_eq!(db.source_plan_pins.registered_artifact_count(), 1);
}
