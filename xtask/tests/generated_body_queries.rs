use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn source(text: &str) -> (PreparedSourceView, PreparedSourceInputs) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([241; 32]);
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
        panic!("source absent")
    };
    (view, inputs)
}

fn discover(v: &PreparedSourceView) -> SourceDiscovery {
    SourceDiscovery::new(
        vec![v.share()],
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        Digest([245; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap()
}

fn finish(w: &mut SourceDiscovery) {
    loop {
        match w.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => w
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => w
                .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedDeriveAppend { .. }
            | SourceDiscoveryStep::NeedSourceDeriveSelection { .. }
            | SourceDiscoveryStep::NeedSourceDeriveExpansion { .. }
            | SourceDiscoveryStep::NeedSourceDeriveAppend { .. }
            | SourceDiscoveryStep::NeedDeriveDeclaration { .. }
            | SourceDiscoveryStep::NeedProceduralSelection { .. }
            | SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("unexpected procedural selection in this fixture")
            }
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::Incomplete { .. } => {
                panic!("fixture failed to complete discovery")
            }
        }
    }
}
fn run_body(
    db: &mut lrr::db::memo::Database,
    body: lrr::source::manifest::BodyKey,
) -> lrr::engine::run::RunReport {
    let lrr::db::poll::QueryPoll::Ready(Ok(mut machine)) =
        lrr::engine::run::RunMachine::start(db, body, 10_000)
    else {
        panic!("generated body entry not ready")
    };
    match machine.poll(db) {
        lrr::engine::run::RunPoll::Completed(report) => report,
        lrr::engine::run::RunPoll::Fault(error) => {
            panic!("generated execution fault: {}", error.message)
        }
        _ => panic!("generated execution did not complete"),
    }
}
#[test]
fn generated_signature_body_queries_and_vm_execute_actual_owned_output() {
    use lrr::db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    };
    let (v, inputs) = source(
        "use made as selected;macro_rules! make{()=>{pub fn made()->i64{7_i64}pub fn cold(){missing::<NoSuchType>();}}}make!();",
    );
    let mut w = discover(&v);
    finish(&mut w);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    w.publish(&mut db).unwrap();
    let lrr::frontend::names::NameLookup::Defined(def) = w.lookup("selected").unwrap() else {
        panic!("generated alias absent")
    };
    let QueryPoll::Ready(Ok(signature)) =
        db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(def.identity))
    else {
        panic!("generated signature absent")
    };
    let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
        panic!("signature kind differs")
    };
    assert_eq!(signature.definition, def.identity);
    assert!(signature.parameters.is_empty());
    let source = db.input(&InputKey::DefinitionSource(def.identity)).unwrap();
    let InputValue::DefinitionSource(parse) = source.as_ref() else {
        panic!("generated parse locator absent")
    };
    assert!(db.input(&InputKey::SourceBlob(parse.content)).is_none());
    assert!(
        db.input(&InputKey::GeneratedSyntax(parse.content))
            .is_some()
    );
    let body = db.input(&InputKey::DefinitionBody(def.identity)).unwrap();
    let InputValue::DefinitionBody(body) = body.as_ref() else {
        panic!("generated body absent")
    };
    let report = run_body(&mut db, body.key());
    assert!(matches!(report.value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7"));
    assert!(report.opcode_counters.operations_completed > 0);
    let lrr::frontend::names::NameLookup::Defined(cold) = w.lookup("cold").unwrap() else {
        panic!("cold metadata absent")
    };
    let input = db.input(&InputKey::DefinitionBody(cold.identity)).unwrap();
    let InputValue::DefinitionBody(cold_body) = input.as_ref() else {
        panic!("cold body handle absent")
    };
    assert!(db.memo(&QueryKey::BodyStructure(cold_body.key())).is_none());
}
#[test]
fn identical_generated_text_keeps_distinct_owned_parse_and_body_identities() {
    use lrr::db::{
        dependency::{InputKey, InputValue},
        memo::Database,
    };
    let (v, inputs) = source("macro_rules! make{()=>{pub fn made()->i64{7_i64}}}make!();make!();");
    let mut w = discover(&v);
    finish(&mut w);
    let lrr::frontend::names::NameLookup::Ambiguous(defs) = w.lookup("made").unwrap() else {
        panic!("duplicate generated definitions not ambiguous")
    };
    assert_eq!(defs.len(), 2);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    w.publish(&mut db).unwrap();
    let mut parses = std::collections::BTreeSet::new();
    for def in defs {
        let input = db.input(&InputKey::DefinitionSource(def.identity)).unwrap();
        let InputValue::DefinitionSource(parse) = input.as_ref() else {
            panic!("generated source absent")
        };
        assert!(parses.insert(parse.clone()));
        let input = db.input(&InputKey::DefinitionBody(def.identity)).unwrap();
        let InputValue::DefinitionBody(body) = input.as_ref() else {
            panic!("generated body absent")
        };
        assert_eq!(body.def_key, def.identity);
        assert!(
            matches!(run_body(&mut db,body.key()).value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7")
        );
    }
}
#[test]
fn generated_body_queries_bind_the_current_discovery_and_record_inputs() {
    use lrr::db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
    };
    let (v, inputs) = source("macro_rules! make{()=>{pub fn made(){}}}make!();");
    let mut w = discover(&v);
    finish(&mut w);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    w.publish(&mut db).unwrap();
    let lrr::frontend::names::NameLookup::Defined(def) = w.lookup("made").unwrap() else {
        panic!("generated definition absent")
    };
    let input = db.input(&InputKey::DefinitionSource(def.identity)).unwrap();
    let InputValue::DefinitionSource(parse) = input.as_ref() else {
        panic!("generated source absent")
    };
    let key = QueryKey::ParseFile(parse.clone());
    assert!(matches!(
        db.fetch(&SyntaxProvider, key.clone()),
        QueryPoll::Ready(Ok(_))
    ));
    assert!(db.memo(&key).unwrap().dependencies.iter().any(|d|matches!(d,DependencyStamp::Query{key:QueryKey::DiscoverySeal(seal),..} if seal==w.key())));
    let (_, changed) = source("pub fn replacement(){}");
    changed.publish(&mut db).unwrap();
    let stale = db.fetch(&SyntaxProvider, key);
    assert!(
        matches!(
            stale,
            QueryPoll::NeedInput(lrr::db::poll::InputRequest::Discover { .. })
        ),
        "unexpected stale parse result: {stale:?}"
    );
}

#[test]
fn generated_query_inputs_are_shared_by_value_and_type_discovery_certificates() {
    use lrr::db::{
        dependency::{InputKey, InputValue},
        memo::Database,
    };
    let (v, inputs) = source("macro_rules! make{()=>{pub fn made()->i64{7_i64}}}make!();");
    let mut value = discover(&v);
    finish(&mut value);
    let mut types = SourceDiscovery::new(
        vec![v.share()],
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Type,
        Digest([245; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap();
    finish(&mut types);
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    value.publish(&mut db).unwrap();
    types.publish(&mut db).unwrap();
    let lrr::frontend::names::NameLookup::Defined(def) = value.lookup("made").unwrap() else {
        panic!("generated definition absent")
    };
    let input = db.input(&InputKey::DefinitionBody(def.identity)).unwrap();
    let InputValue::DefinitionBody(body) = input.as_ref() else {
        panic!("generated body absent")
    };
    assert!(
        matches!(run_body(&mut db,body.key()).value,lrr::engine::run::ValueSummary::Signed(ref n) if n=="7")
    );
}
