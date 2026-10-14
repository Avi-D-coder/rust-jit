use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
    },
    engine::{test_catalog::TestCatalog, test_harness::run_original},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn prepared(text: &str) -> (PreparedSourceInputs, PreparedSourceView) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    let path = SourcePath::new("src/lib.rs").unwrap();
    let unit = Digest([187; 32]);
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source missing")
    };
    (inputs, view)
}
#[test]
fn original_test_catalog_filters_without_querying_signatures_or_bodies() {
    let (inputs, view) = prepared(
        "#[test]fn selected(){}#[test]fn broken<T:Missing>(){undefined();}#[test]#[should_panic(bad=1)]fn malformed(){}",
    );
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let events = db.trace.events.len();
    let sources = vec![view];
    let catalog = TestCatalog::original(&sources).unwrap();
    assert_eq!(catalog.cases().len(), 3);
    assert_eq!(catalog.select(Some("selected"), true).len(), 1);
    assert_eq!(db.trace.events.len(), events);
    let selected = catalog.select(Some("selected"), true);
    let result = run_original(&mut db, &sources, selected[0], false, 1000).unwrap();
    assert!(result.passed());
    let broken = catalog.select(Some("broken"), true)[0].definition;
    assert!(db.trace.events.iter().all(
        |event| !matches!(event.key,QueryKey::FunctionSignature(key) if key==broken)
            && !matches!(&event.key,QueryKey::BodyStructure(body) if body.definition==broken)
    ));
}
#[test]
fn repeated_selected_test_execution_creates_independent_machines_and_finishes_each_run() {
    let (inputs, view) = prepared(
        "#[test]fn case(){let mut x=0i64;x=x+1;}#[test]#[should_panic]fn expected(){let n=127i8;n+1i8;}",
    );
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let sources = vec![view];
    let catalog = TestCatalog::original(&sources).unwrap();
    for name in ["case", "expected", "case", "expected"] {
        let case = catalog.select(Some(name), true)[0];
        assert!(
            run_original(&mut db, &sources, case, false, 1000)
                .unwrap()
                .passed()
        );
    }
    db.begin_run().unwrap();
    db.finish_run().unwrap();
}

fn test_discovery(view: &PreparedSourceView) -> lrr::expand::source_discovery::SourceDiscovery {
    lrr::expand::source_discovery::SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Digest([188; 32]),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest([189; 32]),
        Digest([190; 32]),
        std::sync::Arc::new(
            lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &ProfileIdentity::current(),
            )
            .unwrap(),
        ),
    )
    .unwrap()
}
fn finish_test_discovery(work: &mut lrr::expand::source_discovery::SourceDiscovery) {
    use lrr::expand::source_discovery::SourceDiscoveryStep;
    loop {
        match work.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => work
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => work
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => break,
            _ => panic!("unexpected test discovery state"),
        }
    }
}
#[test]
fn test_catalog_requires_its_own_completed_and_published_domain() {
    use lrr::db::{poll::QueryPoll, provider::SyntaxProvider, seals::SealDomain};
    let (inputs, view) =
        prepared("#[test]fn original(){} macro_rules! make{()=>{#[test]fn generated(){}}}make!();");
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let mut work = test_discovery(&view);
    assert_eq!(
        work.key().domain,
        SealDomain::TestCatalog {
            id: view.outline().root_scope.crate_key
        }
    );
    let key = QueryKey::DiscoverySeal(work.key().clone());
    assert!(matches!(
        db.fetch(&SyntaxProvider, key.clone()),
        QueryPoll::NeedInput(_)
    ));
    assert!(db.memo(&key).is_none());
    assert!(work.test_catalog(&mut db).is_err());
    finish_test_discovery(&mut work);
    assert!(work.test_catalog(&mut db).is_err());
    work.publish(&mut db).unwrap();
    let catalog = work.test_catalog(&mut db).unwrap();
    assert_eq!(
        catalog
            .cases()
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        vec!["generated", "original"]
    );
    assert_eq!(catalog.seal().unwrap().key(), *work.key());
    assert!(
        catalog.select(Some("original"), true)[0]
            .expansion_ancestry
            .is_empty()
    );
    assert_eq!(
        catalog.select(Some("generated"), true)[0]
            .expansion_ancestry
            .len(),
        1
    );
    let mut scope = lrr::expand::source_discovery::SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        Digest([188; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([189; 32]), Digest([190; 32]))
    .unwrap();
    finish_test_discovery(&mut scope);
    assert!(scope.test_catalog(&mut db).is_err());
}
#[test]
fn sealed_test_metadata_does_not_query_signatures_or_executable_bodies() {
    let (inputs, view) = prepared(
        "macro_rules! make{()=>{#[test]fn broken<T:Missing>(){undefined();}#[test]async fn asynchronous(){missing();}}}make!();#[test]fn plain(){}",
    );
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let mut work = test_discovery(&view);
    finish_test_discovery(&mut work);
    work.publish(&mut db).unwrap();
    let catalog = work.test_catalog(&mut db).unwrap();
    assert_eq!(catalog.cases().len(), 3);
    assert!(catalog.select(Some("asynchronous"), true)[0].unsupported_signature);
    assert!(catalog.cases().iter().all(|c| {
        c.body
            .as_ref()
            .is_some_and(|b| b.definition == c.definition)
    }));
    assert!(db.trace.events.iter().all(|e| !matches!(
        e.key,
        QueryKey::FunctionSignature(_) | QueryKey::BodyStructure(_) | QueryKey::Thunk(_)
    )));
}
#[test]
fn unknown_test_producer_prevents_catalog_even_when_every_known_name_is_filtered() {
    use lrr::expand::source_discovery::SourceDiscoveryStep;
    let (inputs, view) = prepared("#[test]fn known(){} unknown_factory!();");
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let mut work = test_discovery(&view);
    assert!(matches!(
        work.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(work.publish(&mut db).is_err());
    assert!(work.test_catalog(&mut db).is_err());
}

#[test]
fn published_test_catalog_refuses_an_edited_source_revision() {
    let (inputs, view) = prepared("#[test]fn before(){}");
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let mut work = test_discovery(&view);
    finish_test_discovery(&mut work);
    work.publish(&mut db).unwrap();
    let catalog = work.test_catalog(&mut db).unwrap();
    catalog.require_current(&mut db).unwrap();
    let (edited, _) = prepared("#[test]fn after(){}");
    edited.publish(&mut db).unwrap();
    assert_eq!(
        catalog.require_current(&mut db).unwrap_err().code,
        "LRR-E-TEST-DISCOVERY"
    );
}
#[test]
fn published_test_catalog_refuses_a_later_expansion_epoch() {
    let (inputs, view) = prepared("#[test]fn before(){}");
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut work = test_discovery(&view);
    finish_test_discovery(&mut work);
    work.publish(&mut db).unwrap();
    let catalog = work.test_catalog(&mut db).unwrap();
    catalog.require_current(&mut db).unwrap();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(
        catalog.require_current(&mut db).unwrap_err().code,
        "LRR-E-TEST-DISCOVERY"
    );
}

#[test]
fn generated_test_catalog_retains_actual_nested_producer_ancestry() {
    let (inputs, view) = prepared(
        "macro_rules! outer{()=>{macro_rules! inner{()=>{#[test]fn nested(){}}}inner!();}}outer!();",
    );
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let mut work = test_discovery(&view);
    finish_test_discovery(&mut work);
    work.publish(&mut db).unwrap();
    let catalog = work.test_catalog(&mut db).unwrap();
    let ancestry = &catalog.select(Some("nested"), true)[0].expansion_ancestry;
    assert_eq!(ancestry.len(), 2);
    assert_ne!(ancestry[0], ancestry[1]);
    for expansion in ancestry {
        assert!(
            db.input(&lrr::db::dependency::InputKey::ExpansionRecord(*expansion))
                .is_some()
        );
    }
}
