//! Ordinary literal/VM controls derived from unchanged originals. No fixture or
//! four-context completion claim follows from these partial literal entries.
use super::*;
use crate::ir::op::DecodedLiteral;
const MATCH_STR: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/match/match-borrowed-str.rs"
);
const NUL_STR: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/str/nul-char-equivalence.rs"
);
fn static_source() -> TyTerm {
    TyTerm::Ref(
        crate::traits::term::RegionTerm::Static,
        false,
        Box::new(TyTerm::Str),
    )
}
#[test]
fn official_string_literals_enter_normal_vm_with_static_source_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for original in [MATCH_STR, NUL_STR] {
            let mut c = literal_context_in(
                original,
                edition,
                LiteralKind::String,
                100,
                1000,
                1_000_000,
                if original == MATCH_STR {
                    "compare"
                } else {
                    "main"
                },
            );
            let expected = c
                .recipe
                .recipe()
                .operations
                .iter()
                .find_map(|op| match op {
                    Op::Literal { literal, .. } => match &literal.decoded {
                        Some(DecodedLiteral::String(p)) => Some(p.utf8().to_owned()),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap();
            if original == MATCH_STR {
                assert_eq!(expected, "foo");
            } else {
                assert_eq!(expected.as_bytes(), &[0, 0, 0, 0]);
            }
            let (_, decodings) = crate::syntax::string_literal::observe_decoding(|| {
                let warm =
                    c.db.fetch(
                        &SyntaxProvider,
                        QueryKey::Thunk(c.recipe.recipe().key.clone()),
                    )
                    .into_ready()
                    .unwrap()
                    .unwrap();
                let QueryValue::Thunk(warm) = warm.value.as_ref() else {
                    panic!()
                };
                assert!(std::sync::Arc::ptr_eq(
                    warm.string_literal_roots().next().unwrap(),
                    c.recipe.string_literal_roots().next().unwrap()
                ));
                assert!(matches!(run(&mut c).unwrap(), StepOutcome::Next(_)));
            });
            assert_eq!(
                decodings, 0,
                "warm query and installed literal do no lexical decoding"
            );
            assert_eq!(
                c.frame.source_view(c.destination).unwrap(),
                Some(static_source())
            );
            let cell = c.frame.cell(c.destination).unwrap();
            let ticket = c.store.copy_from(cell).unwrap();
            assert_eq!(c.store.str_reference_contents(ticket).unwrap(), expected);
            assert_eq!(c.frame.counters.started_by_opcode[0], 1);
            assert_eq!(
                c.frame.counters.operations_started,
                c.frame.counters.operations_completed
            );
            let before = footprint(&c);
            let counters = serde_json::to_value(c.frame.counters()).unwrap();
            assert!(run(&mut c).is_err());
            assert_eq!(footprint(&c), before);
            assert_eq!(serde_json::to_value(c.frame.counters()).unwrap(), counters);
        }
    }
}
#[test]
fn string_literal_source_and_descriptor_refuse_before_owning_publication() {
    for edition in [2015, 2018, 2021, 2024] {
        let mut c = literal_context_in(
            MATCH_STR,
            edition,
            LiteralKind::String,
            100,
            1000,
            1_000_000,
            "compare",
        );
        c.frame.set_hint(c.destination, Some(TypeId(9))).unwrap();
        let before = footprint(&c);
        assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-TYPE-CONTEXT");
        assert_untouched(&c, before, None);
        let mut c = literal_context_in(
            MATCH_STR,
            edition,
            LiteralKind::String,
            100,
            1000,
            1_000_000,
            "compare",
        );
        c.frame
            .set_source_view(c.destination, Some(TyTerm::Primitive(PrimitiveType::Bool)))
            .unwrap();
        // Rigid destination source is an actual storage contract here.
        c.frame.slots[c.destination.0 as usize].strict_hint = true;
        let before = footprint(&c);
        assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-TYPE-CONTEXT");
        assert_untouched(&c, before, Some(TyTerm::Primitive(PrimitiveType::Bool)));
    }
}
#[test]
fn source_receipt_cannot_be_relabelled_as_another_literal_slot_or_payload() {
    let c = literal_context_in(
        MATCH_STR,
        2015,
        LiteralKind::String,
        100,
        1000,
        1_000_000,
        "compare",
    );
    let other = literal_context(NUL_STR, 2015, LiteralKind::String, 100, 1000, 1_000_000);
    let other_proof = other.recipe.string_literal_roots().next().unwrap().clone();
    for change in 0..6 {
        let recipe = c.recipe.recipe();
        let mut draft = RecipeDraft {
            key: recipe.key.clone(),
            origin: recipe.origin.clone(),
            operations: recipe.operations.to_vec(),
            exits: vec![recipe.exit.clone()],
            dependencies: recipe.dependencies.to_vec(),
        };
        let Op::Literal {
            literal,
            destination,
            ..
        } = draft
            .operations
            .iter_mut()
            .find(|op| matches!(op, Op::Literal { .. }))
            .unwrap()
        else {
            panic!()
        };
        match change {
            0 => literal.spelling = "\"forged\"".into(),
            1 => literal.source.relative_start += 1,
            2 => *destination = SlotId(destination.0 + 2),
            3 => literal.kind = LiteralKind::ByteString,
            4 => literal.decoded = None,
            5 => literal.decoded = Some(DecodedLiteral::String(other_proof.clone())),
            _ => unreachable!(),
        }
        assert!(verify(draft, &c.frame.body).is_err());
    }
}
#[test]
fn fuel_and_combined_string_backing_lease_ticket_cell_quota_are_atomic() {
    for edition in [2015, 2018, 2021, 2024] {
        let probe = literal_context_in(
            MATCH_STR,
            edition,
            LiteralKind::String,
            100,
            1000,
            1_000_000,
            "compare",
        );
        for fuel in [0, 1] {
            let mut c = literal_context_in(
                MATCH_STR,
                edition,
                LiteralKind::String,
                fuel,
                1000,
                1_000_000,
                "compare",
            );
            // Account the actual ordinary ScopeEnter prefix where present.
            assert_eq!(prefix(&c), 0);
            let before = footprint(&c);
            assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
            assert_eq!(c.frame.fuel_snapshot().used, fuel);
            if fuel == 0 {
                assert_untouched(&c, before, None);
            } else {
                assert_eq!(
                    c.frame.source_view(c.destination).unwrap(),
                    Some(static_source())
                );
            }
        }
        for free in 0..4 {
            let mut c = literal_context_in(
                MATCH_STR,
                edition,
                LiteralKind::String,
                100,
                probe.store.budget().handles() + free,
                1_000_000,
                "compare",
            );
            let before = footprint(&c);
            assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
            assert_untouched(&c, before, None);
        }
    }
}
