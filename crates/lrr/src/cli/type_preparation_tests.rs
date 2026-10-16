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
        let discovery = prepare(&compiler, &mut db, &view, &sources, &mut snapshot).unwrap();
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
    match prepare(&compiler, &mut db, &view, &sources, &mut snapshot) {
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
            match prepare(self.compiler, db, self.root, self.sources, &mut snapshot) {
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
