//! Genuine cold namespace authority and whole unchanged stock runtime witness.
use super::*;
use crate::{
    core::digest::StableEncode,
    db::{
        key::{BucketKey, CurrentModuleNameKey, QueryKey},
        poll::QueryPoll,
        provider::{Provided, Provider, SyntaxProvider},
        seals::SealDomain,
        value::QueryValue,
    },
    frontend::current_module_names::ModuleNameCompletion,
};
const ORIGINAL: &[u8] = include_bytes!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-to-subfield.rs"
);
fn path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-to-subfield.rs",
    )
}
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual Type preparation: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual Type preparation demand: {input:?}"),
    }
}
#[test]
fn actual_type_fixed_point_proves_absence_without_preparing_executable_bodies() {
    let original = path();
    assert_eq!(std::fs::read(&original).unwrap(), ORIGINAL);
    for edition in [2015, 2018, 2021, 2024] {
        let (prepared, view, sources, mut snapshot) =
            super::super::capture_standalone_edition(&original, false, edition).unwrap();
        let (compiler, prepared, view, sources) =
            PreparedCompileInputs::prepare(&original, prepared, view, sources).unwrap();
        let mut db = Database::default();
        prepared.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        db.trace.detailed = true;
        let discovery = prepare(&compiler, &mut db, &view, &sources, &mut snapshot, None).unwrap();
        assert_eq!(
            discovery.key().domain,
            SealDomain::Scope {
                id: view.outline().root_scope.fingerprint(),
                namespace: Namespace::Type
            }
        );
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::Thunk(_)
                | QueryKey::BodyStructure(_)
                | QueryKey::PatternPlan(_)
                | QueryKey::SourceLocalPlan(_)
                | QueryKey::ConstructorPlan(_)
                | QueryKey::TypeSchema(_)
                | QueryKey::Constant(_)
        )));
        let key = QueryKey::CurrentModuleName(Box::new(CurrentModuleNameKey {
            source: view.selection().outline.clone(),
            path: view.selection().path.as_str().to_owned(),
            bucket: BucketKey {
                scope: view.outline().root_scope.clone(),
                namespace: Namespace::Type,
                name: "isize".into(),
            },
        }));
        let result = ready(db.fetch(&SyntaxProvider, key.clone()));
        let QueryValue::CurrentModuleName(observation) = result.value.as_ref() else {
            panic!("actual Type module query variant differs")
        };
        assert_eq!(observation.completion(), ModuleNameCompletion::Complete);
        assert_eq!(
            observation.lookup(),
            Some(&crate::frontend::names::NameLookup::Absent)
        );
        assert!(
            matches!(observation.certificate_dependency(),Some(crate::db::dependency::DependencyStamp::Input{key:InputKey::DiscoverySeal(key),..}) if key.domain==discovery.key().domain)
        );
        db.begin_expansion_epoch(2).unwrap();
        let result = ready(db.fetch(&SyntaxProvider, key));
        let QueryValue::CurrentModuleName(observation) = result.value.as_ref() else {
            panic!()
        };
        assert_eq!(observation.completion(), ModuleNameCompletion::Unsealed);
        assert_eq!(observation.lookup(), None);
    }
    assert_eq!(std::fs::read(&original).unwrap(), ORIGINAL);
}
#[test]
fn type_preparation_rejects_run_and_query_before_cold_capture() {
    let original = path();
    let (prepared, view, sources, mut snapshot) =
        super::super::capture_standalone_edition(&original, false, 2024).unwrap();
    let (compiler, prepared, view, sources) =
        PreparedCompileInputs::prepare(&original, prepared, view, sources).unwrap();
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let revision = db.revision();
    db.begin_run().unwrap();
    match prepare(&compiler, &mut db, &view, &sources, &mut snapshot, None) {
        Err(error) => assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT"),
        Ok(_) => panic!("active run acquired Type preparation"),
    }
    db.finish_run().unwrap();
    assert_eq!(db.revision(), revision);
    struct DuringQuery<'a> {
        compiler: &'a PreparedCompileInputs,
        root: &'a PreparedSourceView,
        sources: &'a [PreparedSourceView],
        snapshot: Arc<Snapshot>,
    }
    impl Provider for DuringQuery<'_> {
        fn execute(&self, db: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
            let mut snapshot = Arc::clone(&self.snapshot);
            match prepare(self.compiler, db, self.root, self.sources, &mut snapshot, None) {
                Err(error) => QueryPoll::Ready(Err(error)),
                Ok(_) => panic!("query acquired Type preparation"),
            }
        }
    }
    match db.fetch(
        &DuringQuery {
            compiler: &compiler,
            root: &view,
            sources: &sources,
            snapshot,
        },
        QueryKey::Signature(crate::core::digest::Digest([247; 32])),
    ) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT"),
        other => panic!("query Type preparation barrier changed: {other:?}"),
    }
    assert_eq!(db.revision(), revision);
}
#[test]
fn unchanged_local_record_main_uses_ordinary_construct_project_and_assign_all_editions() {
    let original = path();
    assert_eq!(std::fs::read(&original).unwrap(), ORIGINAL);
    for edition in [2015, 2018, 2021, 2024] {
        let report = match super::super::standalone_edition(&original, None, edition) {
            Ok(super::super::StandaloneOutcome::Completed(report)) => report,
            Ok(_) => panic!("unchanged local record main did not complete: edition={edition}"),
            Err(error) => {
                panic!("unchanged local record main failed: edition={edition}, {error:?}")
            }
        };
        assert_eq!(report.status, "completed");
        assert!(matches!(
            report.value,
            crate::engine::run::ValueSummary::Unit
        ));
        assert!(
            report.opcode_counters.started_by_opcode[27] > 0,
            "ordinary constructor start, edition={edition}"
        );
        assert!(
            report.opcode_counters.started_by_opcode[29] > 0,
            "ordinary constructor finish, edition={edition}"
        );
        assert!(
            report.opcode_counters.started_by_opcode[37] > 0,
            "ordinary field projection, edition={edition}"
        );
        assert_eq!(
            report.opcode_counters.started_by_opcode[34], 2,
            "both unchanged assignments execute exactly once, edition={edition}"
        );
        assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert!(report.opcode_counters.return_scopes_cleaned > 0);
    }
    assert_eq!(std::fs::read(&original).unwrap(), ORIGINAL);
}

#[test]
fn unchanged_local_annotated_impl_main_executes_through_ordinary_entry_all_editions() {
    let original = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../fixtures/upstream/rustc-checkout/tests/ui/coherence/coherence-impl-in-fn.rs");
    let bytes = include_bytes!("../../../../fixtures/upstream/rustc-checkout/tests/ui/coherence/coherence-impl-in-fn.rs");
    assert_eq!(std::fs::read(&original).unwrap(), bytes);
    for edition in [2015, 2018, 2021, 2024] {
        let report = match super::super::standalone_edition(&original, None, edition) {
            Ok(super::super::StandaloneOutcome::Completed(report)) => report,
            Ok(_) => panic!("unchanged annotated impl main did not complete: edition={edition}"),
            Err(error) => panic!("unchanged annotated impl main failed: edition={edition}, {error:?}"),
        };
        assert_eq!(report.status, "completed");
        assert!(matches!(report.value, crate::engine::run::ValueSummary::Unit));
        assert_eq!(report.opcode_counters.operations_started, report.opcode_counters.operations_completed);
        eprintln!("unchanged annotated local impl main edition{edition}: complete through ordinary interpreter");
    }
    assert_eq!(std::fs::read(&original).unwrap(), bytes);
}

#[test]
fn unchanged_local_annotation_receipts_bind_actual_headers_nodes_and_generations_all_editions() {
    use crate::{frontend::callables::CallSelection, ir::{op::Op, pc::{NodeId, Phase}}};
    let original = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../fixtures/upstream/rustc-checkout/tests/ui/coherence/coherence-impl-in-fn.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, context_weak) = {
        let (inputs, view, sources, mut snapshot) =
            super::super::capture_standalone_edition(&original, false, edition).unwrap();
        let (compiler, inputs, view, sources) = PreparedCompileInputs::prepare(
            &original, inputs, view, sources).unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap(); db.begin_expansion_epoch(1).unwrap();
        prepare(&compiler, &mut db, &view, &sources, &mut snapshot, None).unwrap();
        let mut discovery = SourceDiscovery::new_deferred_imports(sources,
            view.outline().root_scope.clone(), Namespace::Value, view.selection().outline.crate_key, 1).unwrap()
            .with_declarative_compiler(compiler.environment_identity(),
                crate::expand::source_declarative::engine_version(), compiler.catalog.clone()).unwrap();
        compiler.discover_diagnostic(&mut discovery, &mut db, &mut snapshot).unwrap();
        discovery.publish(&mut db).unwrap();
        let CallSelection::Source(main) = ready(discovery.entry_function(&mut db, "main")) else { panic!() };
        let main = crate::cli::retained_entry::prepare(&mut db, &discovery, &view, "main", main).unwrap();
        let body_key = main.body().unwrap().key();
        let plan = ready(db.prepare_source_local_plan(&discovery, &[&view], &body_key));
        let context = plan.context(); let source = context.source();
        assert!(Arc::ptr_eq(plan.environment().unwrap().prepared(), context));
        let row = context.facts().items.iter().find(|row| row.attribute_input.is_some()).unwrap();
        let names = plan.declaration_name_completions();
        names.require_local_declaration(source.parsed(), source.body(), source.scope(), row).unwrap();
        let mut wrong = row.clone(); wrong.item.header.tokens.clear();
        assert!(names.require_local_declaration(source.parsed(), source.body(), source.scope(), &wrong).is_err());
        let mut scope = source.scope().clone(); scope.owner = Some(crate::core::digest::Digest([0;32]));
        assert!(names.require_local_declaration(source.parsed(), source.body(), &scope, row).is_err());
        let mut body = source.body().clone(); body.def_key = crate::core::digest::Digest([0;32]);
        assert!(names.require_local_declaration(source.parsed(), &body, source.scope(), row).is_err());
        let index = source.index();
        let node = index.nodes().iter().find(|node|
            node.role == crate::syntax::body_index::NodeRole::OpaqueItem
                && source.body().range.start + node.range.start == row.item.declaration_span.start
                && source.body().range.start + node.range.end == row.item.declaration_span.end).unwrap();
        let pc = index.pc(node.id, Phase::Entry).unwrap();
        let admission = names.admit_node(context, index, &pc).unwrap().unwrap();
        admission.require(source.parsed(), source.body(), index, &pc).unwrap();
        admission.require_published(&mut db).unwrap();
        let other = index.pc(NodeId(0), Phase::Entry).unwrap();
        assert!(admission.require(source.parsed(), source.body(), index, &other).is_err());
        let original_parser = view.retained_declaration_parser().unwrap();
        assert!(admission.require(&original_parser, source.body(), index, &pc).is_err());
        let start = db.trace.events.len();
        let recipe = ready(db.fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone())));
        let QueryValue::Thunk(recipe) = recipe.value.as_ref() else { panic!() };
        assert!(matches!(recipe.recipe().operations.as_ref(), [Op::Declaration]));
        assert!(db.trace.events[start..].iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) => body == &body_key,
            QueryKey::Thunk(pc) => pc.body == body_key,
            _ => true,
        }));
        let environment = ready(discovery.completed_source_environment(&mut db));
        let members = environment.member_index().unwrap();
        let (units, boundaries) = environment.generated_publication_for_control();
        assert_eq!(units.len(), 2);
        environment.admit_generated_units(units.clone(), boundaries.clone()).unwrap();
        let mut changed = boundaries.clone(); changed.pop_first();
        assert!(environment.admit_generated_units(units.clone(), changed).is_err());
        assert!(environment.admit_generated_units(Vec::new(), boundaries.clone()).is_err());
        environment.admit_generated_units(units, boundaries).unwrap();
        assert!(Arc::ptr_eq(&members, &environment.member_index().unwrap()));
        discovery.publish(&mut db).unwrap();
        let again = ready(db.prepare_source_local_plan(&discovery, &[&view], &body_key));
        assert!(Arc::ptr_eq(&plan, &again));
        db.begin_expansion_epoch(2).unwrap();
        assert!(!matches!(db.fetch(&SyntaxProvider, QueryKey::Thunk(pc)), QueryPoll::Ready(Ok(_))),
            "a cached declaration recipe cannot outlive its actual epoch");
        eprintln!("entered annotated declaration edition{edition}: ordinary Op::Declaration; exact header/node/parser/epoch and generation refusals; immutable context/plan reuse");
        (db, Arc::downgrade(context))
        };
        db.publish_inputs(Default::default()).unwrap();
        db.evict_between_runs(crate::engine::telemetry::CacheBudgets::default()).unwrap();
        assert!(context_weak.upgrade().is_none(), "revoked annotation contexts must retire without a source-plan ownership cycle");

    }
}
