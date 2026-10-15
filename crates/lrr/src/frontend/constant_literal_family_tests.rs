//! Reached LRR family/ordering controls; original fixtures remain unchanged.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    source::snapshot::{CfgSet, SourceBlob},
};
fn result(text: &str, edition: u16) -> Result<ConstEvaluation, ConstFailure> {
    let source = ConstantSource::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
        Digest([145; 32]),
        CfgSet::guest_default(),
    )
    .unwrap();
    let at = source
        .declarations()
        .iter()
        .find(|d| d.name() == "A")
        .unwrap()
        .range();
    evaluate(&source.constant(at).unwrap(), ConstLimits::default())
}
fn assert_type_at(text: &str, fragment: &str, edition: u16) {
    let error = result(text, edition).unwrap_err();
    let initializer = text.find('=').unwrap() + 1;
    let start = initializer + text[initializer..].find(fragment).unwrap();
    assert_eq!(error.diagnostic().code, "LRR-E-CONST-TYPE");
    assert_eq!(
        error.range(),
        SourceRange {
            start: start as u32,
            end: (start + fragment.len()) as u32
        }
    );
    assert_eq!(error.source(), Digest::of_bytes(text.as_bytes()));
    assert!(error.numeric_panic().is_none());
    assert!(error.panic().is_none());
}
#[test]
fn reached_integer_rejects_bool_char_unit_and_float_context_at_its_source_literal() {
    for edition in [2015, 2018, 2021, 2024] {
        for ty in ["bool", "char", "()", "f32", "f64"] {
            assert_type_at(&format!("const A:{ty}=1;"), "1", edition);
        }
    }
}
#[test]
fn reached_float_rejects_bool_char_unit_and_integer_context_at_its_source_literal() {
    for edition in [2015, 2018, 2021, 2024] {
        for ty in ["bool", "char", "()", "i32", "u8", "usize"] {
            for spelling in ["1.0", "1.0f32", "1e400f32"] {
                assert_type_at(&format!("const A:{ty}={spelling};"), spelling, edition);
            }
        }
    }
}
#[test]
fn while_break_value_unit_mismatch_is_a_reached_constant_type_error() {
    for edition in [2015, 2018, 2021, 2024] {
        assert_type_at("const A:i32={while true{break 3;}5};", "3", edition);
    }
}
#[test]
fn correct_reached_scalar_literal_families_keep_their_exact_values() {
    for edition in [2015, 2018, 2021, 2024] {
        for (text, expected) in [
            ("const A:u8=7;", ValueRepr::unsigned(TypeId(9), 7).unwrap()),
            (
                "const A:i32=-2147483648;",
                ValueRepr::signed(TypeId(6), -2147483648).unwrap(),
            ),
            ("const A:f32=1.0;", ValueRepr::f32(1.0)),
            ("const A:f64=1.0;", ValueRepr::f64(1.0)),
            ("const A:bool=true;", ValueRepr::boolean(true)),
            ("const A:char='x';", ValueRepr::character('x')),
            ("const A:()=();", ValueRepr::unit()),
        ] {
            let value = result(text, edition).unwrap();
            assert_eq!(value.value().scalar().unwrap(), expected);
            assert_eq!(value.source(), Digest::of_bytes(text.as_bytes()));
        }
    }
}
#[test]
fn suffix_integer_validity_and_numeric_panic_keep_the_existing_reached_order() {
    for edition in [2015, 2018, 2021, 2024] {
        let error = result("const A:bool=1u8;", edition).unwrap_err();
        assert_eq!(error.diagnostic().code, "LRR-E-CONST-TYPE");
        assert_eq!(
            error.diagnostic().message,
            "constant integer suffix conflicts with context"
        );
        assert_type_at("const A:bool=1u8;", "1u8", edition);
        let invalid = result("const A:bool=1bad;", edition).unwrap_err();
        assert_eq!(invalid.diagnostic().code, "LRR-E-CONST-TYPE");
        assert_eq!(
            invalid.diagnostic().message,
            "invalid constant integer spelling"
        );
        assert_type_at("const A:bool=1bad;", "1bad", edition);
        let float_invalid = result("const A:f32=1e400f32;", edition).unwrap_err();
        assert_eq!(float_invalid.diagnostic().code, "LRR-E-VALUE-REPR");
        let overflow = result("const A:u8=256;", edition).unwrap_err();
        assert_eq!(overflow.diagnostic().code, "LRR-E-VALUE-REPR");
        assert!(overflow.numeric_panic().is_none());
        let unsigned = result("const A:u8=-0u8;", edition).unwrap_err();
        assert_eq!(unsigned.diagnostic().code, "LRR-E-OP-TYPE");
        let panic = result("const A:i32=1/0;", edition).unwrap_err();
        assert_eq!(panic.diagnostic().code, "LRR-E-CONST-PANIC");
        assert_eq!(
            panic.numeric_panic(),
            Some(crate::vm::numeric::PanicKind::DivisionByZero)
        );
        assert_type_at("const A:bool=1/0;", "1", edition);
    }
}
#[test]
fn cold_literal_family_errors_are_not_checked_before_the_selected_branch() {
    for edition in [2015, 2018, 2021, 2024] {
        for text in [
            "const A:i32=if false{let bad:bool=1;0}else{9};",
            "const A:i32=if false{let bad:()=1.0;0}else{9};",
        ] {
            assert_eq!(
                result(text, edition).unwrap().value().scalar().unwrap(),
                ValueRepr::signed(TypeId(6), 9).unwrap()
            );
        }
    }
}
