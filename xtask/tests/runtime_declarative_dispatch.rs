//! Actual literal expansion dispatch in unchanged stock bodies; later macro frontier remains.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        dependencies::SourceDependency,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
const ROOT2015: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/edition-keywords-2015-2015.rs"
);
const ROOT2018: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/edition-keywords-2018-2015.rs"
);
const AUX2015: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/editions/auxiliary/edition-kw-macro-2015.rs"
);
fn prepare(
    year: u16,
    text: &str,
) -> (
    Vec<lrr::source::changes::PreparedSourceView>,
    SourceDiscovery,
    PreparedSourceInputs,
) {
    let mut root_profile = ProfileIdentity::current();
    root_profile.edition = year;
    let mut aux_profile = ProfileIdentity::current();
    aux_profile.edition = 2015;
    let units = vec![
        SourceUnitInput {
            path: SourcePath::new("caller.rs").unwrap(),
            crate_key: Digest([2; 32]),
            module_path: vec![],
            profile: root_profile,
            cfg: CfgSet::guest_default(),
            blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        },
        SourceUnitInput {
            path: SourcePath::new("edition-kw-macro-2015.rs").unwrap(),
            crate_key: Digest([1; 32]),
            module_path: vec![],
            profile: aux_profile,
            cfg: CfgSet::guest_default(),
            blob: SourceBlob::new(AUX2015.as_bytes().to_vec()).unwrap(),
        },
    ];
    let selections = units
        .iter()
        .map(|unit| SourceSelection {
            path: unit.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: unit.blob.content(),
                    profile: unit.profile.clone(),
                },
                crate_key: unit.crate_key,
                module_path: vec![],
                cfg: unit.cfg.fingerprint(),
            },
        })
        .collect::<Vec<_>>();
    let inputs = PreparedSourceInputs::refresh_with_dependencies(
        None,
        units,
        Default::default(),
        vec![SourceDependency {
            consumer: Digest([2; 32]),
            alias: "edition_kw_macro_2015".into(),
            provider: Digest([1; 32]),
        }],
    )
    .unwrap();
    let views = selections
        .iter()
        .map(|selection| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(selection).unwrap() else {
                panic!("source absent")
            };
            view
        })
        .collect::<Vec<_>>();
    let discovery = SourceDiscovery::new(
        views.iter().map(|view| view.share()).collect(),
        views[0].outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        Digest([5; 32]),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest([6; 32]),
        lrr::expand::source_declarative::engine_version(),
        std::sync::Arc::new(
            lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &views[0].selection().outline.parse.profile,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    (views, discovery, inputs)
}
fn sealed(
    year: u16,
    text: &str,
) -> (
    Vec<lrr::source::changes::PreparedSourceView>,
    SourceDiscovery,
    PreparedSourceInputs,
) {
    let (views, mut discovery, inputs) = prepare(year, text);
    for _ in 0..1000 {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => return (views, discovery, inputs),
            _ => panic!("unexpected stock discovery authority or incomplete result"),
        }
    }
    panic!("stock discovery did not seal")
}

#[test]
fn unchanged_stock_declarative_literals_execute_before_nonliteral_frontier() {
    use lrr::{
        db::{memo::Database, poll::QueryPoll},
        engine::run::{RunMachine, RunPoll},
    };
    for (year, text) in [(2015, ROOT2015), (2018, ROOT2018)] {
        let (views, discovery, inputs) = sealed(year, text);
        let body = views[0]
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "check_async")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        discovery.publish(&mut db).unwrap();
        let contract = lrr::engine::fixture::FixtureManifest::parse(
            r#"schema_version=1
id="M017-DECLARATIVE-DISPATCH-COMPONENT"
category="relaxed"
source="caller.rs"
entry="check_async"
modes=["off"]
oracle=false
[expect]
status="fault"
diagnostic_code="LRR-E-UNSUPPORTED"
stdout=""
stderr=""
"#,
        )
        .unwrap();
        let markers = views[0].fixture_markers().unwrap();
        let observer = lrr::engine::fixture_observer::FixturePreparationObserver::start(
            &db, &views, &views[0], &markers, &contract,
        )
        .unwrap();
        db.begin_run().unwrap();
        let mut run = match RunMachine::start_with_discovery(&mut db, body, 10_000, discovery) {
            QueryPoll::Ready(Ok(run)) => run,
            QueryPoll::Ready(Err(e)) => panic!("actual stock startup {e}"),
            QueryPoll::NeedInput(i) => panic!("actual stock startup demand {i:?}"),
        };
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("full unchanged body must expose the pending nonliteral frontier")
        };
        assert_eq!(error.code, "LRR-E-UNSUPPORTED");
        assert_eq!(run.declarative_expressions().count(), 2);
        assert_eq!(
            observer.collect(&db, &run).unwrap_err().code,
            "LRR-E-FIXTURE-OBSERVATION"
        );
        let trace_before = db.trace.events.len();
        let fuel_before = run.fuel_snapshot();
        let observed = observer
            .collect_with_declarative_expressions(&db, &run, run.declarative_expressions())
            .unwrap();
        assert_eq!(observed.status.as_deref(), Some("fault"));
        assert_eq!(
            observed.diagnostic_code.as_deref(),
            Some("LRR-E-UNSUPPORTED")
        );
        assert_eq!(db.trace.events.len(), trace_before);
        assert_eq!(run.fuel_snapshot(), fuel_before);
        assert_eq!(
            run.opcode_counters().unwrap().started_by_opcode[0],
            if year == 2015 { 4 } else { 3 },
            "original initializer literals and both sealed macro literals must execute before nonliteral frontier: {error}"
        );
    }
}
