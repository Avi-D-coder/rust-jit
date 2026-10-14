use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    frontend::constants::{ConstLimits, ConstValue, evaluate},
    runtime::{shape::TypeId, value::ValueRepr},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{pure_constants::ConstantSource, spans::SourceRange},
};
fn source(text: &str, edition: u16) -> ConstantSource {
    let mut profile = ProfileIdentity::current();
    profile.edition = edition;
    ConstantSource::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        profile,
        Digest([174; 32]),
        CfgSet::guest_default(),
    )
    .unwrap()
}
fn at(text: &str, spelling: &str) -> SourceRange {
    let start = text.find(spelling).unwrap();
    SourceRange {
        start: u32::try_from(start).unwrap(),
        end: u32::try_from(start + spelling.len()).unwrap(),
    }
}
fn declaration(source: &ConstantSource, name: &str) -> SourceRange {
    source
        .declarations()
        .iter()
        .find(|d| d.name() == name)
        .unwrap()
        .range()
}
fn scalar(value: &ConstValue) -> ValueRepr {
    value.scalar().unwrap()
}
#[test]
fn pure_stock_array_lengths_keep_four_owning_editions() {
    let arithmetic = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/consts/arithmetic-expr-in-array-len.rs"
    );
    let bool_len = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/consts/bool-comparison-in-const-array-len.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expr, expected) in [
            (arithmetic, "2*4", 8),
            (bool_len, "((1 < 2) == false) as usize", 0),
        ] {
            let src = source(text, edition);
            let plan = src.array_length(at(text, expr)).unwrap();
            let result = evaluate(&plan, ConstLimits::default()).unwrap();
            assert_eq!(
                scalar(result.value()),
                ValueRepr::unsigned(TypeId(13), expected).unwrap()
            );
            assert_eq!(result.source(), Digest::of_bytes(text.as_bytes()));
            assert_eq!(plan.profile().edition, edition);
            assert_eq!(
                result.admit_array_length(8, 8, 64).unwrap(),
                usize::try_from(expected).unwrap()
            );
            if expected != 0 {
                assert!(result.admit_array_length(8, 7, 64).is_err());
                assert!(result.admit_array_length(8, 8, 63).is_err());
            }
        }
    }
}
#[test]
fn pure_stock_exact_constants_and_recursive_gcd_remain_partial() {
    let text = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/consts/control-flow/basics.rs"
    );
    let src = source(text, 2024);
    let plan = src.constant(declaration(&src, "ABS_DIFF")).unwrap();
    let result = evaluate(&plan, ConstLimits::default()).unwrap();
    assert_eq!(
        scalar(result.value()),
        ValueRepr::unsigned(TypeId(11), 1).unwrap()
    );
    assert_eq!(result.dependencies().len(), 3);
    let args = [48, 18]
        .map(|n| ConstValue::from_scalar(ValueRepr::unsigned(TypeId(11), n).unwrap()).unwrap());
    let call = src
        .const_call(declaration(&src, "gcd"), args.to_vec())
        .unwrap();
    assert_eq!(
        scalar(evaluate(&call, ConstLimits::default()).unwrap().value()),
        ValueRepr::unsigned(TypeId(11), 6).unwrap()
    );
    // main/abs_diff/fib/is_prime and compiler diagnostics are unexecuted.
}
#[test]
fn pure_stock_cycle_and_reached_panic_have_exact_source_attribution() {
    let text = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/consts/recursive-const-stack-overflow.rs"
    );
    let src = source(text, 2024);
    let plan = src.constant(declaration(&src, "FOO")).unwrap();
    let error = evaluate(&plan, ConstLimits::default()).unwrap_err();
    assert_eq!(error.diagnostic().code, "LRR-E-CONST-CYCLE");
    assert_eq!(error.source(), Digest::of_bytes(text.as_bytes()));
    let text = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/consts/issue-66693-panic-in-array-len.rs"
    );
    let src = source(text, 2015);
    // The earlier diagnostic comment also spells panic!(); select the exact
    // original repeat operand rather than its first textual occurrence.
    let repeat = at(text, "[false; panic!()]");
    let operand = SourceRange {
        start: repeat.start + 8,
        end: repeat.end - 1,
    };
    assert_eq!(
        &text[operand.start as usize..operand.end as usize],
        "panic!()"
    );
    assert_ne!(operand, at(text, "panic!()"));
    let plan = src.array_length(operand).unwrap();
    let error = evaluate(&plan, ConstLimits::default()).unwrap_err();
    assert_eq!(error.diagnostic().code, "LRR-E-CONST-PANIC");
    assert_eq!(error.range(), operand);
}
#[test]
fn pure_cold_effects_short_circuit_and_reached_refusals_are_separate() {
    let text = "fn io()->i32{0} const GOOD:i32=if true {7}else{io()}; const BAD:i32=io(); const LAZY:bool=false && missing();";
    let src = source(text, 2024);
    for (name, value) in [
        ("GOOD", ValueRepr::signed(TypeId(6), 7).unwrap()),
        ("LAZY", ValueRepr::boolean(false)),
    ] {
        let plan = src.constant(declaration(&src, name)).unwrap();
        assert_eq!(
            scalar(evaluate(&plan, ConstLimits::default()).unwrap().value()),
            value
        );
    }
    let error = evaluate(
        &src.constant(declaration(&src, "BAD")).unwrap(),
        ConstLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.diagnostic().code, "LRR-E-CONST-EFFECT");
    let start = u32::try_from(text.rfind("io()").unwrap()).unwrap();
    assert_eq!(
        error.range(),
        SourceRange {
            start,
            end: start + 4
        }
    );
}
#[test]
fn pure_checked_arithmetic_and_resource_limits_do_not_publish_values() {
    for (text, code) in [
        ("const A:u8=255+1;", "LRR-E-CONST-PANIC"),
        ("const A:i32=1/0;", "LRR-E-CONST-PANIC"),
    ] {
        let src = source(text, 2024);
        assert_eq!(
            evaluate(
                &src.constant(declaration(&src, "A")).unwrap(),
                ConstLimits::default()
            )
            .unwrap_err()
            .diagnostic()
            .code,
            code
        );
    }
    let src = source("const A:i32=1+2;", 2024);
    let plan = src.constant(declaration(&src, "A")).unwrap();
    let low = ConstLimits::new(1, 128, 1 << 20).unwrap();
    assert_eq!(
        evaluate(&plan, low).unwrap_err().diagnostic().code,
        "LRR-E-CONST-LIMIT"
    );
    assert!(ConstLimits::new(100001, 128, 1 << 20).is_err());
    let src = source("const fn recurse(n:u32)->u32{recurse(n+1)}", 2024);
    let args = vec![ConstValue::from_scalar(ValueRepr::unsigned(TypeId(11), 0).unwrap()).unwrap()];
    let plan = src.const_call(declaration(&src, "recurse"), args).unwrap();
    assert_eq!(
        evaluate(&plan, ConstLimits::new(100000, 3, 1 << 20).unwrap())
            .unwrap_err()
            .diagnostic()
            .code,
        "LRR-E-CONST-LIMIT"
    );
}
#[test]
fn pure_private_identity_immutable_arrays_and_namespace_controls() {
    let text = "const A:[u8;3]=[5;3];";
    let src = source(text, 2024);
    let plan = src.constant(declaration(&src, "A")).unwrap();
    let first = evaluate(&plan, ConstLimits::default()).unwrap();
    let second = evaluate(&plan, ConstLimits::default()).unwrap();
    assert_eq!(first.value().elements().unwrap().len(), 3);
    assert_eq!(first.value().fingerprint(), second.value().fingerprint());
    assert!(src.constant(at(text, "[5;3]")).is_err());
    assert!(
        ConstValue::from_scalar(ValueRepr {
            kind: 8,
            type_id: 1,
            payload: 42
        })
        .is_err()
    );
    assert_eq!(
        evaluate(&plan, ConstLimits::new(100000, 128, 8).unwrap())
            .unwrap_err()
            .diagnostic()
            .code,
        "LRR-E-CONST-LIMIT"
    );
    for text in [
        "type u32=bool; const A:u32=true;",
        "mod u32{} const A:u32=3;",
        "use other::u32;const A:u32=3;",
    ] {
        let src = source(text, 2024);
        assert!(src.constant(declaration(&src, "A")).is_err());
    }
    let other = ConstantSource::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
        Digest([175; 32]),
        CfgSet::guest_default(),
    )
    .unwrap();
    assert_ne!(
        plan.identity(),
        other.constant(declaration(&other, "A")).unwrap().identity()
    );
    // No live guest arrays were created; fresh runtime instantiation is a root hook.
}

#[test]
fn pure_return_context_and_cold_later_statements_stay_lazy() {
    let text = "const fn first()->u32 {return 7; missing();} const fn wrong()->u32{return true;}";
    let src = source(text, 2024);
    let first = src.const_call(declaration(&src, "first"), vec![]).unwrap();
    assert_eq!(
        scalar(evaluate(&first, ConstLimits::default()).unwrap().value()),
        ValueRepr::unsigned(TypeId(11), 7).unwrap()
    );
    let wrong = src.const_call(declaration(&src, "wrong"), vec![]).unwrap();
    assert_eq!(
        evaluate(&wrong, ConstLimits::default())
            .unwrap_err()
            .diagnostic()
            .code,
        "LRR-E-CONST-TYPE"
    );
}

#[test]
fn pure_raw_names_and_disabled_imports_keep_source_identity() {
    let text = "#[cfg(any())] use elsewhere::u32; #[cfg(any())] type u32=bool; const r#first:u32=4; const SECOND:u32=first+1;";
    let src = source(text, 2024);
    let plan = src.constant(declaration(&src, "SECOND")).unwrap();
    assert_eq!(
        scalar(evaluate(&plan, ConstLimits::default()).unwrap().value()),
        ValueRepr::unsigned(TypeId(11), 5).unwrap()
    );
    let src = source("const A:i64={let n=1;n};", 2024);
    let plan = src.constant(declaration(&src, "A")).unwrap();
    assert_eq!(
        evaluate(&plan, ConstLimits::default())
            .unwrap_err()
            .diagnostic()
            .code,
        "LRR-E-CONST-UNSUPPORTED"
    );
}

#[test]
fn pure_local_value_and_callable_shadows_do_not_capture_root_definitions() {
    for (text, code) in [
        (
            "const fn same()->u32{4} const fn live(same:u32)->u32{same()}",
            "LRR-E-CONST-TYPE",
        ),
        (
            "const ROOT:u32=4; const LIVE:u32={ROOT;const ROOT:bool=true;4};",
            "LRR-E-CONST-SOURCE",
        ),
    ] {
        let src = source(text, 2024);
        let error = if text.contains("fn live") {
            let plan = src
                .const_call(
                    declaration(&src, "live"),
                    vec![
                        ConstValue::from_scalar(ValueRepr::unsigned(TypeId(11), 3).unwrap())
                            .unwrap(),
                    ],
                )
                .unwrap();
            evaluate(&plan, ConstLimits::default()).unwrap_err()
        } else {
            let plan = src.constant(declaration(&src, "LIVE")).unwrap();
            evaluate(&plan, ConstLimits::default()).unwrap_err()
        };
        assert_eq!(error.diagnostic().code, code);
    }
}

#[test]
fn pure_const_call_cycle_key_includes_immutable_arguments() {
    let src = source("const fn again(n:u32)->u32{again(n)}", 2024);
    let args = vec![ConstValue::from_scalar(ValueRepr::unsigned(TypeId(11), 1).unwrap()).unwrap()];
    let plan = src.const_call(declaration(&src, "again"), args).unwrap();
    assert_eq!(
        evaluate(&plan, ConstLimits::default())
            .unwrap_err()
            .diagnostic()
            .code,
        "LRR-E-CONST-CYCLE"
    );
}

#[test]
fn pure_anonymous_lengths_refuse_foreign_lexical_capture() {
    for text in [
        "const N:usize=4;fn live(N:usize){let _=[0;N];}",
        "const N:usize=4;fn live(){let N=2usize;let _=[0;N];}",
        "const N:usize=4;mod inner{const N:usize=2;type A=[u8;N];}",
    ] {
        let src = source(text, 2024);
        let start = u32::try_from(text.rfind(";N]").unwrap() + 1).unwrap();
        assert_eq!(
            src.array_length(SourceRange {
                start,
                end: start + 1
            })
            .unwrap_err()
            .code,
            "LRR-E-CONST-SOURCE"
        );
    }
}
