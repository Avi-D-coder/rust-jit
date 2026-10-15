//! Pure source graph controls. No host evaluator or fixture-pass assertion.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{
        pc::{NodeId, Phase},
        recipe::RecipeExit,
    },
    source::snapshot::{CfgSet, SourceBlob},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/match/match-on-negative-integer-ranges.rs"
);
pub(super) fn entered(
    text: &str,
    edition: u16,
) -> (
    ParsedFile,
    BodyHandle,
    BodyIndex,
    Vec<(PcKey, RuntimeMacroArguments)>,
) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([217; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .clone()
        .unwrap();
    let index = parsed.index_body(&body, Default::default()).unwrap();
    let mut sites = Vec::new();
    for node in index
        .nodes()
        .iter()
        .filter(|n| n.role == NodeRole::Expression(ExpressionKind::Macro))
    {
        let call = parsed
            .macro_call_at(super::super::spans::SourceRange {
                start: body.range.start + node.range.start,
                end: body.range.start + node.range.end,
            })
            .unwrap();
        sites.push((
            index.pc(node.id, Phase::Entry).unwrap(),
            parsed
                .runtime_macro_arguments(&call, RuntimeMacroKind::AssertEq)
                .unwrap(),
        ));
    }
    (parsed, body, index, sites)
}
fn key(index: &BodyIndex, site: &PcKey, ordinal: usize) -> PcKey {
    index
        .pc(
            NodeId(site.node),
            if ordinal == 0 {
                Phase::Entry
            } else {
                Phase::Operand(u16::try_from(ordinal - 1).unwrap())
            },
        )
        .unwrap()
}
#[test]
fn both_unchanged_original_arguments_have_bounded_forward_source_branches_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index, sites) = entered(ORIGINAL, edition);
        assert_eq!(sites.len(), 2);
        for (site, arguments) in sites {
            let plan = parsed
                .runtime_macro_scalar_plan(&body, &index, &site, &arguments)
                .unwrap();
            let mut branches = 0;
            let mut minima = 0;
            assert!(
                plan.steps().len()
                    <= usize::from(index.nodes()[site.node as usize].runtime_scalar_phases)
            );
            for (ordinal, step) in plan.steps().iter().enumerate() {
                match step {
                    RuntimeScalarStep::Control(control) => {
                        let pc = key(&index, &site, ordinal);
                        control.require(&parsed.key(), &index, &pc).unwrap();
                        let source = &ORIGINAL[(body.range.start + control.span().relative_start)
                            as usize
                            ..(body.range.start + control.span().relative_end) as usize];
                        assert!(source.starts_with("match ") || source.starts_with("if let "));
                        let mut wrong = pc.clone();
                        wrong.phase = Phase::Commit.code().unwrap();
                        assert!(control.require(&parsed.key(), &index, &wrong).is_err());
                        let mut foreign = parsed.key();
                        foreign.profile.edition = if edition == 2024 { 2021 } else { 2024 };
                        assert!(control.require(&foreign, &index, &pc).is_err());
                        if matches!(control.recipe_exit(), RecipeExit::Branch { .. }) {
                            branches += 1;
                        }
                    }
                    RuntimeScalarStep::Operation(Op::Literal { literal, .. })
                        if literal.spelling == "-128i8" =>
                    {
                        minima += 1;
                        assert!(matches!(
                            literal.decoded.as_ref(),
                            Some(crate::ir::op::DecodedLiteral::Integer {
                                negative: true,
                                suffix: Some(PrimitiveType::I8),
                                ..
                            })
                        ));
                        assert_eq!(
                            &ORIGINAL[(body.range.start + literal.source.relative_start) as usize
                                ..(body.range.start + literal.source.relative_end) as usize],
                            "-128i8"
                        );
                    }
                    RuntimeScalarStep::Fault(error) => panic!(
                        "unchanged original source acquired an unsupported subtree: {}",
                        error.message
                    ),
                    _ => {}
                }
            }
            assert_eq!(branches, 2);
            assert_eq!(minima, 1);
        }
    }
}
#[test]
fn unknown_upper_guard_and_future_result_remain_source_positioned_fault_steps() {
    for text in [
        "fn main(){assert_eq!(false,match 9_i32{0_i32..=UNKNOWN=>true,_=>false});}",
        "fn main(){assert_eq!(true,match 1_i32{1_i32=>true,_=>loop{}});}",
        "fn main(){assert_eq!(false,match 9_i32{0_i32 if missing()=>true,_=>false});}",
    ] {
        let (parsed, body, index, sites) = entered(text, 2024);
        let (site, args) = &sites[0];
        let plan = parsed
            .runtime_macro_scalar_plan(&body, &index, site, args)
            .unwrap();
        let fault = plan
            .steps()
            .iter()
            .position(|step| matches!(step, RuntimeScalarStep::Fault(_)))
            .expect("needed unsupported branch was erased");
        let fault_pc = key(&index, site, fault);
        let span = plan
            .fault_span(&fault_pc)
            .expect("fault lost its original source origin");
        let written = &text[(body.range.start + span.relative_start) as usize
            ..(body.range.start + span.relative_end) as usize];
        assert!(
            written == "UNKNOWN" || written == "loop{}" || written == "if missing()",
            "unexpected actual fault source: {written}"
        );
        assert!(
            plan.steps()
                .iter()
                .take(fault)
                .any(|step| matches!(step, RuntimeScalarStep::Control(_))),
            "unsupported cold subtree must follow a real source branch"
        );
        assert!(
            plan.steps()
                .iter()
                .take(fault)
                .filter_map(|step| match step {
                    RuntimeScalarStep::Control(control) => Some(control.recipe_exit()),
                    _ => None,
                })
                .any(|exit| match exit {
                    RecipeExit::Next(pc) => controls_test_ordinal(&pc) > fault,
                    RecipeExit::Branch { yes, no, .. } =>
                        controls_test_ordinal(&yes) > fault || controls_test_ordinal(&no) > fault,
                    _ => false,
                }),
            "actual source branch must be able to skip the fault"
        );
    }
}
fn controls_test_ordinal(pc: &PcKey) -> usize {
    match Phase::from_code(pc.phase) {
        Phase::Entry => 0,
        Phase::Operand(n) => usize::from(n) + 1,
        _ => panic!(),
    }
}
#[test]
fn conditional_ref_result_cannot_mint_whole_argument_borrow_authority() {
    for text in [
        "fn take(x:&i32)->i32{*x}fn main(){assert_eq!(take(match true{true=>&20,_=>&2}),20);}",
        "fn take(x:&i32)->i32{*x}fn main(){assert_eq!(take(if true{&20}else{&2}),20);}",
    ] {
        let (parsed, body, index, sites) = entered(text, 2024);
        let (site, args) = &sites[0];
        let plan = parsed
            .runtime_macro_scalar_plan(&body, &index, site, args)
            .unwrap();
        assert!(plan.call(0).unwrap().argument_borrow(0).unwrap().is_none());
        assert!(
            plan.steps()
                .iter()
                .any(|step| matches!(step, RuntimeScalarStep::Fault(_)))
        );
    }
    for expression in ["&20", "(&20)", "((&20))"] {
        let text =
            format!("fn take(x:&i32)->i32{{*x}}fn main(){{assert_eq!(take({expression}),20);}}");
        let (parsed, body, index, sites) = entered(&text, 2024);
        let (site, args) = &sites[0];
        let plan = parsed
            .runtime_macro_scalar_plan(&body, &index, site, args)
            .unwrap();
        assert!(plan.call(0).unwrap().argument_borrow(0).unwrap().is_some());
    }
}

#[test]
fn opaque_match_exhaustion_needs_actual_source_proof_and_public_match_exit_stays_indexed() {
    use crate::ir::{
        recipe::{Origin, RecipeDraft},
        validate,
    };
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index, sites) = entered(
            "fn main(){assert_eq!(true,match false{true=>true});}",
            edition,
        );
        let (site, args) = &sites[0];
        let plan = parsed
            .runtime_macro_scalar_plan(&body, &index, site, args)
            .unwrap();
        let (ordinal, control) = plan
            .steps()
            .iter()
            .enumerate()
            .find_map(|(n, step)| match step {
                RuntimeScalarStep::Control(control) if control.exhaustion().is_some() => {
                    Some((n, control))
                }
                _ => None,
            })
            .unwrap();
        let proof = control.exhaustion().unwrap();
        let draft = RecipeDraft {
            key: key(&index, site, ordinal),
            origin: Origin {
                span: control.span().clone(),
                expansion_ancestry: vec![],
            },
            operations: vec![],
            exits: vec![RecipeExit::MatchNoArm],
            dependencies: vec![],
        };
        assert!(validate::verify(draft.clone(), &index).is_err());
        validate::verify_runtime_scalar_match_exhaustion(draft.clone(), &index, &proof).unwrap();
        let mut wrong = draft.clone();
        wrong.key = site.clone();
        assert!(validate::verify_runtime_scalar_match_exhaustion(wrong, &index, &proof).is_err());
        let mut wrong = draft.clone();
        wrong.origin.span.relative_end -= 1;
        assert!(validate::verify_runtime_scalar_match_exhaustion(wrong, &index, &proof).is_err());
        let mut wrong = draft.clone();
        wrong.operations.push(Op::Declaration);
        assert!(validate::verify_runtime_scalar_match_exhaustion(wrong, &index, &proof).is_err());
        let mut wrong = draft.clone();
        wrong.exits = vec![RecipeExit::Next(site.clone())];
        assert!(validate::verify_runtime_scalar_match_exhaustion(wrong, &index, &proof).is_err());
        let mut wrong = draft.clone();
        wrong.origin.expansion_ancestry.push(Digest([211; 32]));
        assert!(validate::verify_runtime_scalar_match_exhaustion(wrong, &index, &proof).is_err());
        let (other, otherbody, otherindex, _) =
            entered("fn main(){match false{true=>(),false=>()};}", edition);
        let node = otherindex
            .nodes()
            .iter()
            .find(|n| n.role == NodeRole::Expression(ExpressionKind::Match))
            .unwrap();
        let ordinary = RecipeDraft {
            key: otherindex.pc(node.id, Phase::Exit).unwrap(),
            origin: Origin {
                span: otherindex.span(node.id).unwrap(),
                expansion_ancestry: vec![],
            },
            operations: vec![],
            exits: vec![RecipeExit::MatchNoArm],
            dependencies: vec![],
        };
        validate::verify(ordinary.clone(), &otherindex).unwrap();
        assert!(
            validate::verify_runtime_scalar_match_exhaustion(ordinary, &otherindex, &proof)
                .is_err()
        );
        assert!(
            control
                .require(&other.key(), &otherindex, &key(&index, site, ordinal))
                .is_err()
        );
        assert_ne!(body.key(), otherbody.key());
    }
}
