//! T050/T051 zero-binding scalar alternative phases; root typed no-arm prerequisite.
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
    ir::{
        op::{DecodedLiteral, HintRecipe, Op, PrimitiveType},
        pc::Phase,
        recipe::RecipeExit,
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        parse::ParseKey,
    },
};
const ORIGINAL: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/or-patterns/issue-69875-should-have-been-expanded-earlier-non-exhaustive.rs"
);
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

#[test]
fn unchanged_nested_integer_alternatives_prepare_first_leaf_only_with_exact_origin() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view) = source(ORIGINAL, edition);
        let handle = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "main")
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
            panic!("or index")
        };
        let node = index
            .nodes()
            .iter()
            .find(|n| n.role == NodeRole::Expression(ExpressionKind::Match))
            .unwrap();
        let pc = index.pc(node.id, Phase::Entry).unwrap();
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!("or entry")
        };
        assert!(recipe.recipe().operations.iter().any(|op| matches!(
            op,
            Op::SetHint {
                hint: HintRecipe::Primitive(PrimitiveType::I32),
                ..
            }
        )));
        let arm = index
            .nodes()
            .iter()
            .find(|n| n.role == NodeRole::MatchArm)
            .unwrap();
        let fetched = db
            .fetch(
                &SyntaxProvider,
                QueryKey::Thunk(index.pc(arm.id, Phase::Entry).unwrap()),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!("arm entry")
        };
        let RecipeExit::Next(first) = &recipe.recipe().exit else {
            panic!("first alternative")
        };
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(first.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!("leaf recipe")
        };
        assert_eq!(
            recipe.recipe().origin.span,
            index.span(lrr::ir::pc::NodeId(first.node)).unwrap()
        );
        assert!(recipe.recipe().operations.iter().any(|op|matches!(op,Op::Literal{literal,..} if matches!(literal.decoded,Some(DecodedLiteral::Integer{negative:false,magnitude:0,..})))));
        assert!(!db.trace.events.iter().any(|e|matches!(&e.key,QueryKey::Thunk(other) if other.phase==Phase::Entry.code().unwrap() && other.node!=pc.node && other.node!=arm.id.0 && other.node!=first.node)));
    }
}
fn run(text: &str, expected: bool, compares: u64) {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view) = source(text, edition);
        let handle = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "value")
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
            panic!("ordered execution index")
        };
        let patterns = index
            .nodes()
            .iter()
            .filter(|n| n.role == NodeRole::Pattern)
            .map(|n| n.id.0)
            .collect::<Vec<_>>();
        let cold = index
            .nodes()
            .iter()
            .filter(|n| {
                let start = handle.range.start.checked_add(n.range.start).unwrap() as usize;
                let end = handle.range.start.checked_add(n.range.end).unwrap() as usize;
                text.get(start..end) == Some("unknown()")
            })
            .map(|n| n.id.0)
            .collect::<Vec<_>>();
        let revision = db.source_revision();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut machine)) =
            RunMachine::start_with_sources(&mut db, body.clone(), 100_000, vec![view])
        else {
            panic!("or startup")
        };
        match machine.poll(&mut db) {
            RunPoll::Completed(report) => {
                assert!(matches!(report.value,ValueSummary::Bool(actual) if actual==expected));
                assert_eq!(report.opcode_counters.started_by_opcode[10], compares);
                assert_eq!(
                    report.opcode_counters.started_by_opcode[0],
                    compares.checked_add(2).unwrap()
                );
                assert_eq!(
                    report.opcode_counters.operations_started,
                    report.opcode_counters.operations_completed
                );
                assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
            }
            RunPoll::Fault(e) => panic!("edition{edition}: {e}"),
            RunPoll::NeedInput(x) => panic!("or suspension {x:?}"),
            RunPoll::Panicked(p) => panic!("or panic {:?}", p.panic.kind),
        }
        assert_eq!(db.source_revision(), revision);
        assert_eq!(db.trace.truncated, 0);
        let entered = db
            .trace
            .events
            .iter()
            .filter_map(|e| match &e.key {
                QueryKey::Thunk(pc)
                    if pc.body == body
                        && pc.phase == Phase::Entry.code().unwrap()
                        && patterns.contains(&pc.node) =>
                {
                    Some(pc.node)
                }
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(u64::try_from(entered.len()).unwrap(), compares);
        assert!(!db.trace.events.iter().any(|e| matches!(&e.key,
            QueryKey::Thunk(pc) if pc.body == body && cold.contains(&pc.node))));
        db.finish_run().unwrap();
    }
}
#[test]
fn disclosed_original_pattern_adapters_preserve_order_single_source_and_local_copy() {
    run(
        "fn value()->bool{match 0{0 | (1 | 2)=>true,_=>unknown()}}fn main(){}",
        true,
        1,
    );
    run(
        "fn value()->bool{match 1{0 | (1 | 2)=>true,_=>unknown()}}fn main(){}",
        true,
        2,
    );
    run(
        "fn value()->bool{match 2{0 | (1 | 2)=>true,_=>unknown()}}fn main(){}",
        true,
        3,
    );
    run(
        "fn value()->bool{let source:i8=-128;let result=match source{-127 | -128=>true,_=>unknown()};source;result}fn main(){}",
        true,
        2,
    );
    run(
        "fn value()->bool{match true{false | true=>true,_=>unknown()}}fn main(){}",
        true,
        2,
    );
}
#[test]
fn reached_or_binding_range_and_guard_frontiers_refuse_before_compare_or_commit() {
    for (text, marker, phase) in [
        (
            "fn value(source:i8)->bool{match source{0 | 0..=1=>true,_=>false}}fn main(){}",
            "0..=1",
            Phase::Entry,
        ),
        (
            "fn value(source:bool)->bool{match source{true | other=>true,_=>false}}fn main(){}",
            "true | other=>true,",
            Phase::Entry,
        ),
        (
            "fn value(source:bool)->bool{match source{true | false if unknown()=>true,_=>false}}fn main(){}",
            "true",
            Phase::Commit,
        ),
    ] {
        let (mut db, view) = source(text, 2024);
        let handle = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "value")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::BodyStructure(handle.key()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
            panic!("unsupported header index")
        };
        let start = u32::try_from(text.find(marker).unwrap())
            .unwrap()
            .checked_sub(handle.range.start)
            .unwrap();
        let end = start
            .checked_add(u32::try_from(marker.len()).unwrap())
            .unwrap();
        let node = index
            .nodes()
            .iter()
            .find(|n| {
                n.range.start == start
                    && n.range.end == end
                    && matches!(n.role, NodeRole::MatchArm | NodeRole::Pattern)
            })
            .unwrap();
        let pc = index.pc(node.id, phase).unwrap();
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!("unsupported reached recipe")
        };
        assert!(
            recipe
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::Unsupported(_)))
        );
        assert!(!recipe.recipe().operations.iter().any(|op| matches!(
            op,
            Op::NumericBinary { .. }
                | Op::ReadLocal { .. }
                | Op::ReadParameter { .. }
                | Op::Literal { .. }
                | Op::ScopeEnter { .. }
        )));
        assert!(
            db.trace
                .events
                .iter()
                .all(|event| !matches!(&event.key,QueryKey::Thunk(other) if other!=&pc))
        );
    }
}
#[test]
fn cold_later_unsupported_arm_and_unmatched_guard_stay_semantically_unprepared() {
    for text in [
        "fn value()->bool{match true{true=>true,0..=1=>unknown()}}fn main(){}",
        "fn value()->bool{match true{true=>true,false if unknown()=>false}}fn main(){}",
        "fn value()->bool{match 0i8{0=>true,other=>unknown()}}fn main(){}",
        "fn value()->bool{match 0i8{0 | 1..=2=>true,_=>unknown()}}fn main(){}",
    ] {
        run(text, true, 1)
    }
    run(
        "fn value()->bool{match true{false | false if unknown()=>unknown(),true=>true}}fn main(){}",
        true,
        3,
    );
}
#[test]
fn effectful_prefix_and_selected_body_commit_once_in_written_alternative_order() {
    let text = "fn value()->i32{let mut seen:i32=0;seen=seen+1;match 1{0 | 1=>{seen=seen+1;},_=>unknown()};seen}fn main(){}";
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view) = source(text, edition);
        let body = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "value")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut machine)) =
            RunMachine::start_with_sources(&mut db, body, 100_000, vec![view])
        else {
            panic!("effectful match startup")
        };
        let RunPoll::Completed(report) = machine.poll(&mut db) else {
            panic!("effectful match did not complete")
        };
        assert!(matches!(report.value,ValueSummary::Signed(ref n) if n=="2"));
        assert_eq!(report.opcode_counters.started_by_opcode[16], 2);
        assert_eq!(report.opcode_counters.started_by_opcode[10], 4);
        assert_eq!(report.opcode_counters.started_by_opcode[0], 6);
        assert_eq!(
            report.opcode_counters.operations_started,
            report.opcode_counters.operations_completed
        );
        assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        db.finish_run().unwrap();
    }
}

#[test]
fn no_compile_style_exhaustiveness_rejection_success_and_reached_no_arm_are_distinct() {
    run(
        "fn value()->bool{match true{true=>true}}fn main(){}",
        true,
        1,
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view) = source(
            "fn value()->bool{match false{true=>true}}fn main(){}",
            edition,
        );
        let body = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "value")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut machine)) =
            RunMachine::start_with_sources(&mut db, body, 100_000, vec![view])
        else {
            panic!("no-arm startup")
        };
        let RunPoll::Fault(error) = machine.poll(&mut db) else {
            panic!("missing reached no-arm fault")
        };
        assert_eq!(error.code, "LRR-E-MATCH-NO-ARM");
        db.finish_run().unwrap();
    }
}
#[test]
fn typed_no_arm_exit_preserves_canonical_cells_and_charges_only_one_failed_exit() {
    use lrr::{
        ir::pc::NodeId,
        runtime::cell::GuestStore,
        vm::{
            frame::Frame,
            step::{StepOutcome, step},
        },
    };
    let (mut db, view) = source("fn main(){match false{true=>true};}", 2024);
    let handle = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let fetched = db
        .fetch(&SyntaxProvider, QueryKey::BodyStructure(handle.key()))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
        panic!("no-arm index")
    };
    let index = index.as_ref().clone();
    let node = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Match))
        .unwrap();
    let terminal = index.pc(node.id, Phase::Exit).unwrap();
    let mut current = index.pc(NodeId(0), Phase::Entry).unwrap();
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let mut frame = Frame::new(index.clone(), current.clone(), &mut store).unwrap();
    frame.set_fuel_limit(100).unwrap();
    let fetch = |db: &mut Database, pc| {
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(pc))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!("no-arm thunk")
        };
        recipe.as_ref().clone()
    };
    for _ in 0..64 {
        if current == terminal {
            break;
        }
        let recipe = fetch(&mut db, current);
        let StepOutcome::Next(next) = step(&mut frame, &mut store, &recipe).unwrap() else {
            panic!("no-arm continuation")
        };
        current = next;
    }
    assert_eq!(current, terminal);
    let recipe = fetch(&mut db, terminal);
    assert!(recipe.recipe().operations.is_empty());
    assert!(matches!(recipe.recipe().exit, RecipeExit::MatchNoArm));
    let source = node.operands[0];
    let cell = frame
        .cell(index.nodes()[source.0 as usize].value.unwrap())
        .unwrap();
    let before = (
        store.state(cell).unwrap(),
        store.location_epoch(cell).unwrap(),
        store.budget().handles(),
        store.budget().bytes(),
    );
    let fuel = frame.fuel_snapshot().used;
    let started = frame.counters().operations_started;
    let completed = frame.counters().exits_completed;
    assert_eq!(
        step(&mut frame, &mut store, &recipe).unwrap_err().code,
        "LRR-E-MATCH-NO-ARM"
    );
    assert_eq!(
        (
            store.state(cell).unwrap(),
            store.location_epoch(cell).unwrap(),
            store.budget().handles(),
            store.budget().bytes()
        ),
        before
    );
    assert_eq!(frame.fuel_snapshot().used, fuel.checked_add(1).unwrap());
    assert_eq!(frame.counters().operations_started, started);
    assert_eq!(frame.counters().exits_completed, completed);
}
