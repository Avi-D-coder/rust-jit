//! Source-plan controls; full original execution belongs to the xtask adapter.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::pc::{NodeId, Phase},
    source::snapshot::{CfgSet, SourceBlob},
};

fn entered(
    source: &str,
    edition: u16,
    kind: RuntimeMacroKind,
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
        .outline(Digest([232; 32]), vec![], &CfgSet::guest_default())
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
    let arguments = parsed.runtime_macro_arguments(&invocation, kind).unwrap();
    let key = index
        .pc(NodeId(u32::try_from(ordinal).unwrap()), Phase::Entry)
        .unwrap();
    (parsed, body, index, key, arguments)
}

#[test]
fn original_labelled_block_first_call_keeps_actual_source_and_all_owning_editions() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/for-loop-while/label_break_value.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index, key, arguments) =
            entered(ORIGINAL, edition, RuntimeMacroKind::AssertEq);
        let plan = parsed
            .runtime_macro_scalar_plan(&body, &index, &key, &arguments)
            .unwrap();
        let call = plan.call(0).unwrap();
        assert_eq!(call.name(), "label_break");
        assert!(call.explicit().is_empty());
        assert_eq!(call.arguments().len(), 2);
        assert_eq!(call.parent(), None);
        assert_ne!(call.arguments()[0], call.arguments()[1]);
        assert!(call.arguments().iter().all(|slot| *slot != call.result()));
        let span = call.span();
        assert_eq!(
            &ORIGINAL[(body.range.start + span.relative_start) as usize
                ..(body.range.start + span.relative_end) as usize],
            "label_break(true, false)"
        );
        assert_eq!(call.source().profile.edition, edition);
        assert_eq!(call.body(), &body.key());
        assert_ne!(call.identity(), Digest([0; 32]));
        call.require(
            &body.key(),
            call.prepare_key(),
            RuntimeMacroCallPhase::Prepare,
        )
        .unwrap();
        assert!(
            call.require(&body.key(), call.seal_key(), RuntimeMacroCallPhase::Prepare)
                .is_err()
        );
        assert_eq!(call.advance_key(1).unwrap(), call.seal_key());
        assert!(call.allocated_bytes().unwrap() > std::mem::size_of_val(call.as_ref()));
        assert!(call.argument_key(0).unwrap() != call.argument_key(1).unwrap());
        parsed
            .require_runtime_macro_call(&body, &index, call.prepare_key(), call)
            .unwrap();
        assert!(
            plan.steps()
                .iter()
                .all(|step| !matches!(step, RuntimeScalarStep::Fault(_)))
        );
    }
}

#[test]
fn terminal_bool_calls_have_real_resume_phase_and_zero_arguments_still_prepare() {
    for (source, count) in [
        (
            "fn predicate(x:bool)->bool{x}fn main(){assert!((predicate(true)));}",
            1,
        ),
        ("fn truth()->bool{true}fn main(){assert!((truth()));}", 0),
    ] {
        let (parsed, body, index, key, arguments) = entered(source, 2024, RuntimeMacroKind::Assert);
        let plan = parsed
            .runtime_macro_scalar_plan(&body, &index, &key, &arguments)
            .unwrap();
        let call = plan.call(0).unwrap();
        assert_eq!(call.arguments().len(), count);
        assert_ne!(call.prepare_key(), call.seal_key());
        assert_ne!(call.seal_key(), call.resume_key());
        assert!(matches!(
            plan.steps().last(),
            Some(RuntimeScalarStep::Operation(Op::SetHint {
                hint: HintRecipe::Primitive(PrimitiveType::Bool),
                ..
            }))
        ));
        assert!(
            plan.steps()
                .iter()
                .any(|step| matches!(step, RuntimeScalarStep::CallPrepare(0)))
        );
    }
}

#[test]
fn nested_call_order_precedes_later_unsupported_argument_without_callee_work() {
    let source = "fn outer(a:i32,b:i32)->i32{a}fn inner()->i32{1}fn main(){assert_eq!(outer(inner(), unknown.method()),1);}";
    let (parsed, body, index, key, arguments) = entered(source, 2024, RuntimeMacroKind::AssertEq);
    let plan = parsed
        .runtime_macro_scalar_plan(&body, &index, &key, &arguments)
        .unwrap();
    let parent = plan.call(0).unwrap();
    let child = plan.call(1).unwrap();
    assert_eq!(parent.name(), "outer");
    assert_eq!(child.name(), "inner");
    assert_eq!(child.parent(), Some(0));
    assert_eq!(child.result(), parent.arguments()[0]);
    let seal = plan
        .steps()
        .iter()
        .position(|step| matches!(step, RuntimeScalarStep::CallSeal(1)))
        .unwrap();
    let fault = plan
        .steps()
        .iter()
        .position(|step| matches!(step, RuntimeScalarStep::Fault(_)))
        .unwrap();
    assert!(seal < fault);
    let RuntimeScalarStep::Fault(error) = &plan.steps()[fault] else {
        unreachable!()
    };
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert!(
        plan.steps()[..fault]
            .iter()
            .any(|step| matches!(step, RuntimeScalarStep::CallAdvance(0, 0)))
    );
    assert!(child.argument_key(0).is_err());
}

#[test]
fn actual_lexical_shadow_and_other_source_cannot_admit_a_call_witness() {
    let source = "fn f()->i32{0}fn main(){let f=0;assert_eq!(f(),0);}";
    let (parsed, body, index, key, arguments) = entered(source, 2024, RuntimeMacroKind::AssertEq);
    let plan = parsed
        .runtime_macro_scalar_plan(&body, &index, &key, &arguments)
        .unwrap();
    let call = plan.call(0).unwrap();
    assert_eq!(
        parsed
            .require_runtime_macro_call(&body, &index, call.prepare_key(), call)
            .unwrap_err()
            .code,
        "LRR-E-UNSUPPORTED"
    );
    let (other, _, _, _, _) = entered(
        "fn f()->i32{1}fn main(){let f=0;assert_eq!(f(),0);}",
        2024,
        RuntimeMacroKind::AssertEq,
    );
    assert!(
        other
            .require_runtime_macro_call(&body, &index, call.prepare_key(), call)
            .is_err()
    );
}
