//! Source/component evidence only; original whole-main execution is separate.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::pc::{NodeId, Phase},
    source::snapshot::{CfgSet, SourceBlob},
};

fn entered(
    source: &str,
    edition: u16,
) -> (
    ParsedFile,
    BodyHandle,
    BodyIndex,
    PcKey,
    RuntimeMacroArguments,
) {
    entered_function(source, edition, "main")
}
fn entered_function(
    source: &str,
    edition: u16,
    function: &str,
) -> (
    ParsedFile,
    BodyHandle,
    BodyIndex,
    PcKey,
    RuntimeMacroArguments,
) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([193; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == function)
        .unwrap()
        .body
        .clone()
        .unwrap();
    let index = parsed.index_body(&body, Default::default()).unwrap();
    let (ordinal, node) = index
        .nodes()
        .iter()
        .enumerate()
        .find(|(_, node)| node.role == NodeRole::Expression(ExpressionKind::Macro))
        .unwrap();
    let invocation = parsed
        .macro_call_at(super::super::spans::SourceRange {
            start: body.range.start + node.range.start,
            end: body.range.start + node.range.end,
        })
        .unwrap();
    let arguments = parsed
        .runtime_macro_arguments(&invocation, RuntimeMacroKind::AssertEq)
        .unwrap();
    let key = index
        .pc(NodeId(ordinal.try_into().unwrap()), Phase::Entry)
        .unwrap();
    (parsed, body, index, key, arguments)
}
fn step<'a>(plan: &'a RuntimeScalarPlan, key: &PcKey) -> &'a RuntimeScalarStep {
    let ordinal = match Phase::from_code(key.phase) {
        Phase::Entry => 0,
        Phase::Operand(n) => usize::from(n) + 1,
        _ => panic!("not a scalar phase"),
    };
    &plan.steps()[ordinal]
}
fn written<'a>(source: &'a str, body: &BodyHandle, span: &BodySpan) -> &'a str {
    &source[(body.range.start + span.relative_start) as usize
        ..(body.range.start + span.relative_end) as usize]
}
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-call.rs"
);
#[test]
fn unchanged_reference_call_arguments_keep_written_borrow_order_and_original_origins_all_editions()
{
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index, key, arguments) = entered(ORIGINAL, edition);
        let plan = parsed
            .runtime_macro_scalar_plan(&body, &index, &key, &arguments)
            .unwrap();
        let call = plan.call(0).unwrap();
        assert_eq!(call.name(), "has_two");
        assert_eq!(call.arguments().len(), 2);
        assert!(
            plan.steps()
                .iter()
                .all(|step| !matches!(step, RuntimeScalarStep::Fault(_)))
        );
        parsed
            .require_runtime_macro_call(&body, &index, call.prepare_key(), call)
            .unwrap();
        for (ordinal, expected) in [(0, "20"), (1, "2")] {
            let borrow = call.argument_borrow(ordinal).unwrap().unwrap();
            borrow.require(call, &body.key(), ordinal).unwrap();
            assert_eq!(borrow.argument_ordinal(), ordinal);
            assert_eq!(borrow.source(), &parsed.key());
            assert_eq!(borrow.source().profile.edition, edition);
            assert_eq!(borrow.argument_slot(), call.arguments()[ordinal as usize]);
            assert_eq!(borrow.access(), BorrowAccess::Shared);
            assert_eq!(
                written(ORIGINAL, &body, borrow.argument_span()),
                format!("&{expected}")
            );
            assert_eq!(
                written(ORIGINAL, &body, borrow.borrow_span()),
                format!("&{expected}")
            );
            assert_eq!(written(ORIGINAL, &body, borrow.operand_span()), expected);
            for phase in [
                borrow.hint_key().unwrap(),
                borrow.place_key(),
                borrow.borrow_key(),
            ] {
                let (owner, selected) = plan.argument_borrow_at(phase).unwrap().unwrap();
                assert_eq!(owner.identity(), call.identity());
                assert_eq!(selected, borrow);
            }
            assert!(
                plan.argument_borrow_at(call.advance_key(ordinal).unwrap())
                    .unwrap()
                    .is_none()
            );
            let value = borrow.temporary().unwrap();
            assert_eq!(borrow.place(), SlotId(value.0 + 1));
            assert!(
                matches!(step(&plan,borrow.hint_key().unwrap()),RuntimeScalarStep::Operation(Op::SetHint{child,hint:HintRecipe::BorrowTarget{reference}}) if *child==value && *reference==borrow.argument_slot())
            );
            assert!(
                matches!(step(&plan,borrow.operand_key().unwrap()),RuntimeScalarStep::Operation(Op::Literal{destination,literal,..}) if *destination==value && literal.spelling==expected)
            );
            assert!(
                matches!(step(&plan,borrow.place_key()),RuntimeScalarStep::Operation(Op::ValuePlace{source,destination}) if *source==value && *destination==borrow.place())
            );
            assert!(
                matches!(step(&plan,borrow.borrow_key()),RuntimeScalarStep::Operation(Op::Borrow{place,access:BorrowAccess::Shared,destination}) if *place==borrow.place() && *destination==borrow.argument_slot())
            );
            assert!(borrow.hint_key().unwrap().phase < borrow.place_key().phase);
            assert!(borrow.borrow_key().phase < call.advance_key(ordinal).unwrap().phase);
        }
        assert!(
            call.advance_key(0).unwrap().phase
                < call
                    .argument_borrow(1)
                    .unwrap()
                    .unwrap()
                    .hint_key()
                    .unwrap()
                    .phase
        );
        assert!(
            call.argument_borrow(0)
                .unwrap()
                .unwrap()
                .require(call, &body.key(), 1)
                .is_err()
        );
        assert!(call.argument_borrow(2).is_err());
    }
}
#[test]
fn borrowed_argument_fact_cannot_cross_original_source_or_owning_edition() {
    let (parsed, body, index, key, args) = entered(ORIGINAL, 2015);
    let plan = parsed
        .runtime_macro_scalar_plan(&body, &index, &key, &args)
        .unwrap();
    let call = plan.call(0).unwrap();
    let borrow = call.argument_borrow(0).unwrap().unwrap();
    let (other, other_body, other_index, other_key, other_args) = entered(ORIGINAL, 2024);
    let other_plan = other
        .runtime_macro_scalar_plan(&other_body, &other_index, &other_key, &other_args)
        .unwrap();
    assert!(
        borrow
            .require(other_plan.call(0).unwrap(), &other_body.key(), 0)
            .is_err()
    );
    assert!(
        other
            .require_runtime_macro_call(&other_body, &other_index, call.prepare_key(), call)
            .is_err()
    );
    let changed = ORIGINAL.replacen("&20", "&21", 1);
    let (other, other_body, other_index, other_key, other_args) = entered(&changed, 2015);
    let other_plan = other
        .runtime_macro_scalar_plan(&other_body, &other_index, &other_key, &other_args)
        .unwrap();
    assert!(
        borrow
            .require(other_plan.call(0).unwrap(), &other_body.key(), 0)
            .is_err()
    );
    assert_ne!(call.identity(), other_plan.call(0).unwrap().identity());
}
#[test]
fn local_reference_argument_uses_its_actual_binding_place_without_copy_read() {
    let source = "fn take(x:&i32,y:&mut i32)->i32{*x+*y}fn main(){let x=1;let mut y=2;assert_eq!(take((&x),&mut (y)),3);}";
    let (parsed, body, index, key, args) = entered(source, 2024);
    let plan = parsed
        .runtime_macro_scalar_plan(&body, &index, &key, &args)
        .unwrap();
    let call = plan.call(0).unwrap();
    assert!(
        plan.steps()
            .iter()
            .all(|step| !matches!(step, RuntimeScalarStep::Fault(_)))
    );
    for (argument, access) in [(0, BorrowAccess::Shared), (1, BorrowAccess::Mutable)] {
        let borrow = call.argument_borrow(argument).unwrap().unwrap();
        borrow.require(call, &body.key(), argument).unwrap();
        assert_eq!(borrow.access(), access);
        assert!(borrow.temporary().is_none());
        assert!(borrow.hint_key().is_none() && borrow.operand_key().is_none());
        assert!(
            matches!(step(&plan,borrow.place_key()),RuntimeScalarStep::Operation(Op::MakePlace{destination,..}) if *destination==borrow.place())
        );
        assert!(
            matches!(step(&plan,borrow.borrow_key()),RuntimeScalarStep::Operation(Op::Borrow{place,destination,access:actual}) if *place==borrow.place() && *destination==borrow.argument_slot() && *actual==access)
        );
    }
    assert!(
        !plan
            .steps()
            .iter()
            .any(|step| matches!(step, RuntimeScalarStep::Operation(Op::ReadPlace { .. })))
    );
}
#[test]
fn unsupported_borrow_form_is_deferred_after_earlier_argument_call() {
    for operand in ["&raw const p", "&[1]", "&p.0", "&*p"] {
        let source = format!(
            "fn earlier()->i32{{panic!()}}fn take(a:i32,b:i32)->i32{{a}}fn main(){{let p=1;assert_eq!(take(earlier(),{operand}),0);}}"
        );
        let (parsed, body, index, key, args) = entered(&source, 2024);
        let plan = parsed
            .runtime_macro_scalar_plan(&body, &index, &key, &args)
            .unwrap();
        let earlier = plan.call(1).unwrap();
        let fault=plan.steps().iter().position(|step|matches!(step,RuntimeScalarStep::Fault(error) if error.code=="LRR-E-UNSUPPORTED")).unwrap();
        let before_fault = match Phase::from_code(earlier.seal_key().phase) {
            Phase::Entry => 0,
            Phase::Operand(n) => usize::from(n) + 1,
            _ => panic!(),
        };
        assert!(before_fault < fault);
        assert!(plan.call(0).unwrap().argument_borrow(1).unwrap().is_none());
    }
    // Ordering component only: no callee or panic executes during planning.
}

#[test]
fn parameter_borrow_plan_addresses_actual_parameter_cell_without_copy_all_editions() {
    // LRR private-source authority control; whole stock runtime acceptance uses
    // the unchanged original fixture separately.
    const SOURCE: &str = "fn take(x:&isize)->isize{*x} fn caller(x:isize){assert_eq!(take(&x),0);}";
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index, key, arguments) = entered_function(SOURCE, edition, "caller");
        let plan = parsed
            .runtime_macro_scalar_plan(&body, &index, &key, &arguments)
            .unwrap();
        let call = plan.call(0).unwrap();
        let borrow = call.argument_borrow(0).unwrap().unwrap();
        borrow.require(call, &body.key(), 0).unwrap();
        assert!(borrow.temporary().is_none());
        assert!(borrow.hint_key().is_none());
        assert!(
            matches!(step(&plan,borrow.place_key()),RuntimeScalarStep::Operation(Op::ParameterPlace{ordinal:0,destination}) if *destination==borrow.place())
        );
        assert!(
            matches!(step(&plan,borrow.borrow_key()),RuntimeScalarStep::Operation(Op::Borrow{place,access:BorrowAccess::Shared,destination}) if *place==borrow.place() && *destination==call.arguments()[0])
        );
        assert!(
            plan.argument_borrow_at(borrow.place_key())
                .unwrap()
                .is_some()
        );
        assert!(plan.steps().iter().all(|step| !matches!(
            step,
            RuntimeScalarStep::Operation(Op::ReadParameter { .. } | Op::ReadLocal { .. })
                | RuntimeScalarStep::Fault(_)
        )));
    }
}
