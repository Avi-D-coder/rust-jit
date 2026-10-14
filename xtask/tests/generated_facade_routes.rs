//! Unchanged rustc issue36768 source/items in fixed2024 pure discovery adaptations.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
    },
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
use std::sync::Arc;
const NAMES: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/issue-36768.rs");
fn cfg() -> CfgSet {
    let mut cfg = CfgSet::guest_default();
    cfg.atoms.insert("test".into());
    cfg
}
fn captured(source: &str, unit: Digest) -> (PreparedSourceInputs, PreparedSourceView) {
    let profile = ProfileIdentity::current();
    let cfg = cfg();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("main.rs").unwrap();
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
        panic!("captured fixture absent")
    };
    (inputs, view)
}

fn catalog(text: &str) -> (Vec<String>, usize) {
    let (inputs, view) = captured(text, Digest([21; 32]));
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::for_tests(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Digest([22; 32]),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest([23; 32]),
        Digest([24; 32]),
        Arc::new(
            lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &ProfileIdentity::current(),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let mut outputs = 0usize;
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => {
                let record = Arc::new(request.expand().unwrap());
                eprintln!(
                    "actual declarative record: {:?}",
                    record.key().fingerprint()
                );
                discovery.admit_expansion(record).unwrap();
                outputs += 1;
                assert!(outputs <= 1);
            }
            SourceDiscoveryStep::Sealed(_) => break,
            _ => {
                panic!("unexpected declaration demand in captured compiler facade metadata fixture")
            }
        }
    }
    discovery.publish(&mut db).unwrap();
    let catalog = discovery.test_catalog(&mut db).unwrap();
    assert!(catalog.seal().is_some());
    for name in ["core", "foo"] {
        let case = catalog.cases().iter().find(|c| c.name == name).unwrap();
        assert!(
            matches!(discovery.lookup(name).unwrap(),lrr::frontend::names::NameLookup::Defined(d) if d.identity==case.definition)
        );
    }
    assert!(!db.trace.events.iter().any(|e| matches!(
        e.key,
        QueryKey::BodyStructure(_)
            | QueryKey::FunctionSignature(_)
            | QueryKey::Thunk(_)
            | QueryKey::Signature(_)
    )));
    (
        catalog.cases().iter().map(|c| c.name.clone()).collect(),
        outputs,
    )
}
#[test]
fn original_extern_core_uses_owned_compiler_facade_and_keeps_value_test_identity() {
    let (names, outputs) = catalog(NAMES);
    assert_eq!(names, ["core", "foo"]);
    assert_eq!(outputs, 0);
}
#[test]
fn generated_extern_core_uses_the_same_captured_facade_as_original_source() {
    let start = NAMES.find("#[test]").unwrap();
    // Exact unchanged item suffix; retained crate directives precede a real macro.
    let text = format!(
        "{}macro_rules! emit{{()=>{{{}}}}}emit!();",
        &NAMES[..start],
        &NAMES[start..]
    );
    assert!(text.contains(&NAMES[start..]));
    let (names, outputs) = catalog(&text);
    assert_eq!(names, ["core", "foo"]);
    assert_eq!(outputs, 1);
}
