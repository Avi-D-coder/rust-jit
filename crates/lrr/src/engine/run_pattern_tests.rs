//! The single shared pattern transaction: unchanged rustc originals plus LRR
//! route, retry, laziness and exact-once controls. Every owner (match arms and
//! guards, matched `let`, `let`-`else`, `if let`, `while let`) executes the
//! same Open/Test/Commit steps; no context-specific pattern evaluator exists.
use super::*;
use crate::ir::op::PatternOperation;

const MATCH_RANGE: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/match-range.rs");
const MATCH_RANGE_INFER: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/match-range-infer.rs"
);

fn complete(source: &str) -> RunReport {
    let (mut db, mut run) = borrows::tests::start_stock_body(source, "main");
    match run.poll(&mut db) {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("unexpected fault: {error:?}"),
        RunPoll::Panicked(report) => {
            panic!("unexpected guest panic: {}", report.panic.description())
        }
        RunPoll::NeedInput(input) => panic!("unexpected input demand: {input:?}"),
    }
}
fn pattern_steps(report: &RunReport) -> u64 {
    report.opcode_counters.started_by_opcode[45]
}
/// The pattern steps lowered for every pattern-owner phase of `source`.
fn lowered_pattern_operations(source: &str) -> Vec<PatternOperation> {
    let (mut db, run) = borrows::tests::start_stock_body(source, "main");
    let index = Arc::clone(&run.frame.body);
    let mut steps = Vec::new();
    for node in index.nodes() {
        let mut phases = vec![Phase::Entry, Phase::Commit, Phase::Exit];
        for n in 0..node.operands.len() {
            phases.push(Phase::Operand(u16::try_from(n).unwrap()));
        }
        for phase in phases {
            let Ok(pc) = index.pc(node.id, phase) else {
                continue;
            };
            if crate::syntax::pattern_plan::PatternSourcePlan::owner_for(&index, &pc)
                .unwrap()
                .is_none()
            {
                continue;
            }
            let Some(Ok(outcome)) = db.fetch(&SyntaxProvider, QueryKey::Thunk(pc)).into_ready()
            else {
                continue;
            };
            if let QueryValue::Thunk(recipe) = outcome.value.as_ref() {
                for op in recipe.recipe().operations.iter() {
                    if let Op::Pattern { operation } = op {
                        steps.push(operation.clone());
                    }
                }
            }
        }
    }
    steps
}

#[test]
fn rustc_range_originals_execute_through_the_shared_matcher() {
    for original in [MATCH_RANGE, MATCH_RANGE_INFER] {
        let report = complete(original);
        assert!(pattern_steps(&report) > 0);
    }
}

#[test]
fn false_guard_retries_later_alternatives_of_the_same_arm_without_replaying_the_scrutinee() {
    const SOURCE: &str = "
        fn one(n: i32) -> i32 { n }
        fn main() {
            let r = match (one(1_i32), 1_i32) {
                (1, y) | (y, 1) if y == 1_i32 => y + 10_i32,
                _ => 0_i32,
            };
            if r != 11_i32 { let z = 0_i32; let _ = 1_i32 / z; }
        }";
    let report = complete(SOURCE);
    // CallSeal: the scrutinee call is executed exactly once.
    assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
    const SECOND: &str = "
        fn main() {
            let x = 1_i32;
            let r = match (x, 2_i32) { (1, y) | (y, 1) if y > 5_i32 => y, (a, b) => a + b };
            if r != 3_i32 { let z = 0_i32; let _ = 1_i32 / z; }
        }";
    complete(SECOND);
}

#[test]
fn guard_side_effects_run_once_per_reached_alternative() {
    const SOURCE: &str = "
        fn main() {
            let mut seen = 0_i32;
            let v = 3_i32;
            match v {
                a @ 1..=5 if { seen = seen + 1_i32; a > 4_i32 } => {}
                b @ (3 | 4) => { if b != 3_i32 { let z = 0_i32; let _ = 1_i32 / z; } }
                _ => {}
            }
            if seen != 1_i32 { let z = 0_i32; let _ = 1_i32 / z; }
        }";
    complete(SOURCE);
}

#[test]
fn every_owner_kind_uses_the_same_transaction_steps() {
    const SOURCE: &str = "
        fn main() {
            let t = (1_i32, (2_i32, 3_i32));
            let (a, (b, c)) = t;
            let (first, ..) = t;
            let (x, 9) = t.1 else { return; };
            if let (1, _) = t { let _u = a + b + c + first; }
            let mut n = 0_i32;
            while let 0..=2 = n { n = n + 1_i32; }
            let m = match t { (p, q) if p > 5_i32 => p, (p, _) => p };
            let _ = x + m;
        }";
    let steps = lowered_pattern_operations(SOURCE);
    assert!(
        steps
            .iter()
            .any(|step| matches!(step, PatternOperation::Open { .. }))
    );
    assert!(
        steps
            .iter()
            .any(|step| matches!(step, PatternOperation::Test { .. }))
    );
    assert!(
        steps
            .iter()
            .any(|step| matches!(step, PatternOperation::Commit { .. }))
    );
    // The let-else above takes its diverging else branch.
    complete(SOURCE);
}

#[test]
fn reference_patterns_and_default_binding_modes_bind_through_the_scrutinee() {
    const SOURCE: &str = "
        fn main() {
            let r = &(5_i32, 6_i32);
            match r { &(a, b) => { if a + b != 11_i32 { let z = 0_i32; let _ = 1_i32 / z; } } }
            match r { (a, b) => { if *a + *b != 11_i32 { let z = 0_i32; let _ = 1_i32 / z; } } }
            let (p, q) = r;
            if *p != 5_i32 || *q != 6_i32 { let z = 0_i32; let _ = 1_i32 / z; }
        }";
    complete(SOURCE);
}

#[test]
fn unreached_arms_keep_unsupported_patterns_and_results_cold() {
    // Only the never-selected arm names a path pattern and an unsupported
    // call; reaching the first arm must not prepare or refuse either.
    const SOURCE: &str = "
        fn main() {
            let k = match 1_i32 { 1 => 7_i32, Some(x) => x.unknown_method(), _ => 0_i32 };
            if k != 7_i32 { let z = 0_i32; let _ = 1_i32 / z; }
            // The unselected or-alternative names an unresolvable path; its
            // resolution refusal stays with that slot and is never reached.
            let j = match 1_i32 { 1 | Missing::X => 7_i32, _ => 0_i32 };
            if j != 7_i32 { let z = 0_i32; let _ = 1_i32 / z; }
        }";
    complete(SOURCE);
}

#[test]
fn plain_identifier_naming_a_constant_is_not_a_fresh_binding() {
    const SOURCE: &str = "
        const LIMIT: i32 = 5;
        fn main() {
            let k = match 4_i32 { LIMIT => 1_i32, _ => 2_i32 };
            if k != 2_i32 { let z = 0_i32; let _ = 1_i32 / z; }
        }";
    // The source-selected constant runs through the ordinary initializer;
    // a runtime refusal is not evidence that its pattern worked.
    complete(SOURCE);
    // A function name in the Value namespace is shadowed by a new binding.
    complete(
        "fn a() {} fn main() { match 3_i32 { a => { if a != 3_i32 { let z = 0_i32; let _ = 1_i32 / z; } } } }",
    );
}

#[test]
fn array_and_string_literal_patterns_use_the_same_matcher() {
    const SOURCE: &str = r#"
        fn main() {
            let a = [1_i32, 2_i32, 3_i32];
            let [first, .., last] = a;
            if first + last != 4_i32 { let z = 0_i32; let _ = 1_i32 / z; }
            let k = match a { [1, 2, x] if x > 2_i32 => x, [..] => 0_i32 };
            if k != 3_i32 { let z = 0_i32; let _ = 1_i32 / z; }
            let s = "lrr";
            let j = match s { "rust" => 1_i32, "lrr" => 2_i32, _ => 3_i32 };
            if j != 2_i32 { let z = 0_i32; let _ = 1_i32 / z; }
        }"#;
    complete(SOURCE);
}

#[test]
fn nominal_constructor_patterns_share_the_matcher_across_owner_kinds() {
    const SOURCE: &str = "
        enum E { A, B(i32), C { x: u8 } }
        struct P { x: i32, y: i32 }
        struct T(i32, i32);
        struct U;
        fn classify(e: E) -> i32 { match e { E::A => 1_i32, E::B(n) => n, E::C { x } => x as i32 } }
        fn main() {
            if classify(E::A) != 1_i32 || classify(E::B(7_i32)) != 7_i32 { let z = 0_i32; let _ = 1_i32 / z; }
            let b = E::B(5_i32);
            if let E::B(v) = b { if v != 5_i32 { let z = 0_i32; let _ = 1_i32 / z; } }
            let E::B(w) = E::B(6_i32) else { return; };
            let p = P { x: 1_i32, y: 2_i32 };
            let P { x, .. } = p;
            let t = T(3_i32, 4_i32);
            let T(a, b) = t;
            let k = match U { U => 1_i32 };
            if w + x + a + b + k != 15_i32 { let z = 0_i32; let _ = 1_i32 / z; }
            let r = &E::B(9_i32);
            match r { E::B(n) => { if *n != 9_i32 { let z = 0_i32; let _ = 1_i32 / z; } } _ => {} }
        }";
    complete(SOURCE);
    let steps = lowered_pattern_operations(SOURCE);
    assert!(steps.iter().any(|step| matches!(step,
        PatternOperation::Test { tree, .. } if !tree.names().is_empty())));
}

#[test]
fn glob_imported_variants_are_paths_not_fresh_bindings() {
    const SOURCE: &str = "
        enum E { A, B(i32) }
        use E::*;
        fn main() {
            let s = match A { B(_) => 2_i32, A => 1_i32 };
            if s != 1_i32 { let z = 0_i32; let _ = 1_i32 / z; }
            let r = match B(4_i32) { A => 0_i32, B(n) => n };
            if r != 4_i32 { let z = 0_i32; let _ = 1_i32 / z; }
        }";
    complete(SOURCE);
}
