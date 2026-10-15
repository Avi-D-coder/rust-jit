//! T050/T051 closed scalar MatchExpr acceptance; upstream copies unchanged.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    engine::run::{RunMachine, RunPoll, ValueSummary},
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    ir::{
        op::{HintRecipe, Op, PrimitiveType},
        pc::Phase,
        recipe::RecipeExit,
    },
    source::manifest::Namespace,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{BodyIndexLimits, ExpressionKind, NodeRole},
        parse::{ParseKey, ParsedFile},
    },
};
const MATCH_EXPR: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/binding/expr-match.rs");
const MATCH_PANIC: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/binding/expr-match-panic.rs");
const MATCH_LIT: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/binding/match-pattern-lit.rs");
fn source(text: &str, edition: u16) -> (Database, PreparedSourceView) {
    let path = SourcePath::new("integer-pattern.rs").unwrap();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let owner = Digest::of_bytes(b"LRR scalar numeric pattern source");
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: owner,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            blob,
            profile,
            cfg,
            crate_key: owner,
            module_path: vec![],
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("pattern source absent")
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    (db, view)
}
#[derive(Clone, Copy)]
enum EntryContext {
    Primitive(PrimitiveType),
    Parameter(u32),
    Unsupported,
}
fn entry(text: &str, function: &str, expected: EntryContext) {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view) = source(text, edition);
        let handle = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == function)
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let body = handle.key();
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::BodyStructure(body.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
            panic!("match index")
        };
        let node = index
            .nodes()
            .iter()
            .find(|n| n.role == NodeRole::Expression(ExpressionKind::Match))
            .unwrap();
        assert!(node.runtime_temporaries.iter().all(Option::is_some));
        let pc = index.pc(node.id, Phase::Entry).unwrap();
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!("match entry")
        };
        assert_eq!(recipe.recipe().key, pc);
        assert_eq!(recipe.recipe().origin.span, index.span(node.id).unwrap());
        match expected {
            EntryContext::Primitive(ty) => {
                assert!(recipe.recipe().operations.iter().any(
                    |op| matches!(op,Op::SetHint{hint:HintRecipe::Primitive(found),..} if *found==ty)
                ));
                assert!(
                    !recipe
                        .recipe()
                        .operations
                        .iter()
                        .any(|op| matches!(op, Op::Unsupported(_) | Op::Literal { .. }))
                );
                assert!(matches!(recipe.recipe().exit, RecipeExit::Next(_)));
            }
            EntryContext::Parameter(ordinal) => {
                assert!(recipe.recipe().operations.iter().any(
                    |op| matches!(op, Op::SetHint { hint: HintRecipe::DefinitionParameter(found), .. } if *found == ordinal)
                ));
                assert!(!recipe.recipe().operations.iter().any(|op| matches!(
                    op,
                    Op::Unsupported(_)
                        | Op::Literal { .. }
                        | Op::ReadLocal { .. }
                        | Op::ReadParameter { .. }
                )));
                assert!(matches!(recipe.recipe().exit, RecipeExit::Next(_)));
            }
            EntryContext::Unsupported => {
                assert!(
                    recipe
                        .recipe()
                        .operations
                        .iter()
                        .any(|op| matches!(op, Op::Unsupported(_)))
                );
                assert!(!recipe.recipe().operations.iter().any(|op| matches!(
                    op,
                    Op::SetHint { .. }
                        | Op::ReadLocal { .. }
                        | Op::ReadParameter { .. }
                        | Op::Literal { .. }
                )));
            }
        }
        assert!(
            db.trace
                .events
                .iter()
                .all(|event| !matches!(&event.key,QueryKey::Thunk(other) if other!=&pc))
        );
    }
}
#[test]
fn unchanged_scalar_match_sources_prepare_exact_entry_without_arm_thunks() {
    entry(
        MATCH_EXPR,
        "test_basic",
        EntryContext::Primitive(PrimitiveType::Bool),
    );
    entry(
        MATCH_PANIC,
        "test_simple",
        EntryContext::Primitive(PrimitiveType::Bool),
    );
    entry(
        MATCH_LIT,
        "altlit",
        EntryContext::Primitive(PrimitiveType::Isize),
    );
}
fn run(text: &str, function: &str, expected: Option<bool>) {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view) = source(text, edition);
        let handle = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == function)
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let body = handle.key();
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::BodyStructure(body.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
            panic!("execution index")
        };
        let cold = index
            .nodes()
            .iter()
            .filter(|n| {
                let a = handle.range.start.checked_add(n.range.start).unwrap() as usize;
                let b = handle.range.start.checked_add(n.range.end).unwrap() as usize;
                text.get(a..b)
                    .is_some_and(|s| s == "unknown()" || s == "panic!()")
            })
            .map(|n| n.id.0)
            .collect::<Vec<_>>();
        let profile = view.selection().outline.parse.profile.clone();
        let mut discovery = SourceDiscovery::new(
            vec![view.share()],
            view.outline().root_scope.clone(),
            Namespace::Value,
            Digest::of_bytes(b"LRR match original discovery"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"LRR match declarative environment"),
            Digest::of_bytes(b"LRR match compiler facade"),
            std::sync::Arc::new(
                lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(&profile)
                    .unwrap(),
            ),
        )
        .unwrap();
        loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::Sealed(_) => break,
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Incomplete {
                    diagnostic,
                    blocked,
                } => panic!(
                    "{function} edition{edition} match original discovery: {diagnostic}; {blocked:?}"
                ),
                _ => panic!("match original discovery needs unsupported preparation"),
            }
        }
        let macros =
            lrr::engine::standalone_macros::CompletedStandaloneRuntimeMacros::from_discovery(
                &view, &discovery,
            )
            .unwrap();
        let contexts = macros.contexts().unwrap();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut machine)) =
            RunMachine::start_with_discovery(&mut db, body.clone(), 100_000, discovery)
        else {
            panic!("match startup")
        };
        let revision = db.source_revision();
        match machine.poll_with_runtime_macros(&mut db, &contexts) {
            RunPoll::Completed(report) => {
                match expected {
                    Some(wanted) => {
                        assert!(matches!(report.value,ValueSummary::Bool(found) if found==wanted))
                    }
                    None => assert!(matches!(report.value, ValueSummary::Unit)),
                };
                assert_eq!(
                    report.opcode_counters.operations_started,
                    report.opcode_counters.operations_completed
                );
                assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
                if text == MATCH_EXPR && function == "main" {
                    assert_eq!(
                        report.opcode_counters.started_by_opcode[19], 4,
                        "all original helpers called"
                    );
                    // Successful assertions branch to their Unit Exit; Op::Assert
                    // belongs only to the failing Commit phase. The installed table
                    // contains reached recipes, never a whole-body preinstallation.
                    let mut assertion_entries = std::collections::BTreeSet::new();
                    let mut assertion_exits = std::collections::BTreeSet::new();
                    for installed in machine.installed_recipes().recipes() {
                        let recipe = installed.recipe();
                        let span = &recipe.origin.span;
                        let owner = view
                            .outline()
                            .items
                            .iter()
                            .filter_map(|item| item.body.as_ref())
                            .find(|owner| owner.key() == span.body_key)
                            .expect("installed original body");
                        let start = owner.range.start + span.relative_start;
                        let end = owner.range.start + span.relative_end;
                        if text.get(start as usize..end as usize) != Some("assert!(rs)") {
                            continue;
                        }
                        let identity = (recipe.key.body.clone(), recipe.key.node);
                        match Phase::from_code(recipe.key.phase) {
                            Phase::Entry => {
                                assert!(matches!(recipe.exit, RecipeExit::Branch { .. }));
                                assert!(assertion_entries.insert(identity));
                            }
                            Phase::Exit => {
                                assert!(matches!(recipe.operations.as_ref(), [Op::Unit { .. }]));
                                assert!(assertion_exits.insert(identity));
                            }
                            _ => panic!("original successful assertion reached failure phase"),
                        }
                    }
                    assert_eq!(
                        assertion_entries.len(),
                        5,
                        "all original assertions entered"
                    );
                    assert_eq!(
                        assertion_exits, assertion_entries,
                        "all predicates succeeded"
                    );
                    assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
                    assert_eq!(report.opcode_counters.started_by_opcode[25], 0);
                    assert_eq!(machine.peak_guest_frames(), 2);
                }
            }
            RunPoll::Fault(e) => panic!("edition{edition}: {e}"),
            RunPoll::NeedInput(input) => panic!("unexpected match suspension: {input:?}"),
            RunPoll::Panicked(p) => panic!("unexpected match panic: {:?}", p.panic.kind),
        }
        assert_eq!(db.source_revision(), revision);
        assert_eq!(db.trace.truncated, 0);
        assert!(!db.trace.events.iter().any(
            |e| matches!(&e.key,QueryKey::Thunk(pc) if pc.body==body && cold.contains(&pc.node))
        ));
        db.finish_run().unwrap();
    }
}
#[test]
fn unchanged_bool_match_selected_bodies_execute_and_panic_arm_stays_cold() {
    for function in [
        "test_basic",
        "test_inferrence",
        "test_alt_as_alt_head",
        "test_alt_as_block_result",
    ] {
        run(MATCH_EXPR, function, None)
    }
    run(MATCH_EXPR, "main", None);
    run(MATCH_PANIC, "test_simple", None);
}
#[test]
fn disclosed_scalar_matches_select_once_preserve_local_copy_and_cold_arm() {
    for text in [
        "fn value()->bool{match true{true=>true,false=>unknown()}}fn main(){}",
        "fn value()->bool{let input:bool=false;let result=match input{true=>unknown(),false=>true};input;result}fn main(){}",
        "fn value()->bool{let input:i8=-128;let result=match input{-128=>true,_=>unknown()};input;result}fn main(){}",
        "fn value()->bool{match 7u16{1=>unknown(),7=>true,_=>unknown()}}fn main(){}",
    ] {
        run(text, "value", Some(true))
    }
}
#[test]
fn parameter_scrutinees_defer_to_source_guard_and_qualified_names_refuse_before_read() {
    for (text, name) in [
        (
            "fn value(input:String)->bool{match input{_=>true}}fn main(){}",
            "value",
        ),
        (
            "fn value<T>(input:T)->bool{match input{_=>true}}fn main(){}",
            "value",
        ),
        (
            "type bool=String;fn value(input:bool)->u8{match input{_=>0}}fn main(){}",
            "value",
        ),
        (
            "fn main(){let input:bool=true;match ::input{true=>true,false=>false};}",
            "main",
        ),
    ] {
        // The ordinal is a deferred source relation, not acceptance of String,
        // a rigid T or a shadowed bool as a primitive. Actual VM guard controls
        // in parameter_place_tests reject nonprimitive sources before effects.
        let expected = if name == "main" {
            EntryContext::Unsupported
        } else {
            EntryContext::Parameter(0)
        };
        entry(text, name, expected)
    }
}
#[test]
fn scalar_match_temporary_slots_have_exact_checked_source_owned_budget() {
    let text = "fn main(){match true{true=>false,false=>true};}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"scalar match budget"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline.items[0].body.as_ref().unwrap();
    let index = parsed.index_body(handle, Default::default()).unwrap();
    let node = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Match))
        .unwrap();
    let [Some(expected), Some(test)] = node.runtime_temporaries else {
        panic!("two match temporaries")
    };
    assert_eq!(test.0, expected.0.checked_add(2).unwrap());
    assert_eq!(expected.0 % 2, 0);
    let limits = BodyIndexLimits {
        slots: index.slot_count(),
        ..Default::default()
    };
    assert_eq!(parsed.index_body(handle, limits).unwrap(), index);
    let limits = BodyIndexLimits {
        slots: index.slot_count().checked_sub(1).unwrap(),
        ..Default::default()
    };
    assert_eq!(
        parsed.index_body(handle, limits).unwrap_err().code,
        "LRR-E-BODY-INDEX-LIMIT"
    );
}
