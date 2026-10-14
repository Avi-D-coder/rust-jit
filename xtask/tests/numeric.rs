use lrr::{
    runtime::{shape::TypeId, value::ValueRepr},
    vm::numeric::{Binary, NumericFailure, PanicKind, Unary, binary, unary},
};
fn signed(id: u32, value: i64) -> ValueRepr {
    ValueRepr::signed(TypeId(id), value).unwrap()
}
fn unsigned(id: u32, value: u64) -> ValueRepr {
    ValueRepr::unsigned(TypeId(id), value).unwrap()
}
fn check_signed(op: Binary, a: i64, b: i64, id: u32, width: u32) {
    let result = binary(op, signed(id, a), signed(id, b));
    let min = -(1i128 << (width - 1));
    let max = (1i128 << (width - 1)) - 1;
    if matches!(op, Binary::Div | Binary::Rem) && b == 0 {
        assert!(matches!(
            result,
            Err(NumericFailure::GuestPanic(PanicKind::DivisionByZero))
        ));
        return;
    }
    if matches!(op, Binary::Div | Binary::Rem) && a as i128 == min && b == -1 {
        assert!(matches!(
            result,
            Err(NumericFailure::GuestPanic(PanicKind::Overflow))
        ));
        return;
    }
    let a = a as i128;
    let b = b as i128;
    let expected = match op {
        Binary::Add => a + b,
        Binary::Sub => a - b,
        Binary::Mul => a * b,
        Binary::Div => a / b,
        Binary::Rem => a % b,
        _ => panic!("oracle operation outside arithmetic set"),
    };
    if expected < min || expected > max {
        assert!(matches!(
            result,
            Err(NumericFailure::GuestPanic(PanicKind::Overflow))
        ))
    } else {
        let actual = result.unwrap();
        assert_eq!(actual.type_id, id);
        assert_eq!(actual.payload as i64, expected as i64)
    }
}
fn check_unsigned(op: Binary, a: u64, b: u64, id: u32, width: u32) {
    let result = binary(op, unsigned(id, a), unsigned(id, b));
    let max = (1u128 << width) - 1;
    if matches!(op, Binary::Div | Binary::Rem) && b == 0 {
        assert!(matches!(
            result,
            Err(NumericFailure::GuestPanic(PanicKind::DivisionByZero))
        ));
        return;
    }
    if op == Binary::Sub && a < b {
        assert!(matches!(
            result,
            Err(NumericFailure::GuestPanic(PanicKind::Overflow))
        ));
        return;
    }
    let a = a as u128;
    let b = b as u128;
    let expected = match op {
        Binary::Add => a + b,
        Binary::Sub => a - b,
        Binary::Mul => a * b,
        Binary::Div => a / b,
        Binary::Rem => a % b,
        _ => panic!("oracle operation outside arithmetic set"),
    };
    if expected > max {
        assert!(matches!(
            result,
            Err(NumericFailure::GuestPanic(PanicKind::Overflow))
        ))
    } else {
        let actual = result.unwrap();
        assert_eq!(actual.type_id, id);
        assert_eq!(actual.payload, expected as u64)
    }
}
#[test]
fn exhaustive_i8_and_u8_arithmetic_matches_widened_mathematical_oracle() {
    for op in [
        Binary::Add,
        Binary::Sub,
        Binary::Mul,
        Binary::Div,
        Binary::Rem,
    ] {
        for a in i8::MIN..=i8::MAX {
            for b in i8::MIN..=i8::MAX {
                check_signed(op, a as i64, b as i64, 4, 8)
            }
        }
        for a in u8::MIN..=u8::MAX {
            for b in u8::MIN..=u8::MAX {
                check_unsigned(op, a as u64, b as u64, 9, 8)
            }
        }
    }
}
#[test]
fn seeded_wider_arithmetic_preserves_each_descriptor_and_checked_range() {
    let mut state = 0x6c72725f32303234u64;
    for _ in 0..1000 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let a = state;
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let b = state;
        for op in [
            Binary::Add,
            Binary::Sub,
            Binary::Mul,
            Binary::Div,
            Binary::Rem,
        ] {
            for (id, width, x, y) in [
                (5, 16, a as i16 as i64, b as i16 as i64),
                (6, 32, a as i32 as i64, b as i32 as i64),
                (7, 64, a as i64, b as i64),
                (8, 64, a as i64, b as i64),
            ] {
                check_signed(op, x, y, id, width)
            }
            for (id, width, x, y) in [
                (10, 16, a as u16 as u64, b as u16 as u64),
                (11, 32, a as u32 as u64, b as u32 as u64),
                (12, 64, a, b),
                (13, 64, a, b),
            ] {
                check_unsigned(op, x, y, id, width)
            }
        }
    }
}
#[test]
fn shifts_check_actual_left_width_and_allow_distinct_integer_count_descriptors() {
    for a in i8::MIN..=i8::MAX {
        for count in -1..=9 {
            for op in [Binary::Shl, Binary::Shr] {
                let result = binary(op, signed(4, a as i64), signed(6, count));
                if !(0..8).contains(&count) {
                    assert!(matches!(
                        result,
                        Err(NumericFailure::GuestPanic(PanicKind::InvalidShift))
                    ))
                } else {
                    let expected = if op == Binary::Shl {
                        (((a as u8 as u16) << count) & 0xff) as u8 as i8
                    } else {
                        ((a as i16) >> count) as i8
                    };
                    assert_eq!(result.unwrap().payload as i64, expected as i64)
                }
            }
        }
    }
    for (id, width) in [(4, 8), (5, 16), (6, 32), (7, 64), (8, 64)] {
        assert!(binary(Binary::Shl, signed(id, 1), unsigned(11, width - 1)).is_ok());
        assert!(matches!(
            binary(Binary::Shl, signed(id, 1), unsigned(12, width)),
            Err(NumericFailure::GuestPanic(PanicKind::InvalidShift))
        ))
    }
    assert!(
        matches!(binary(Binary::Shr,unsigned(9,1),ValueRepr::boolean(true)),Err(NumericFailure::RuntimeFault(error)) if error.code=="LRR-E-OP-TYPE")
    );
}
#[test]
fn unary_minima_and_bitwise_boolean_integer_operations_have_typed_outcomes() {
    for (id, width) in [(4, 8), (5, 16), (6, 32), (7, 64), (8, 64)] {
        let min = if width == 64 {
            i64::MIN
        } else {
            -(1i64 << (width - 1))
        };
        assert!(matches!(
            unary(Unary::Neg, signed(id, min)),
            Err(NumericFailure::GuestPanic(PanicKind::Overflow))
        ));
        assert_eq!(unary(Unary::Not, signed(id, 0)).unwrap(), signed(id, -1))
    }
    for (id, width) in [(9, 8), (10, 16), (11, 32), (12, 64), (13, 64)] {
        let max = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        assert_eq!(
            unary(Unary::Not, unsigned(id, 0)).unwrap(),
            unsigned(id, max)
        )
    }
    for a in [false, true] {
        for b in [false, true] {
            for (op, expected) in [
                (Binary::BitAnd, a & b),
                (Binary::BitOr, a | b),
                (Binary::BitXor, a ^ b),
                (Binary::Eq, a == b),
                (Binary::Lt, (!a) & b),
            ] {
                assert_eq!(
                    binary(op, ValueRepr::boolean(a), ValueRepr::boolean(b)).unwrap(),
                    ValueRepr::boolean(expected)
                )
            }
        }
    }
    assert_eq!(
        unary(Unary::Not, ValueRepr::boolean(true)).unwrap(),
        ValueRepr::boolean(false)
    );
}
#[test]
fn float_operations_round_at_each_f32_step_and_preserve_ieee_comparisons() {
    let a = ValueRepr::f32(16_777_216.0);
    let sum = binary(Binary::Add, a, ValueRepr::f32(1.0)).unwrap();
    assert_eq!(sum.payload, a.payload);
    assert_eq!(binary(Binary::Sub, sum, a).unwrap().payload, 0);
    let nan = ValueRepr::f32(f32::from_bits(0x7fc12345));
    assert_eq!(
        binary(Binary::Eq, nan, nan).unwrap(),
        ValueRepr::boolean(false)
    );
    assert_eq!(
        binary(Binary::Ne, nan, nan).unwrap(),
        ValueRepr::boolean(true)
    );
    assert_eq!(
        binary(Binary::Le, nan, ValueRepr::f32(0.0)).unwrap(),
        ValueRepr::boolean(false)
    );
    let inf = binary(Binary::Div, ValueRepr::f64(1.0), ValueRepr::f64(0.0)).unwrap();
    assert!(f64::from_bits(inf.payload).is_infinite());
    assert_eq!(
        unary(Unary::Neg, ValueRepr::f32(0.0)).unwrap().payload,
        0x80000000
    );
    assert_eq!(
        binary(
            Binary::Lt,
            ValueRepr::character('a'),
            ValueRepr::character('β')
        )
        .unwrap(),
        ValueRepr::boolean(true)
    );
}
#[test]
fn unsupported_pairs_distinct_word_types_and_unresolved_widths_are_runtime_faults() {
    for (left, right) in [
        (unsigned(12, 1), unsigned(13, 1)),
        (signed(7, 1), signed(8, 1)),
        (ValueRepr::f32(1.0), ValueRepr::f64(1.0)),
        (ValueRepr::boolean(true), ValueRepr::boolean(false)),
    ] {
        assert!(
            matches!(binary(Binary::Add,left,right),Err(NumericFailure::RuntimeFault(error)) if error.code=="LRR-E-OP-TYPE")
        )
    }
    let literal = ValueRepr {
        kind: 11,
        type_id: 0,
        payload: 1,
    };
    assert!(
        matches!(binary(Binary::Add,literal,literal),Err(NumericFailure::RuntimeFault(error)) if error.code=="LRR-E-TYPE-CONTEXT")
    );
    assert!(
        matches!(binary(Binary::Add,ValueRepr{kind:8,type_id:17,payload:1},ValueRepr::unit()),Err(NumericFailure::RuntimeFault(error)) if error.code=="LRR-E-OP-TYPE")
    );
}

#[test]
fn concrete_literal_ticket_is_reserved_before_a_shared_width_binding_commits() {
    let mut store = lrr::runtime::cell::GuestStore::new(1, 2, 1 << 20).unwrap();
    let literal = store.integer_literal(false, 42, "42".into()).unwrap();
    assert_eq!(
        store
            .materialize_ticket(literal, TypeId(7))
            .unwrap_err()
            .code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert_eq!(store.peek_ticket(literal).unwrap().kind, 11);
    // A different reached descriptor can still bind: the failed result reservation
    // did not silently commit the earlier i64 hint to this shared record.
    assert_eq!(
        store.materialize(literal, TypeId(6)).unwrap(),
        signed(6, 42)
    );
    let mut store = lrr::runtime::cell::GuestStore::new(1, 10, 1 << 20).unwrap();
    let literal = store.integer_literal(false, 42, "42".into()).unwrap();
    let concrete = store.materialize_ticket(literal, TypeId(7)).unwrap();
    assert_eq!(store.peek_ticket(concrete).unwrap(), signed(7, 42));
    assert!(store.materialize(literal, TypeId(6)).is_err());
}

#[test]
fn integer_casts_truncate_extend_and_retain_distinct_destination_descriptors() {
    use lrr::vm::numeric::cast;
    let types = [
        (4, 8, true),
        (5, 16, true),
        (6, 32, true),
        (7, 64, true),
        (8, 64, true),
        (9, 8, false),
        (10, 16, false),
        (11, 32, false),
        (12, 64, false),
        (13, 64, false),
    ];
    let samples = [
        0,
        1,
        127,
        128,
        255,
        256,
        0x8000,
        0xffff,
        0x80000000,
        0xffffffff,
        0x8000000000000000,
        u64::MAX,
    ];
    for &(source_id, width, is_signed) in &types {
        for bits in samples {
            let mask = if width == 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            let raw = bits & mask;
            let source = if is_signed {
                let number = if width == 64 {
                    raw as i64
                } else {
                    ((raw << (64 - width)) as i64) >> (64 - width)
                };
                signed(source_id, number)
            } else {
                unsigned(source_id, raw)
            };
            for &(target_id, target_width, target_signed) in &types {
                let target_mask = if target_width == 64 {
                    u64::MAX
                } else {
                    (1u64 << target_width) - 1
                };
                let result_bits = source.payload & target_mask;
                let expected = if target_signed {
                    let number = if target_width == 64 {
                        result_bits as i64
                    } else {
                        ((result_bits << (64 - target_width)) as i64) >> (64 - target_width)
                    };
                    signed(target_id, number)
                } else {
                    unsigned(target_id, result_bits)
                };
                assert_eq!(cast(source, TypeId(target_id)).unwrap(), expected);
            }
        }
    }
}
#[test]
fn float_integer_casts_saturate_at_the_destination_width_and_truncate_toward_zero() {
    use lrr::vm::numeric::cast;
    for value in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -1000.0,
        -128.9,
        -1.9,
        -0.0,
        0.0,
        1.9,
        127.9,
        128.0,
        255.9,
        256.0,
        1000.0,
    ] {
        let actual = cast(ValueRepr::f64(value), TypeId(4)).unwrap();
        let expected = if value.is_nan() {
            0
        } else {
            value.trunc().clamp(-128.0, 127.0) as i64
        };
        assert_eq!(actual, signed(4, expected));
        let expected = if value.is_nan() {
            0
        } else {
            value.trunc().clamp(0.0, 255.0) as u64
        };
        assert_eq!(
            cast(ValueRepr::f64(value), TypeId(9)).unwrap(),
            unsigned(9, expected)
        );
    }
    for (id, min, max) in [
        (4, i8::MIN as i64, i8::MAX as i64),
        (5, i16::MIN as i64, i16::MAX as i64),
        (6, i32::MIN as i64, i32::MAX as i64),
        (7, i64::MIN, i64::MAX),
        (8, i64::MIN, i64::MAX),
    ] {
        assert_eq!(
            cast(ValueRepr::f32(f32::INFINITY), TypeId(id)).unwrap(),
            signed(id, max)
        );
        assert_eq!(
            cast(ValueRepr::f64(f64::NEG_INFINITY), TypeId(id)).unwrap(),
            signed(id, min)
        );
        assert_eq!(
            cast(ValueRepr::f64(f64::NAN), TypeId(id)).unwrap(),
            signed(id, 0)
        )
    }
    for (id, max) in [
        (9, u8::MAX as u64),
        (10, u16::MAX as u64),
        (11, u32::MAX as u64),
        (12, u64::MAX),
        (13, u64::MAX),
    ] {
        assert_eq!(
            cast(ValueRepr::f64(f64::INFINITY), TypeId(id)).unwrap(),
            unsigned(id, max)
        );
        assert_eq!(
            cast(ValueRepr::f32(f32::NEG_INFINITY), TypeId(id)).unwrap(),
            unsigned(id, 0)
        )
    }
}
#[test]
fn float_casts_preserve_rounding_sign_and_nan_rules_without_intermediate_precision() {
    use lrr::vm::numeric::cast;
    assert_eq!(
        cast(unsigned(12, 16_777_217), TypeId(14)).unwrap(),
        ValueRepr::f32(16_777_216.0)
    );
    assert_eq!(
        cast(unsigned(12, 16_777_219), TypeId(14)).unwrap(),
        ValueRepr::f32(16_777_220.0)
    );
    for value in [-0.0, 1.5, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
        assert_eq!(
            cast(ValueRepr::f64(value), TypeId(14)).unwrap().payload,
            u64::from((value as f32).to_bits())
        )
    }
    for bits in [0, 0x80000000, 1, 0x3fc00000, 0x7f800000] {
        let value = f32::from_bits(bits);
        assert_eq!(
            cast(ValueRepr::f32(value), TypeId(15)).unwrap(),
            ValueRepr::f64(value as f64)
        )
    }
    let nan = cast(ValueRepr::f64(f64::NAN), TypeId(14)).unwrap();
    assert!(f32::from_bits(nan.payload as u32).is_nan());
    // This integer is just above an f32 tie, but f64 cannot retain the +1.
    let witness = (1u64 << 63) + (1u64 << 39) + 1;
    assert_ne!(
        (witness as f32).to_bits(),
        ((witness as f64) as f32).to_bits()
    );
    assert_eq!(
        cast(unsigned(12, witness), TypeId(14)).unwrap().payload,
        u64::from((witness as f32).to_bits())
    );
}
#[test]
fn char_boolean_identity_and_invalid_casts_use_the_explicit_primitive_matrix() {
    use lrr::vm::numeric::cast;
    for number in 0..=255 {
        let value = cast(unsigned(9, number), TypeId(3)).unwrap();
        assert_eq!(
            value,
            ValueRepr::character(char::from_u32(number as u32).unwrap())
        );
        assert_eq!(cast(value, TypeId(9)).unwrap(), unsigned(9, number))
    }
    assert_eq!(
        cast(ValueRepr::character('β'), TypeId(11)).unwrap(),
        unsigned(11, 946)
    );
    assert_eq!(
        cast(ValueRepr::boolean(true), TypeId(7)).unwrap(),
        signed(7, 1)
    );
    for value in [
        ValueRepr::unit(),
        ValueRepr::boolean(false),
        ValueRepr::character('β'),
        ValueRepr::f32(f32::from_bits(0x7fc12345)),
    ] {
        assert_eq!(cast(value, TypeId(value.type_id)).unwrap(), value)
    }
    for (value, target) in [
        (unsigned(10, 65), 3),
        (signed(4, 65), 3),
        (ValueRepr::boolean(true), 14),
        (ValueRepr::character('A'), 15),
        (signed(7, 1), 2),
        (ValueRepr::unit(), 9),
        (signed(7, 1), 17),
        (
            ValueRepr {
                kind: 9,
                type_id: 17,
                payload: 1,
            },
            13,
        ),
    ] {
        assert!(
            matches!(cast(value,TypeId(target)),Err(NumericFailure::RuntimeFault(error)) if error.code=="LRR-E-OP-TYPE")
        )
    }
    assert!(
        matches!(cast(ValueRepr{kind:11,type_id:0,payload:1},TypeId(7)),Err(NumericFailure::RuntimeFault(error)) if error.code=="LRR-E-TYPE-CONTEXT")
    );
}

#[test]
fn seeded_wider_casts_match_mathematical_modulo_and_signed_interpretation() {
    use lrr::vm::numeric::cast;
    let types = [
        (4, 8, true),
        (5, 16, true),
        (6, 32, true),
        (7, 64, true),
        (8, 64, true),
        (9, 8, false),
        (10, 16, false),
        (11, 32, false),
        (12, 64, false),
        (13, 64, false),
    ];
    let mut state = 0x746f32355f636173u64;
    for _ in 0..1000 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        for &(source_id, source_width, source_signed) in &types {
            let modulus = 1i128 << source_width;
            let raw = (state as i128).rem_euclid(modulus);
            let number = if source_signed && raw >= modulus / 2 {
                raw - modulus
            } else {
                raw
            };
            let source = if source_signed {
                signed(source_id, number as i64)
            } else {
                unsigned(source_id, number as u64)
            };
            for &(target_id, width, target_signed) in &types {
                let modulus = 1i128 << width;
                let raw = number.rem_euclid(modulus);
                let number = if target_signed && raw >= modulus / 2 {
                    raw - modulus
                } else {
                    raw
                };
                let expected = if target_signed {
                    signed(target_id, number as i64)
                } else {
                    unsigned(target_id, number as u64)
                };
                assert_eq!(cast(source, TypeId(target_id)).unwrap(), expected);
            }
        }
    }
}
