//! Owning environment/query controls over complete unchanged rustc sources.
//! These inspect common preparation and authority, not a whole-main runtime pass.
use super::*;
use crate::core::digest::StableEncode;
use crate::{
    db::{provider::SyntaxProvider, value::QueryValue},
    engine::telemetry::CacheBudgets,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::{
        compile_input_catalog::CompileInputCatalog,
        source_body_context::type_occurrences::BodyTypeSegment,
        source_locals::PreparedSourceLocalPlan,
        source_locals::owned_plan::type_declarations::BodyTypeOutcome,
    },
    source::{changes::PreparedSourceView, manifest::Namespace},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-to-subfield.rs"
);
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual owning environment: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual owning environment demand: {input:?}"),
    }
}
fn world(text: &str, edition: u16) -> (Database, PreparedSourceView, SourceDiscovery) {
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(text, edition);
    let source = provider.module_source().share();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.trace.detailed = true;
    let discovery = complete(&mut db, &source);
    (db, source, discovery)
}
fn complete(db: &mut Database, source: &PreparedSourceView) -> SourceDiscovery {
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Value,
        Digest::of_bytes(b"owning body environment source controls"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"owning body environment compiler controls"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => {
                discovery.publish(db).unwrap();
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!("actual owning discovery incomplete: {diagnostic:?} {blocked:?}"),
            SourceDiscoveryStep::NeedSourceDeriveSelection { .. } => {
                panic!("actual NeedSourceDeriveSelection")
            }
            SourceDiscoveryStep::NeedSourceDeriveExpansion { .. } => {
                panic!("actual NeedSourceDeriveExpansion")
            }
            SourceDiscoveryStep::NeedSourceDeriveAppend { .. } => {
                panic!("actual NeedSourceDeriveAppend")
            }
            SourceDiscoveryStep::NeedDeriveAppend { .. } => panic!("actual NeedDeriveAppend"),
            SourceDiscoveryStep::NeedDeriveDeclaration { .. } => {
                panic!("actual NeedDeriveDeclaration")
            }
            SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("actual NeedAttributeSelection")
            }
            SourceDiscoveryStep::NeedProceduralSelection { .. } => {
                panic!("actual NeedProceduralSelection")
            }
        }
    }
    discovery
}
fn main_body(db: &mut Database, discovery: &SourceDiscovery) -> BodyKey {
    let selected = ready(discovery.entry_function(db, "main"));
    let crate::frontend::callables::CallSelection::Source(main) = selected else {
        panic!("actual official main did not select source")
    };
    main.body().expect("official main body").key()
}
fn selected_local_types(db: &mut Database, plan: &Arc<PreparedSourceLocalPlan>) {
    let mut count = 0;
    for node in plan.context().source().index().nodes() {
        let Some(occurrence) = ready(plan.context().body_type_occurrence(db, node.id)) else {
            continue;
        };
        let [BodyTypeSegment::Name(name)] = occurrence.segments() else {
            continue;
        };
        let Some(row) = plan.context().facts().items.iter().find(|row| {
            row.item.def_key.namespace == Namespace::Type
                && row.item.def_key.declared_name == name.name()
        }) else {
            continue;
        };
        let definition = row.item.def_key.fingerprint();
        ready(db.publish_local_declaration(plan.context(), definition));
        match ready(plan.prepare_body_type(db, node.id)) {
            BodyTypeOutcome::Ready(proof) => {
                assert_eq!(proof.declaration().definition_identity(), definition);
                ready(proof.declaration().require_current(db));
                assert!(Arc::ptr_eq(proof.declaration().retained_plan(), plan));
            }
            BodyTypeOutcome::Pending(demand) => {
                panic!("actual published local Type pending: {demand:?}")
            }
        }
        count += 1;
    }
    assert_eq!(
        count, 2,
        "complete unchanged source has A and B constructor uses"
    );
}
#[test]
fn unchanged_local_types_use_one_current_published_plan_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source, discovery) = world(ORIGINAL, edition);
        let body = main_body(&mut db, &discovery);
        match db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(body.clone())) {
            QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-BODY-ENVIRONMENT"),
            other => panic!("unpublished body acquired a local environment: {other:?}"),
        }
        let plan = ready(db.prepare_source_local_plan(&discovery, &[&source], &body));
        assert!(plan.environment().is_some());
        selected_local_types(&mut db, &plan);
        let revision = db.revision();
        let from = db.trace.events.len();
        let again = ready(db.prepare_source_local_plan(&discovery, &[&source], &body));
        assert!(
            Arc::ptr_eq(&plan, &again),
            "common memo must retain the actual immutable plan"
        );
        assert_eq!(revision, db.revision());
        assert!(
            !db.trace.events[from..]
                .iter()
                .any(|event| matches!(event.key, QueryKey::Thunk(_) | QueryKey::Constant(_)))
        );
        // Source-only original local facts survive epoch changes, but THIS
        // published environment also owns an actual discovery completion lease.
        db.begin_expansion_epoch(2).unwrap();
        match plan.require_current(&mut db) {
            QueryPoll::Ready(Err(_)) => {}
            other => panic!("obsolete completed environment remained current: {other:?}"),
        }
    }
}
#[test]
fn generated_and_wrapper_bodies_use_actual_boundaries_in_the_same_query() {
    for edition in [2015, 2018, 2021, 2024] {
        for wrapper in [false, true] {
            let text = format!(
                "macro_rules! emit{{()=>{{{ORIGINAL}}}}}{}",
                if wrapper {
                    "macro_rules! wrap{()=>{emit!();}}wrap!();"
                } else {
                    "emit!();"
                }
            );
            let (mut db, source, discovery) = world(&text, edition);
            let body = main_body(&mut db, &discovery);
            let plan = ready(db.prepare_source_local_plan(&discovery, &[&source], &body));
            assert!(plan.context().source().generated_boundary().is_some());
            selected_local_types(&mut db, &plan);
            db.begin_expansion_epoch(2).unwrap();
            match plan.require_current(&mut db) {
                QueryPoll::Ready(Err(_)) => {}
                other => panic!("stale generated owning environment admitted: {other:?}"),
            }
        }
    }
}
#[test]
fn owning_environment_publication_is_atomic_and_retired_inner_roots_stay_charged() {
    let (mut db, source, discovery) = world(ORIGINAL, 2024);
    let body = main_body(&mut db, &discovery);
    let plan = ready(db.prepare_source_local_plan(&discovery, &[&source], &body));
    let environment = Arc::clone(plan.environment().unwrap());
    let old_revision = db.revision();
    db.begin_run().unwrap();
    match db.publish_body_environment(
        Arc::clone(environment.source()),
        Arc::clone(environment.macros()),
    ) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT"),
        other => panic!("run published an owning environment: {other:?}"),
    }
    assert_eq!(old_revision, db.revision());
    db.finish_run().unwrap();
    // Revoke the genuine complete input universe, not a cloned collection of
    // derived rows. The external inner plan/receipt remain actual allocations.
    db.publish_inputs(BTreeMap::new()).unwrap();
    match db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(body)) {
        QueryPoll::Ready(Err(_)) => {}
        other => panic!("revoked environment query admitted: {other:?}"),
    }
    match plan.require_current(&mut db) {
        QueryPoll::Ready(Err(_)) => {}
        other => panic!("revoked inner environment remained current: {other:?}"),
    }
    let retained = db.body_environment_root_bytes(None).unwrap();
    assert!(retained > environment.exclusive_metadata_bytes());
    assert!(db.cache_usage().unwrap().pinned_memo_bytes >= retained);
    let zero = CacheBudgets {
        query_bytes: 0,
        parse_bytes: 0,
        source_input_bytes: 0,
    };
    assert!(db.evict_between_runs(zero).is_err());
    drop(plan);
    drop(environment);
    drop(source);
    drop(discovery);
    // Existing generic query/input/source weak pins retire on the coordinator.
    // This specifically asserts release of the new owned environment graph.
    db.evict_between_runs(CacheBudgets::default()).unwrap();
    assert_eq!(db.body_environment_root_bytes(None).unwrap(), 0);
}

#[test]
fn same_body_and_seal_key_remint_after_genuine_source_environment_replacement() {
    let (mut db, source, discovery) = world(ORIGINAL, 2024);
    let body = main_body(&mut db, &discovery);
    let old = ready(db.prepare_source_local_plan(&discovery, &[&source], &body));
    let old_envelope = old.environment().unwrap().identity();
    let text = format!("{ORIGINAL}\nmod unrelated {{ pub struct Additional; }}");
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(&text, 2024);
    let current = provider.module_source().share();
    inputs.publish(&mut db).unwrap();
    // Actual complete input replacement removes derived environments. Reinstall
    // the old derived row only through its original public coordinator API is
    // forbidden by its source guard; no test mutator fabricates the input.
    let next = complete(&mut db, &current);
    assert_eq!(next.key(), discovery.key());
    assert_eq!(body, main_body(&mut db, &next));
    let plan = ready(db.prepare_source_local_plan(&next, &[&current], &body));
    assert_ne!(plan.environment().unwrap().identity(), old_envelope);
    ready(plan.require_current(&mut db));
    match old.require_current(&mut db) {
        QueryPoll::Ready(Err(_)) => {}
        other => panic!("replaced owning source environment remained current: {other:?}"),
    }
    selected_local_types(&mut db, &plan);
}

#[test]
fn budget_and_pure_query_cannot_publish_a_partial_owning_environment() {
    let (mut db, view, discovery) = world(ORIGINAL, 2024);
    let body = main_body(&mut db, &discovery);
    let indexed = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.clone())));
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!()
    };
    let pc = index
        .pc(crate::ir::pc::NodeId(0), crate::ir::pc::Phase::Entry)
        .unwrap();
    let admitted = Arc::new(ready(crate::frontend::source_body_context::capture(
        &mut db,
        &[&view],
        index,
        &pc,
    )));
    let macros = ready(discovery.sealed_body_macro_environment(&mut db, view.selection()));
    let revision = db.revision();
    db.run_budgets = Some(CacheBudgets {
        query_bytes: 0,
        ..CacheBudgets::default()
    });
    match db.publish_body_environment(Arc::clone(&admitted), Arc::clone(&macros)) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-METADATA-LIMIT"),
        other => panic!("zero budget published actual owning environment: {other:?}"),
    }
    assert!(
        !db.inputs
            .contains_key(&InputKey::BodyEnvironment(body.clone()))
    );
    assert_eq!(db.revision(), revision);
    db.run_budgets = None;
    struct DuringQuery {
        source: Arc<AdmittedBodyContext>,
        macros: Arc<SealedBodyMacroEnvironment>,
    }
    impl crate::db::provider::Provider for DuringQuery {
        fn execute(
            &self,
            db: &mut Database,
            _: &QueryKey,
        ) -> QueryPoll<crate::db::provider::Provided> {
            match db.publish_body_environment(Arc::clone(&self.source), Arc::clone(&self.macros)) {
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                _ => panic!("pure query unexpectedly published owning environment"),
            }
        }
    }
    match db.fetch(
        &DuringQuery {
            source: admitted,
            macros,
        },
        QueryKey::Signature(Digest([245; 32])),
    ) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT"),
        other => panic!("query publication barrier changed: {other:?}"),
    }
    assert!(!db.inputs.contains_key(&InputKey::BodyEnvironment(body)));
    assert_eq!(db.revision(), revision);
}
