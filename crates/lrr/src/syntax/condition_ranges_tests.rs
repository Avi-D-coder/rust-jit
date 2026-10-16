//! Source/phase and interpreter controls. Originals remain unchanged; adapters
//! below are explicitly LRR components, never substitute whole-fixture passes.
use super::*;
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database},
    engine::{
        run::{RunMachine, RunPoll, RunReport, ValueSummary},
        standalone_macros::CompletedStandaloneRuntimeMacros,
    },
    expand::{
        source_declarative,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
use std::sync::Arc;
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/match/match-on-negative-integer-ranges.rs"
);
fn entered(text: &str, edition: u16) -> (ParsedFile, BodyHandle, BodyIndex, ast::LetExpr) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"LRR ordinary let scalar source"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .clone()
        .unwrap();
    let index = parsed.index_body(&body, Default::default()).unwrap();
    let condition = parsed
        .entered_body_root(&body)
        .unwrap()
        .descendants()
        .find_map(ast::LetExpr::cast)
        .unwrap();
    (parsed, body, index, condition)
}
fn recipe(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    condition: &ast::LetExpr,
    phase: Phase,
) -> VerifiedRecipe {
    let id = id_for(index, body, condition.syntax()).unwrap();
    parsed
        .initial_thunk(body, index, &index.pc(id, phase).unwrap())
        .unwrap()
}
fn operand(index: &BodyIndex, body: &BodyHandle, condition: &ast::LetExpr) -> Phase {
    let id = id_for(index, body, condition.syntax()).unwrap();
    let scrutinee = id_for(index, body, condition.expr().unwrap().syntax()).unwrap();
    Phase::Operand(
        u16::try_from(
            index.nodes()[id.0 as usize]
                .operands
                .iter()
                .position(|node| *node == scrutinee)
                .unwrap(),
        )
        .unwrap(),
    )
}
fn operation(recipe: &VerifiedRecipe, operator: NumericBinary) -> bool {
    recipe
        .recipe()
        .operations
        .iter()
        .any(|op| matches!(op,Op::NumericBinary{operator:actual,..} if *actual==operator))
}
#[test]
fn original_signed_if_let_range_component_preserves_bounds_and_lazy_cleanup_all_editions() {
    // Exact unchanged clause text, adapted into an ordinary body OUTSIDE the
    // original because its actual invocation is an opaque assert_eq argument.
    let clause = ORIGINAL
        .split("if let ")
        .nth(1)
        .unwrap()
        .split(" { true } else { false }")
        .next()
        .unwrap();
    let text = format!("fn main(){{let _=if let {clause} {{true}}else{{false}};}}");
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index, condition) = entered(&text, edition);
        let lower = recipe(
            &parsed,
            &body,
            &index,
            &condition,
            operand(&index, &body, &condition),
        );
        assert!(operation(&lower, NumericBinary::Ge));
        assert!(lower.recipe().operations.iter().any(|op|matches!(op,Op::Literal{literal:LiteralRecipe{spelling,kind:LiteralKind::NegativeInteger,..},..} if spelling=="-128i8")));
        assert!(
            matches!(&lower.recipe().exit,RecipeExit::Branch{yes,no,..} if Phase::from_code(yes.phase)==Phase::Commit && Phase::from_code(no.phase)==Phase::Exit)
        );
        let upper = recipe(&parsed, &body, &index, &condition, Phase::Commit);
        assert!(operation(&upper, NumericBinary::Le));
        assert!(upper.recipe().operations.iter().any(|op|matches!(op,Op::Literal{literal:LiteralRecipe{spelling,kind:LiteralKind::NegativeInteger,..},..} if spelling=="-101i8")));
        let id = id_for(&index, &body, condition.syntax()).unwrap();
        let destination = index.nodes()[id.0 as usize].value.unwrap();
        assert!(
            matches!(upper.recipe().operations.first(),Some(Op::Discard{source}) if *source==destination)
        );
        let exit = recipe(&parsed, &body, &index, &condition, Phase::Exit);
        assert!(matches!(
            &exit.recipe().operations[..],
            [
                Op::Pattern {
                    operation: PatternOperation::Finish { .. }
                },
                Op::Discard { .. }
            ]
        ));
    }
}
#[test]
fn ordinary_literals_open_ranges_and_existing_bool_have_real_source_recipes_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in [
            ("fn main(){if let 2u8=2u8{}}", NumericBinary::Eq),
            ("fn main(){if let 'a'='a'{}}", NumericBinary::Eq),
            ("fn main(){if let true=true{}}", NumericBinary::Eq),
            ("fn main(){if let 2..=3=2i32{}}", NumericBinary::Ge),
            ("fn main(){if let 2..=3=2u8{}}", NumericBinary::Ge),
            ("fn main(){if let ..3=2i32{}}", NumericBinary::Lt),
            ("fn main(){if let ..=3=2i32{}}", NumericBinary::Le),
            ("fn main(){if let 2.0..=3.0=2.0f32{}}", NumericBinary::Ge),
            ("fn main(){if let 2.0..=3.0=2.0f64{}}", NumericBinary::Ge),
            ("fn main(){if let 'a'..='z'='m'{}}", NumericBinary::Ge),
        ] {
            let (parsed, body, index, condition) = entered(text, edition);
            let lower = recipe(
                &parsed,
                &body,
                &index,
                &condition,
                operand(&index, &body, &condition),
            );
            assert!(operation(&lower, expected));
        }
    }
}
#[test]
fn upper_source_frontier_is_deferred_and_parameter_source_is_checked_before_owning_read() {
    let (parsed, body, index, condition) = entered("fn main(x:i32){if let 10..=UNKNOWN=x{}}", 2024);
    let entry = recipe(&parsed, &body, &index, &condition, Phase::Entry);
    assert!(entry.recipe().operations.iter().any(|op| matches!(
        op,
        Op::SetHint {
            hint: HintRecipe::DefinitionParameter(0),
            ..
        }
    )));
    assert!(operation(
        &recipe(
            &parsed,
            &body,
            &index,
            &condition,
            operand(&index, &body, &condition)
        ),
        NumericBinary::Ge
    ));
    let upper = recipe(&parsed, &body, &index, &condition, Phase::Commit);
    assert!(
        matches!(upper.recipe().operations.first(),Some(Op::Unsupported(error)) if error.reached_unsupported().code=="LRR-E-UNSUPPORTED")
    );
    let exit = recipe(&parsed, &body, &index, &condition, Phase::Exit);
    assert!(matches!(
        &exit.recipe().operations[..],
        [
            Op::Pattern {
                operation: PatternOperation::Finish { .. }
            },
            Op::Discard { .. }
        ]
    ));
    for text in [
        "type i32=bool;fn main(){let x:i32=true;if let 1..=3=x{}}",
        "fn main(){let x=&1i32;if let 1..=3=x{}}",
    ] {
        let (parsed, body, index, condition) = entered(text, 2024);
        let entry = recipe(&parsed, &body, &index, &condition, Phase::Entry);
        assert!(
            entry
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::Unsupported(_)))
        );
        assert!(
            !entry
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::ReadLocal { .. } | Op::ReadParameter { .. }))
        );
    }
}
fn runtime(text: &str, path: &str, edition: u16) -> RunReport {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new(path).unwrap();
    let unit = Digest::of_bytes(path.as_str().as_bytes());
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
            profile: profile.clone(),
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("actual original source absent");
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        view.outline().root_scope.clone(),
        Namespace::Value,
        unit,
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR ordinary condition owning compiler"),
        source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&profile).unwrap()),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::NeedExpansion { request } => {
                discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap();
            }
            _ => panic!("condition requires additional genuine source preparation"),
        }
    }
    let macros = CompletedStandaloneRuntimeMacros::from_discovery(&view, &discovery).unwrap();
    let contexts = macros.contexts().unwrap();
    db.begin_run().unwrap();
    let mut run = RunMachine::start_with_discovery(&mut db, body, 100_000, discovery)
        .into_ready()
        .unwrap()
        .unwrap();
    let outcome = run.poll_with_runtime_macros(&mut db, &contexts);
    db.finish_run().unwrap();
    let report = match outcome {
        RunPoll::Completed(report) => report,
        RunPoll::Fault(error) => panic!("ordinary condition fault: {error}"),
        RunPoll::Panicked(report) => panic!("ordinary condition panic: {report:?}"),
        RunPoll::NeedInput(input) => panic!("ordinary condition needs {input:?}"),
    };
    assert_eq!(report.live_edition, edition);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
    report
}
#[test]
fn scalar_condition_runtime_components_have_actual_branch_results_and_cold_upper_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (source, expected, comparisons) in [
            (
                "fn main()->bool{if let 1..=3=2i32{true}else{false}}",
                true,
                2,
            ),
            (
                "fn main()->bool{if let 1..3=3i32{true}else{false}}",
                false,
                2,
            ),
            (
                "fn main()->bool{if let 10..=UNKNOWN=1i32{true}else{false}}",
                false,
                1,
            ),
            (
                "fn main()->bool{if let 3..=6=2i32{true}else{false}}",
                false,
                1,
            ),
            ("fn main()->bool{if let ..3=2i32{true}else{false}}", true, 1),
            (
                "fn main()->bool{if let 2..=3=2u8{true}else{false}}",
                true,
                2,
            ),
            (
                "fn main()->bool{if let 'a'..='z'='m'{true}else{false}}",
                true,
                2,
            ),
            (
                "fn main()->bool{if let 1.0f32..=3.0f32=2.0f32{true}else{false}}",
                true,
                2,
            ),
        ] {
            let report = runtime(source, "LRR-condition-component.rs", edition);
            assert!(matches!(report.value,ValueSummary::Bool(value) if value==expected));
            assert_eq!(report.opcode_counters.started_by_opcode[10], comparisons);
        }
    }
}
#[test]
fn unchanged_negative_range_original_whole_main_requires_real_completion_all_editions() {
    // The first assertion also requires generated MatchExpr; the second
    // requires generated IfExpr/LetExpr argument routing. Neither is skipped.
    for edition in [2015, 2018, 2021, 2024] {
        let report = runtime(
            ORIGINAL,
            "tests/ui/match/match-on-negative-integer-ranges.rs",
            edition,
        );
        assert!(matches!(report.value, ValueSummary::Unit));
        assert_eq!(report.opcode_counters.started_by_opcode[26], 2);
    }
}

#[test]
fn nested_constant_and_static_conditions_cannot_select_outer_function_parameter_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (text, name) in [
            (
                "fn outer(x:i32){const C:bool=if let 1=x{true}else{false};}",
                "C",
            ),
            (
                "fn outer(x:i32){static S:bool=if let 1=x{true}else{false};}",
                "S",
            ),
        ] {
            let parsed = ParsedFile::parse(
                &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
                ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            let cfg = CfgSet::guest_default();
            let outline = parsed
                .outline(
                    Digest::of_bytes(b"LRR condition original owner boundary"),
                    vec![],
                    &cfg,
                )
                .unwrap();
            let owner = outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "outer")
                .unwrap();
            let facts = parsed
                .entered_macro_facts(owner.body.as_ref().unwrap(), &cfg, &owner.scope)
                .unwrap();
            let body = facts
                .items
                .iter()
                .find(|row| row.item.def_key.declared_name == name)
                .unwrap()
                .item
                .body
                .clone()
                .unwrap();
            let index = parsed.index_body(&body, Default::default()).unwrap();
            let condition = parsed
                .entered_body_root(&body)
                .unwrap()
                .descendants()
                .find_map(ast::LetExpr::cast)
                .unwrap();
            let entry = recipe(&parsed, &body, &index, &condition, Phase::Entry);
            assert!(entry.recipe().operations.iter().any(|op|matches!(op,Op::Unsupported(error) if error.reached_unsupported().code=="LRR-E-UNSUPPORTED" && error.message=="condition parameter belongs to another source body")));
            assert!(!entry.recipe().operations.iter().any(|op| matches!(
                op,
                Op::SetHint {
                    hint: HintRecipe::DefinitionParameter(_),
                    ..
                } | Op::ReadParameter { .. }
            )));
        }
        let (parsed, body, index, condition) = entered("fn main(x:u8){if let 1=x{}}", edition);
        let entry = recipe(&parsed, &body, &index, &condition, Phase::Entry);
        assert!(entry.recipe().operations.iter().any(|op| matches!(
            op,
            Op::SetHint {
                hint: HintRecipe::DefinitionParameter(0),
                ..
            }
        )));
    }
}
