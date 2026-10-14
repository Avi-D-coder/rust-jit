//! M008/M017 observer revisions across actual stock expansion query suspension.
//! Original check_async remains a nonliteral fault; this is component observation acceptance.
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
    .with_declarative(
        Digest([6; 32]),
        Digest::of_bytes(lrr_expand_adapter::UPSTREAM_COMMIT.as_bytes()),
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
fn unchanged_stock_query_resumes_across_derived_publication_without_stale_observation() {
    use lrr::{
        db::{
            key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, trace::Action,
        },
        engine::{
            fixture::FixtureManifest,
            fixture_observer::FixturePreparationObserver,
            run::{RunMachine, RunPoll},
        },
        service::expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity, Reply},
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
        let parsed = lrr::syntax::parse::ParsedFile::parse(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            views[0].selection().outline.parse.profile.clone(),
        )
        .unwrap();
        let scan = parsed
            .scan_declarations(
                views[0].selection().path.clone(),
                views[0].outline().root_scope.clone(),
                &CfgSet::guest_default(),
                Default::default(),
            )
            .unwrap();
        assert_eq!(scan.parse(), &views[0].selection().outline.parse);
        assert_eq!(scan.cfg(), views[0].selection().outline.cfg);
        let invocation = scan
            .producers()
            .iter()
            .find(|producer| {
                text.get(producer.range.start as usize..producer.range.end as usize)
                    == Some("consumes_async!(async)")
            })
            .unwrap()
            .identity;
        let record = discovery
            .sealed_declaration_expansions()
            .unwrap()
            .into_iter()
            .find(|record| record.key().invocation == invocation)
            .unwrap();
        assert_eq!(record.key().definition_edition, 2015);
        assert_eq!(record.key().call_edition, year);
        let query = QueryKey::ExpansionInput(Box::new(record.key().clone()));
        let mut db = Database::default();
        db.trace.detailed = true;
        inputs.publish(&mut db).unwrap();
        let source_revision = db.source_revision();
        let begin_query_revision = db.revision();
        let contract = FixtureManifest::parse(
            r#"schema_version=1
id="M008-M017-SUSPENDED-QUERY-OBSERVATION"
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
        let trace_start = db.trace.events.len();
        let observer =
            FixturePreparationObserver::start(&db, &views, &views[0], &markers, &contract).unwrap();
        db.begin_run().unwrap();
        let QueryPoll::NeedInput(request) = db.fetch(&SyntaxProvider, query.clone()) else {
            panic!("unpublished actual stock record must suspend")
        };
        assert!(
            db.memo(&query).is_none(),
            "incomplete query must not be cached"
        );
        assert!(db.trace.events.iter().any(|event| event.key == query
            && event.action == Action::Suspended
            && event.revision == begin_query_revision));
        let context = DemandContext {
            request: Digest([20; 32]),
            session: Digest([21; 32]),
            unit: Digest([2; 32]),
            revision: source_revision,
            epoch: 1,
            source_generation: views[0].selection().outline.parse.content,
            native_generation: None,
            compile_environment: record.key().consumer_compile_env,
        };
        let mut coordinator = ExpansionCoordinator::new(context.clone());
        let token = coordinator
            .issue(
                query.clone(),
                PhaseIdentity {
                    frame: Digest([22; 32]),
                    body: body.clone(),
                    index: 0,
                },
                request,
            )
            .unwrap();
        coordinator.start(&context, &token).unwrap();
        coordinator
            .accept(&context, &token, Reply::success(&token, record.output()))
            .unwrap();
        coordinator
            .adopt(&context, &token, &mut db, record.as_ref().clone())
            .unwrap();
        assert_eq!(db.source_revision(), source_revision);
        assert!(db.revision() > begin_query_revision);
        assert!(matches!(
            db.fetch(&SyntaxProvider, query.clone()),
            QueryPoll::Ready(Ok(_))
        ));
        assert!(db.memo(&query).is_some());
        let mut run = match RunMachine::start_with_discovery(&mut db, body, 10_000, discovery) {
            QueryPoll::Ready(Ok(run)) => run,
            _ => panic!("published actual stock source must start"),
        };
        let RunPoll::Fault(error) = run.poll(&mut db) else {
            panic!("actual unchanged body must retain pending nonliteral frontier")
        };
        assert_eq!(error.code, "LRR-E-UNSUPPORTED");
        assert_eq!(run.declarative_expressions().count(), 2);
        assert_eq!(db.source_revision(), source_revision);
        assert!(
            db.trace
                .events
                .windows(2)
                .all(|pair| pair[0].revision <= pair[1].revision)
        );
        assert!(
            observer.collect(&db, &run).is_err(),
            "ordinary collector must refuse ancestry"
        );
        let before_trace = db.trace.events.clone();
        let before_usage = db.cache_usage().unwrap();
        let before_fuel = run.fuel_snapshot();
        let observed = observer
            .collect_with_declarative_expressions(&db, &run, run.declarative_expressions())
            .unwrap();
        assert_eq!(observed.status.as_deref(), Some("fault"));
        assert_eq!(
            observed.diagnostic_code.as_deref(),
            Some("LRR-E-UNSUPPORTED")
        );
        assert_eq!(
            serde_json::to_value(&db.trace.events).unwrap(),
            serde_json::to_value(before_trace).unwrap()
        );
        assert_eq!(db.source_revision(), source_revision);
        assert_eq!(
            serde_json::to_value(db.cache_usage().unwrap()).unwrap(),
            serde_json::to_value(before_usage).unwrap()
        );
        assert_eq!(run.fuel_snapshot(), before_fuel);

        // Explicit trace-corruption controls: the valid interval cannot admit
        // pre-observer, future, or backwards query revisions.
        let saved = db.trace.events[trace_start].revision;
        for invalid in [begin_query_revision - 1, db.revision() + 1] {
            db.trace.events[trace_start].revision = invalid;
            assert!(
                observer
                    .collect_with_declarative_expressions(&db, &run, run.declarative_expressions())
                    .is_err()
            );
        }
        db.trace.events[trace_start].revision = saved;
        let last = db.trace.events.len() - 1;
        let saved_last = db.trace.events[last].revision;
        db.trace.events[last].revision = begin_query_revision;
        assert!(
            observer
                .collect_with_declarative_expressions(&db, &run, run.declarative_expressions())
                .is_err()
        );
        db.trace.events[last].revision = saved_last;
        db.finish_run().unwrap();
        inputs.publish(&mut db).unwrap();
        assert_ne!(db.source_revision(), source_revision);
        assert!(
            observer
                .collect_with_declarative_expressions(&db, &run, run.declarative_expressions())
                .is_err(),
            "base source publication must remain stale even when query events lie in the interval"
        );
    }
}
