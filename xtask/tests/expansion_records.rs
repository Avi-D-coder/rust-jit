//! Actual pinned declarative expansion is fulfilled outside the query provider.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::{ConstantKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::{Provided, Provider, SyntaxProvider},
        value::QueryValue,
    },
    engine::dispatch::{ResumePoll, SavedPhase},
    expand::{
        identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        record::ExpansionRecord,
    },
    service::expansion::{
        DemandContext, ExpansionCoordinator, PhaseIdentity, Reply, StartDecision,
    },
    source::manifest::BodyKey,
    syntax::token_wire::{self, TokenTree},
};
use lrr_expand_adapter::{declarative::DeclarativeMacro, token_wire::Edition};
use std::{
    cell::Cell,
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[allow(dead_code)]
#[path = "../../crates/lrr-expand-adapter/tests/support/declarative.rs"]
mod fixture;
struct Lookup {
    key: ExpansionKey,
    runs: Cell<usize>,
}
impl Provider for Lookup {
    fn execute(&self, db: &mut Database, _: &QueryKey) -> QueryPoll<Provided> {
        self.runs.set(self.runs.get() + 1);
        match db.fetch(
            &SyntaxProvider,
            QueryKey::ExpansionInput(Box::new(self.key.clone())),
        ) {
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::Ready(Ok(outcome)) => {
                let QueryValue::ExpansionInput(descriptor) = outcome.value.as_ref() else {
                    panic!("wrong expansion query")
                };
                let input = db
                    .input(&InputKey::ExpansionRecord(descriptor.key.fingerprint()))
                    .unwrap();
                let InputValue::ExpansionRecord(record) = input.as_ref() else {
                    panic!("wrong expansion input")
                };
                let tokens = record.tokens().unwrap();
                let mut stack: Vec<_> = tokens.roots().iter().collect();
                let mut values = vec![];
                while let Some(token) = stack.pop() {
                    match token {
                        TokenTree::Group { children, .. } => stack.extend(children.iter()),
                        TokenTree::Literal { token_text, .. } => {
                            values.push(token_text.parse::<i128>().unwrap())
                        }
                        _ => {}
                    }
                }
                assert_eq!(values.len(), 1);
                Provided::pure(QueryValue::Constant(values[0]))
            }
        }
    }
}
#[test]
fn mx013_actual_expansion_suspends_and_reenters_without_duplicate_invocation() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m008-expansion-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let source = "macro_rules! id{($value:expr)=>{$value};}fn main(){println!(\"{}\",id!(43));}";
    fs::write(work.join("native.rs"), source).unwrap();
    let build = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "native.rs", "-o", "native"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("native.stderr"), &build.stderr).unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(work.join("native")).output().unwrap();
    assert!(native.status.success());
    fs::write(work.join("native.stdout"), &native.stdout).unwrap();
    let native: i128 = String::from_utf8(native.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let definition = fixture::definition("($value:expr)=>{$value};", Edition::E2024);
    let input = fixture::input("43");
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest::of_bytes(&definition.encode().unwrap()),
        },
        invocation: Digest([2; 32]),
        input_tokens: Digest::of_bytes(&input.encode().unwrap()),
        consumer_compile_env: Digest([3; 32]),
        parent_context: Digest(input.contexts()[0].key().0),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Expression,
        engine_version: Digest([4; 32]),
        procedural_epoch: None,
    };
    let lookup = Lookup {
        key: key.clone(),
        runs: Cell::new(0),
    };
    let query = QueryKey::Constant(ConstantKey {
        definition: Digest([5; 32]),
        arguments: vec![],
        cfg: Digest::default(),
        profile: ProfileIdentity::current(),
    });
    let mut db = Database::default();
    let c = DemandContext {
        request: Digest([6; 32]),
        session: Digest([7; 32]),
        unit: Digest([8; 32]),
        revision: db.source_revision(),
        epoch: 11,
        source_generation: Digest([9; 32]),
        native_generation: None,
        compile_environment: key.consumer_compile_env,
    };
    let mut coordinator = ExpansionCoordinator::new(c.clone());
    let QueryPoll::NeedInput(request) = db.fetch(&lookup, query.clone()) else {
        panic!("missing output did not suspend")
    };
    assert!(db.memo(&query).is_none());
    let token = coordinator
        .issue(
            query.clone(),
            PhaseIdentity {
                frame: Digest([10; 32]),
                body: BodyKey {
                    definition: Digest([11; 32]),
                    exact_body_bytes: Digest([12; 32]),
                },
                index: 3,
            },
            request,
        )
        .unwrap();
    let saved = SavedPhase::new(token.clone(), vec![17u64, 31]);
    assert_eq!(coordinator.start(&c, &token).unwrap(), StartDecision::Start);
    let macro_invocations = Cell::new(0);
    macro_invocations.set(macro_invocations.get() + 1);
    let output = DeclarativeMacro::new(definition)
        .unwrap()
        .expand(&input, fixture::call_site(&input))
        .unwrap()
        .unwrap();
    let owned = token_wire::TokenForest::decode(&output.tokens.encode().unwrap()).unwrap();
    let record = ExpansionRecord::from_tokens(key, &owned).unwrap();
    let output_digest = record.output();
    coordinator
        .accept(&c, &token, Reply::success(&token, output_digest))
        .unwrap();
    let ResumePoll::Waiting(saved) = saved.resume(&mut coordinator, &c) else {
        panic!("unpublished output resumed")
    };
    coordinator.adopt(&c, &token, &mut db, record).unwrap();
    let value = db
        .fetch(&lookup, query.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::Constant(observed) = value.value.as_ref() else {
        panic!("lookup did not resolve")
    };
    assert_eq!(*observed, native);
    let runs = lookup.runs.get();
    db.fetch(&lookup, query.clone())
        .into_ready()
        .unwrap()
        .unwrap();
    assert_eq!(lookup.runs.get(), runs);
    assert_eq!(macro_invocations.get(), 1);
    let ResumePoll::Ready { state, output } = saved.resume(&mut coordinator, &c) else {
        panic!("installed input failed to resume")
    };
    assert_eq!(state, vec![17, 31]);
    assert_eq!(output, output_digest);
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M008","fixture":"MX013","passed":true,"native":native,"observed":observed,"macro_invocations":macro_invocations.get(),"pure_lookup_runs":lookup.runs.get(),"scope":"actual pinned declarative expansion outside SourceDb, token-checked immutable record adoption, pure lookup suspension/reentry/cache reuse and owned saved payload versus native macOS; procedural helper execution and concrete guest VM effects remain later integration"})).unwrap()).unwrap();
}
