use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    facade::macros::RuntimeMacroKind,
    frontend::{
        macro_names::PathLookup,
        runtime_macro_catalog::RuntimeMacroCatalog,
        runtime_macro_plan::{RuntimeMacroPreparation, StaticPanicLowering, prepare_runtime_macro},
    },
    ir::{
        op::Op,
        pc::{NodeId, Phase},
        recipe::{Origin, RecipeDraft, RecipeExit},
        validate::verify,
    },
    runtime::cell::GuestStore,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{ExpressionKind, NodeRole, ScopeId},
        parse::ParsedFile,
        spans::SourceRange,
    },
    vm::{
        frame::{Frame, FrameStatus},
        step::{StepOutcome, step},
    },
};
fn plan(
    text: &str,
) -> (
    ParsedFile,
    Box<lrr::frontend::runtime_macro_plan::RuntimeIntrinsicPlan>,
) {
    let source = format!("fn case(){{{text};}}");
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let call = parsed
        .macro_call_at(SourceRange {
            start: 10,
            end: 10 + text.len() as u32,
        })
        .unwrap();
    let catalog = RuntimeMacroCatalog::new(Digest([178; 32]), 2024).unwrap();
    let definition = catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap() == Some(RuntimeMacroKind::Panic))
        .unwrap()
        .clone();
    let RuntimeMacroPreparation::Intrinsic(plan) =
        prepare_runtime_macro(&parsed, call, PathLookup::Defined(definition), &catalog).unwrap()
    else {
        panic!("panic plan not prepared")
    };
    (parsed, plan)
}
#[test]
fn resolved_static_panic_plans_lower_to_verified_guest_outcomes_with_actual_messages() {
    for (call, message) in [
        ("renamed!()", "explicit panic"),
        ("renamed!(\"hello\\n 🦀\")", "hello\n 🦀"),
        ("renamed!(r#\"raw \\n\"#)", "raw \\n"),
        ("renamed!(\"\",)", ""),
    ] {
        let (parsed, plan) = plan(call);
        let StaticPanicLowering::Operation(op) = plan.lower_static_panic().unwrap() else {
            panic!("static message requested formatting")
        };
        let outline = parsed
            .outline(Digest([177; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let node = body
            .nodes()
            .iter()
            .find(|n| n.role == NodeRole::Expression(ExpressionKind::Macro))
            .unwrap()
            .id;
        let make = |node, operations, next| {
            verify(
                RecipeDraft {
                    key: body.pc(node, Phase::Entry).unwrap(),
                    origin: Origin {
                        span: body.span(node).unwrap(),
                        expansion_ancestry: vec![],
                    },
                    operations,
                    exits: vec![RecipeExit::Next(next)],
                    dependencies: vec![],
                },
                &body,
            )
            .unwrap()
        };
        let mut store = GuestStore::new(1, 1000, 1 << 20).unwrap();
        let mut frame = Frame::new(
            body.clone(),
            body.pc(NodeId(0), Phase::Entry).unwrap(),
            &mut store,
        )
        .unwrap();
        let enter = make(
            NodeId(0),
            vec![Op::ScopeEnter { scope: ScopeId(0) }],
            body.pc(node, Phase::Entry).unwrap(),
        );
        step(&mut frame, &mut store, &enter).unwrap();
        let panic = make(node, vec![*op], body.pc(NodeId(0), Phase::Exit).unwrap());
        let StepOutcome::GuestPanic(report) = step(&mut frame, &mut store, &panic).unwrap() else {
            panic!("lowered operation did not panic")
        };
        assert_eq!(report.description(), message);
        assert_eq!(report.origin.span, body.span(node).unwrap());
        assert_eq!(frame.counters().started_by_opcode[25], 1);
        frame.finish_panic(&mut store).unwrap();
        assert_eq!(frame.status(), FrameStatus::PanicCleaned);
    }
}
#[test]
fn static_panic_lowering_preserves_formatting_demands_without_evaluating_arguments() {
    for call in [
        "renamed!(\"{name}\")",
        "renamed!(\"{{escaped}}\")",
        "renamed!(\"{}\", unknown())",
        "renamed!(message())",
    ] {
        let (_, plan) = plan(call);
        assert!(matches!(
            plan.lower_static_panic().unwrap(),
            StaticPanicLowering::NeedsFormatting
        ));
    }
}
#[test]
fn static_panic_lowering_refuses_oversized_payload_and_nonpanic_intrinsic() {
    let (_, plan) = plan(&format!("renamed!(\"{}\")", "x".repeat(65_537)));
    assert_eq!(
        plan.lower_static_panic().unwrap_err().code,
        "LRR-E-METADATA-LIMIT"
    );
    let source = "fn case(){assert!(true);}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let call = parsed
        .macro_call_at(SourceRange { start: 10, end: 23 })
        .unwrap();
    let catalog = RuntimeMacroCatalog::new(Digest([178; 32]), 2024).unwrap();
    let definition = catalog
        .definitions()
        .find(|d| catalog.select(d).unwrap() == Some(RuntimeMacroKind::Assert))
        .unwrap()
        .clone();
    let RuntimeMacroPreparation::Intrinsic(plan) =
        prepare_runtime_macro(&parsed, call, PathLookup::Defined(definition), &catalog).unwrap()
    else {
        panic!("assert plan absent")
    };
    assert!(plan.lower_static_panic().is_err());
}
