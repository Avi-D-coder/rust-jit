//! T054 immutable cache-boundary supplements. Rust language values come from upstream.
use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    frontend::constants::{
        ConstLimits, ConstValue, evaluate,
        wire::{ConstantWireType, decode, encode},
    },
    ir::op::PrimitiveType,
    runtime::{shape::TypeId, value::ValueRepr},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{pure_constants::ConstantSource, spans::SourceRange},
};
fn value(v: ValueRepr) -> ConstValue {
    ConstValue::from_scalar(v).unwrap()
}
#[test]
fn immutable_wire_preserves_stock_const_values_and_owning_editions() {
    let text = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/consts/arithmetic-expr-in-array-len.rs"
    );
    let start = u32::try_from(text.find("2*4").unwrap()).unwrap();
    for edition in [2015, 2018, 2021, 2024] {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let source = ConstantSource::parse(
            &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            profile,
            Digest([174; 32]),
            CfgSet::guest_default(),
        )
        .unwrap();
        let plan = source
            .array_length(SourceRange {
                start,
                end: start + 3,
            })
            .unwrap();
        let result = evaluate(&plan, ConstLimits::default()).unwrap();
        let ty = ConstantWireType::Scalar(PrimitiveType::Usize);
        let bytes = encode(result.value(), &ty, 1024).unwrap();
        assert_eq!(
            decode(&bytes, &ty, 1024).unwrap().scalar(),
            result.value().scalar()
        );
        assert_eq!(bytes, encode(result.value(), &ty, bytes.len()).unwrap());
        assert!(encode(result.value(), &ty, bytes.len() - 1).is_err());
        assert!(decode(&bytes, &ty, bytes.len() - 1).is_err());
    }
}
#[test]
fn immutable_wire_rejects_every_truncation_trailing_bytes_and_foreign_type() {
    let ty = ConstantWireType::Scalar(PrimitiveType::I8);
    let bytes = encode(
        &value(ValueRepr::signed(TypeId(4), -128).unwrap()),
        &ty,
        1024,
    )
    .unwrap();
    assert_eq!(&bytes[bytes.len() - 8..], &(-128i64).to_le_bytes());
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end], &ty, 1024).is_err());
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(decode(&extra, &ty, 1024).is_err());
    assert!(decode(&bytes, &ConstantWireType::Scalar(PrimitiveType::U8), 1024).is_err());
    for offset in 0..15 {
        let mut malformed = bytes.clone();
        malformed[offset] ^= 0x80;
        assert!(decode(&malformed, &ty, 1024).is_err());
    }
}
#[test]
fn immutable_wire_refuses_guest_handles_and_invalid_scalar_payloads() {
    let ty = ConstantWireType::Scalar(PrimitiveType::Bool);
    let bytes = encode(&value(ValueRepr::boolean(true)), &ty, 1024).unwrap();
    let body = bytes.len() - 16;
    for kind in [0u32, 8, 9, 10, 11, 12, 13, 14, 15, 16, u32::MAX] {
        let mut bad = bytes.clone();
        bad[body..body + 4].copy_from_slice(&kind.to_le_bytes());
        assert!(decode(&bad, &ty, 1024).is_err());
    }
    let mut bad = bytes.clone();
    bad[body + 8..].copy_from_slice(&2u64.to_le_bytes());
    assert!(decode(&bad, &ty, 1024).is_err());
    let mut bad = bytes;
    bad[body + 4..body + 8].copy_from_slice(&1u32.to_le_bytes());
    assert!(decode(&bad, &ty, 1024).is_err());
}
#[test]
fn immutable_wire_preserves_float_bits_and_rejects_wrong_descriptor() {
    let ty = ConstantWireType::Scalar(PrimitiveType::F64);
    for bits in [0, 1u64 << 63, 0x7ff0_0000_0000_0000, 0x7ff8_0000_0000_0042] {
        let v = value(ValueRepr::f64(f64::from_bits(bits)));
        let bytes = encode(&v, &ty, 1024).unwrap();
        assert_eq!(
            decode(&bytes, &ty, 1024).unwrap().scalar().unwrap().payload,
            bits
        );
    }
    assert!(encode(&value(ValueRepr::boolean(true)), &ty, 1024).is_err());
}
#[test]
fn immutable_wire_array_shape_is_explicit_even_when_empty() {
    let text = "const EMPTY: [u8; 0] = []; const VALUES: [i8; 2] = [-128, 127];";
    let source = ConstantSource::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
        Digest([174; 32]),
        CfgSet::guest_default(),
    )
    .unwrap();
    for (name, element, length) in [
        ("EMPTY", PrimitiveType::U8, 0),
        ("VALUES", PrimitiveType::I8, 2),
    ] {
        let at = source
            .declarations()
            .iter()
            .find(|d| d.name() == name)
            .unwrap()
            .range();
        let result = evaluate(&source.constant(at).unwrap(), ConstLimits::default()).unwrap();
        let ty = ConstantWireType::Array {
            element: Box::new(ConstantWireType::Scalar(element)),
            length,
        };
        let bytes = encode(result.value(), &ty, 1024).unwrap();
        let decoded = decode(&bytes, &ty, 1024).unwrap();
        assert_eq!(decoded.elements().unwrap().len(), length as usize);
        assert_eq!(encode(&decoded, &ty, 1024).unwrap(), bytes);
        let wrong = ConstantWireType::Array {
            element: Box::new(ConstantWireType::Scalar(PrimitiveType::Bool)),
            length,
        };
        assert!(decode(&bytes, &wrong, 1024).is_err());
    }
}
#[test]
fn immutable_wire_checks_expanded_shape_limits_before_payload_allocation() {
    let scalar = ConstantWireType::Scalar(PrimitiveType::U8);
    let huge = ConstantWireType::Array {
        element: Box::new(scalar.clone()),
        length: u64::MAX,
    };
    assert!(decode(&[], &huge, 1 << 20).is_err());
    let nested = ConstantWireType::Array {
        element: Box::new(ConstantWireType::Array {
            element: Box::new(scalar.clone()),
            length: 1000,
        }),
        length: 1000,
    };
    assert!(decode(&[], &nested, 1 << 20).is_err());
    let mut deep = scalar;
    for _ in 0..129 {
        deep = ConstantWireType::Array {
            element: Box::new(deep),
            length: 0,
        };
    }
    assert!(decode(&[], &deep, 1 << 20).is_err());
    assert!(
        decode(
            &[],
            &ConstantWireType::Scalar(PrimitiveType::U8),
            (1 << 20) + 1
        )
        .is_err()
    );
}
