//! Unchanged upstream control-target observations; supplementary constant execution.
//! Ordinary upstream mains and rustc diagnostic snapshots remain unexecuted here.
use super::*;
use crate::{
    frontend::constants::{ConstLimits, evaluate},
    runtime::{shape::TypeId, value::ValueRepr},
};
const RUN: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/for-loop-while/label_break_value.rs"
);
const CONTINUE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/label/label_break_value_continue.rs"
);
const BREAK: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/label/label_break_value_unlabeled_break.rs"
);
fn source(text: &str, edition: u16) -> ConstantSource {
    ConstantSource::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
        Digest([220; 32]),
        CfgSet::guest_default(),
    )
    .unwrap()
}
fn plan(text: &str, edition: u16) -> ConstantPlan {
    let source = source(text, edition);
    let at = source
        .declarations()
        .iter()
        .find(|d| d.name() == "A")
        .unwrap()
        .range();
    source.constant(at).unwrap()
}
fn number(text: &str, edition: u16) -> ValueRepr {
    evaluate(&plan(text, edition), ConstLimits::default())
        .unwrap()
        .value()
        .scalar()
        .unwrap()
}
#[test]
fn original_block_break_values_keep_actual_source_target_and_expression_ranges() {
    for edition in [2015, 2018, 2021, 2024] {
        let source = source(RUN, edition);
        let function = source
            .0
            .parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::Fn::cast)
            .find(|f| f.name().is_some_and(|n| n.text() == "break_value"))
            .unwrap();
        let block = function
            .syntax()
            .descendants()
            .filter_map(ast::BlockExpr::cast)
            .find(|b| b.label().is_some())
            .unwrap();
        let Node::Block(_, _, Some(target)) = source
            .node(&ExprRef(ast::Expr::BlockExpr(block.clone())))
            .unwrap()
        else {
            panic!("original labelled block did not retain its source target")
        };
        assert!(!target.is_loop());
        assert_eq!(target.range(), range(block.syntax().text_range()));
        let exits = block
            .syntax()
            .descendants()
            .filter_map(ast::BreakExpr::cast)
            .collect::<Vec<_>>();
        assert_eq!(exits.len(), 2);
        for exit in exits {
            let original = exit.expr().unwrap();
            let Node::Break(actual, Some(value)) =
                source.node(&ExprRef(ast::Expr::BreakExpr(exit))).unwrap()
            else {
                panic!("original break lost its value")
            };
            assert_eq!(actual, target);
            assert_eq!(value.range(), range(original.syntax().text_range()));
        }
        assert_eq!(source.source(), Digest::of_bytes(RUN.as_bytes()));
    }
}
#[test]
fn original_invalid_continues_preserve_three_exact_reached_target_refusals() {
    for edition in [2015, 2018, 2021, 2024] {
        let source = source(CONTINUE, edition);
        let exits = source
            .0
            .parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::ContinueExpr::cast)
            .collect::<Vec<_>>();
        assert_eq!(exits.len(), 3);
        for (exit, message) in exits.into_iter().zip([
            "unlabelled constant exit crosses a labelled block",
            "constant continue targets a labelled block",
            "unlabelled constant exit crosses a labelled block",
        ]) {
            let error = source
                .node(&ExprRef(ast::Expr::ContinueExpr(exit)))
                .unwrap_err();
            assert_eq!(error.code, "LRR-E-CONST-SOURCE");
            assert_eq!(error.message, message);
        }
    }
}
#[test]
fn original_unlabelled_breaks_cannot_cross_a_block_to_an_outer_loop() {
    for edition in [2015, 2018, 2021, 2024] {
        let source = source(BREAK, edition);
        let exits = source
            .0
            .parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::BreakExpr::cast)
            .collect::<Vec<_>>();
        assert_eq!(exits.len(), 2);
        for exit in exits {
            let error = source
                .node(&ExprRef(ast::Expr::BreakExpr(exit)))
                .unwrap_err();
            assert_eq!(error.code, "LRR-E-CONST-SOURCE");
            assert_eq!(
                error.message,
                "unlabelled constant exit crosses a labelled block"
            );
        }
    }
}
#[test]
fn labelled_break_values_use_result_context_and_skip_the_unreached_suffix() {
    for edition in [2015, 2018, 2021, 2024] {
        assert_eq!(
            number("const A:u8='b:{break 'b 7; missing()};", edition),
            ValueRepr::unsigned(TypeId(9), 7).unwrap()
        );
        assert_eq!(
            number("const A:u8='b:{9};", edition),
            ValueRepr::unsigned(TypeId(9), 9).unwrap()
        );
        assert_eq!(
            number(
                "const A:u8={let x='b:{if true {break 'b 7;} 9}; let y:u8=x; y};",
                edition
            ),
            ValueRepr::unsigned(TypeId(9), 7).unwrap()
        );
    }
}
#[test]
fn nested_same_spelling_blocks_unwind_only_the_selected_activation_and_local_scope() {
    let text = "const A:i32={let mut x:i32=1; let y:i32='b:{let x:i32=99; let z:i32='b:{break 'b 3;}; break 'b z;}; x+=y; x};";
    for edition in [2015, 2018, 2021, 2024] {
        assert_eq!(
            number(text, edition),
            ValueRepr::signed(TypeId(6), 4).unwrap()
        );
        assert_eq!(
            number(
                "const A:i32='outer:{let mut x:i32=0; loop {x+=1; 'inner:{break 'outer x;}}};",
                edition
            ),
            ValueRepr::signed(TypeId(6), 1).unwrap()
        );
    }
}
#[test]
fn loop_continue_can_cross_a_block_only_with_the_actual_outer_loop_label() {
    let text = "const A:i32={let mut x:i32=0; 'outer:while x<3 {'block:{x+=1; continue 'outer;} missing();} x};";
    for edition in [2015, 2018, 2021, 2024] {
        assert_eq!(
            number(text, edition),
            ValueRepr::signed(TypeId(6), 3).unwrap()
        );
        assert_eq!(
            number("const A:i32='block:{loop {break;} 5};", edition),
            ValueRepr::signed(TypeId(6), 5).unwrap()
        );
    }
}
#[test]
fn block_target_errors_stay_reached_and_keep_the_exact_source_range() {
    for edition in [2015, 2018, 2021, 2024] {
        assert_eq!(
            number(
                "const A:i32={let x=9; if false {'b:{continue 'b;}} x};",
                edition
            ),
            ValueRepr::signed(TypeId(6), 9).unwrap()
        );
        for (text, fragment, message) in [
            (
                "const A:i32='b:{continue 'b; 9};",
                "continue 'b",
                "constant continue targets a labelled block",
            ),
            (
                "const A:i32=loop {'b:{break;} };",
                "break",
                "unlabelled constant exit crosses a labelled block",
            ),
        ] {
            let error = evaluate(&plan(text, edition), ConstLimits::default()).unwrap_err();
            assert_eq!(error.diagnostic().code, "LRR-E-CONST-SOURCE");
            assert_eq!(error.diagnostic().message, message);
            let start = u32::try_from(text.find(fragment).unwrap()).unwrap();
            assert_eq!(
                error.range(),
                SourceRange {
                    start,
                    end: start + u32::try_from(fragment.len()).unwrap()
                }
            );
            assert_eq!(error.source(), Digest::of_bytes(text.as_bytes()));
        }
    }
}
#[test]
fn return_propagates_through_labelled_blocks_without_becoming_a_block_value() {
    let text = "const fn f()->i32{'b:{return 11;} missing()} const A:i32=f();";
    for edition in [2015, 2018, 2021, 2024] {
        assert_eq!(
            number(text, edition),
            ValueRepr::signed(TypeId(6), 11).unwrap()
        );
    }
}
#[test]
fn labelled_activations_remain_bounded_and_preserve_reached_value_type_errors() {
    for text in [
        "const A:u8='b:{break 'b true;};",
        "const A:i32='a: loop { 'a: { break 'a 3; } };",
    ] {
        let error = evaluate(&plan(text, 2024), ConstLimits::default()).unwrap_err();
        assert_eq!(error.diagnostic().code, "LRR-E-CONST-TYPE");
    }
    let mut text = String::from("const A:i32=");
    for _ in 0..130 {
        text.push_str("'b:{");
    }
    text.push('1');
    for _ in 0..130 {
        text.push('}');
    }
    text.push(';');
    let error = evaluate(&plan(&text, 2024), ConstLimits::default()).unwrap_err();
    assert_eq!(error.diagnostic().code, "LRR-E-CONST-LIMIT");
}
