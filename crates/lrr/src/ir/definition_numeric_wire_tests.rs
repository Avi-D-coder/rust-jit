//! Wire/structural controls only: these recipes grant no source or runtime type authority.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{
        op::{AssignKind, DefinitionNumericContext, LiteralKind, LiteralRecipe, PrimitiveType},
        pc::{NodeId, Phase},
        validate::verify,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{BodyIndex, NodeRole},
        parse::ParsedFile,
    },
};

fn body() -> BodyIndex {
    let blob = SourceBlob::new(b"fn live(){let mut r=0;r+=1;}".to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([41; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    parsed
        .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
        .unwrap()
}
fn draft(body: &BodyIndex, operations: Vec<Op>) -> RecipeDraft {
    RecipeDraft {
        key: body.pc(NodeId(0), Phase::Entry).unwrap(),
        origin: Origin {
            span: body.span(NodeId(0)).unwrap(),
            expansion_ancestry: vec![],
        },
        operations,
        exits: vec![RecipeExit::Next(body.pc(NodeId(0), Phase::Exit).unwrap())],
        dependencies: vec![],
    }
}
fn hint_digest(value: HintRecipe) -> Digest {
    let mut encoder = StableEncoder::new("lrr/v7/definition-numeric-hint-test");
    hint(&mut encoder, &value);
    encoder.finish()
}
#[test]
fn definition_numeric_hint_uses_explicit_context_tags() {
    for (context, words) in [
        (DefinitionNumericContext::Parameter(0), vec![6, 0, 0]),
        (
            DefinitionNumericContext::Parameter(99_999),
            vec![6, 0, 99_999],
        ),
        (DefinitionNumericContext::Result, vec![6, 1]),
    ] {
        let mut expected = StableEncoder::new("lrr/v7/definition-numeric-hint-test");
        for word in words {
            expected.u32(word);
        }
        assert_eq!(
            hint_digest(HintRecipe::DefinitionNumeric(context)),
            expected.finish()
        );
    }
    let parameter = hint_digest(HintRecipe::DefinitionNumeric(
        DefinitionNumericContext::Parameter(0),
    ));
    assert_ne!(
        parameter,
        hint_digest(HintRecipe::DefinitionNumeric(
            DefinitionNumericContext::Parameter(1)
        ))
    );
    assert_ne!(
        parameter,
        hint_digest(HintRecipe::DefinitionNumeric(
            DefinitionNumericContext::Result
        ))
    );
    assert_ne!(
        parameter,
        hint_digest(HintRecipe::Primitive(PrimitiveType::U32))
    );
}
#[test]
fn definition_numeric_compound_kind_preserves_context_and_rhs_option() {
    for (context, context_words) in [
        (DefinitionNumericContext::Parameter(3), vec![0, 3]),
        (DefinitionNumericContext::Result, vec![1]),
    ] {
        for right in [None, Some(PrimitiveType::I32), Some(PrimitiveType::U64)] {
            let mut actual = StableEncoder::new("lrr/v7/definition-numeric-kind-test");
            assign_kind(
                &mut actual,
                &AssignKind::DefinitionNumeric { context, right },
            );
            let mut expected = StableEncoder::new("lrr/v7/definition-numeric-kind-test");
            expected.u32(1);
            for word in &context_words {
                expected.u32(*word);
            }
            match right {
                None => expected.u32(0),
                Some(ty) => {
                    expected.u32(1);
                    expected.u32(ty.tag());
                }
            }
            assert_eq!(actual.finish(), expected.finish());
        }
    }
    let mut actual = StableEncoder::new("lrr/v7/definition-numeric-kind-test");
    assign_kind(
        &mut actual,
        &AssignKind::Primitive {
            left: PrimitiveType::I32,
            right: PrimitiveType::U8,
        },
    );
    let mut expected = StableEncoder::new("lrr/v7/definition-numeric-kind-test");
    for word in [0, 5, 9] {
        expected.u32(word);
    }
    assert_eq!(actual.finish(), expected.finish());
}
#[test]
fn definition_numeric_annotations_accept_any_structural_ordinal() {
    let body = body();
    for context in [
        DefinitionNumericContext::Parameter(0),
        DefinitionNumericContext::Parameter(99_999),
        // E-R5-006: no structural ordinal ceiling.
        DefinitionNumericContext::Parameter(100_000),
        DefinitionNumericContext::Parameter(u32::MAX),
        DefinitionNumericContext::Result,
    ] {
        let recipe = verify(
            draft(
                &body,
                vec![Op::SetHint {
                    child: SlotId(0),
                    hint: HintRecipe::DefinitionNumeric(context),
                }],
            ),
            &body,
        )
        .unwrap();
        assert_eq!(recipe.recipe().operations.len(), 1);
    }
}
#[test]
fn definition_numeric_compound_keeps_indexed_rhs_and_entry_checks() {
    let body = body();
    let binding = body
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Pattern)
        .unwrap()
        .id;
    let node = body
        .nodes()
        .iter()
        .find(|node| node.runtime_temporaries[0].is_some() && node.operands.len() == 2)
        .unwrap();
    let child = body.nodes()[node.operands[1].0 as usize].value.unwrap();
    let make = |context, right| RecipeDraft {
        key: body.pc(node.id, Phase::Entry).unwrap(),
        origin: Origin {
            span: body.span(node.id).unwrap(),
            expansion_ancestry: vec![],
        },
        operations: vec![Op::CompoundHint {
            binding,
            child,
            kind: AssignKind::DefinitionNumeric { context, right },
        }],
        exits: vec![RecipeExit::Next(
            body.pc(node.operands[0], Phase::Entry).unwrap(),
        )],
        dependencies: vec![],
    };
    for context in [
        DefinitionNumericContext::Result,
        DefinitionNumericContext::ResultOrDefault { floating: false },
        DefinitionNumericContext::ResultOrDefault { floating: true },
    ] {
        for right in [None, Some(PrimitiveType::I32)] {
            let valid = make(context, right);
            verify(valid.clone(), &body).unwrap();
            let mut wrong_phase = valid.clone();
            wrong_phase.key = body.pc(node.id, Phase::Operand(0)).unwrap();
            assert!(verify(wrong_phase, &body).is_err());
            let mut wrong_rhs = valid;
            let Op::CompoundHint { child, .. } = &mut wrong_rhs.operations[0] else {
                unreachable!()
            };
            *child = body.nodes()[node.operands[0].0 as usize].value.unwrap();
            assert!(verify(wrong_rhs, &body).is_err());
        }
        for right in [
            PrimitiveType::Unit,
            PrimitiveType::Bool,
            PrimitiveType::Char,
        ] {
            assert!(verify(make(context, Some(right)), &body).is_err());
        }
    }
    for right in [
        PrimitiveType::Unit,
        PrimitiveType::Bool,
        PrimitiveType::Char,
    ] {
        assert!(verify(make(DefinitionNumericContext::Result, Some(right)), &body).is_err());
    }
}
#[test]
fn recipe_schema_separates_definition_context_from_prior_cache_world() {
    assert_eq!(crate::ir::op::LRIR_RECIPE_SCHEMA, 13);
    assert_eq!(Recipe::DOMAIN, "lrr/v13/lrir-recipe");
    let body = body();
    let recipe = verify(
        draft(
            &body,
            vec![Op::SetHint {
                child: SlotId(0),
                hint: HintRecipe::DefinitionNumeric(DefinitionNumericContext::Result),
            }],
        ),
        &body,
    )
    .unwrap();
    let recipe = recipe.recipe();
    let mut expected = StableEncoder::new("lrr/v13/lrir-recipe");
    expected.record(&recipe.key);
    expected.record(&recipe.origin.span.body_key);
    expected.u32(recipe.origin.span.relative_start);
    expected.u32(recipe.origin.span.relative_end);
    expected.u64(0); // expansion ancestry
    expected.u64(1); // one operation
    for word in [2, 0, 6, 1] {
        expected.u32(word);
    } // SetHint slot0, DefinitionNumeric Result
    let RecipeExit::Next(next) = &recipe.exit else {
        unreachable!()
    };
    expected.u32(0);
    expected.record(next);
    expected.u64(0); // dependencies
    assert_eq!(recipe.fingerprint(), expected.finish());
    for domain in [
        "lrr/v12/lrir-recipe",
        "lrr/v6/lrir-recipe",
        "lrr/v7/lrir-recipe",
        "lrr/v8/lrir-recipe",
        "lrr/v9/lrir-recipe",
    ] {
        let mut prior = StableEncoder::new(domain);
        recipe.encode_fields(&mut prior);
        assert_ne!(recipe.fingerprint(), prior.finish());
    }
}

#[test]
fn conditional_result_context_encodes_family_as_one_stable_bool_byte() {
    let mut digests = Vec::new();
    for floating in [false, true] {
        let context = DefinitionNumericContext::ResultOrDefault { floating };
        let mut actual = StableEncoder::new("lrr/v8/conditional-numeric-test");
        definition_numeric_context(&mut actual, context);
        let mut expected = StableEncoder::new("lrr/v8/conditional-numeric-test");
        expected.u32(2);
        expected.u8(u8::from(floating));
        let digest = actual.finish();
        assert_eq!(digest, expected.finish());
        digests.push(digest);
        let mut actual = StableEncoder::new("lrr/v8/conditional-numeric-test");
        hint(&mut actual, &HintRecipe::DefinitionNumeric(context));
        let mut expected = StableEncoder::new("lrr/v8/conditional-numeric-test");
        expected.u32(6);
        expected.u32(2);
        expected.u8(u8::from(floating));
        assert_eq!(actual.finish(), expected.finish());
        for right in [None, Some(PrimitiveType::I32)] {
            let mut actual = StableEncoder::new("lrr/v8/conditional-numeric-test");
            assign_kind(
                &mut actual,
                &AssignKind::DefinitionNumeric { context, right },
            );
            let mut expected = StableEncoder::new("lrr/v8/conditional-numeric-test");
            expected.u32(1);
            expected.u32(2);
            expected.u8(u8::from(floating));
            match right {
                None => expected.u32(0),
                Some(ty) => {
                    expected.u32(1);
                    expected.u32(ty.tag());
                }
            }
            assert_eq!(actual.finish(), expected.finish());
        }
        assert_ne!(
            hint_digest(HintRecipe::DefinitionNumeric(context)),
            hint_digest(HintRecipe::DefinitionNumeric(
                DefinitionNumericContext::Result
            ))
        );
        assert_ne!(
            hint_digest(HintRecipe::DefinitionNumeric(context)),
            hint_digest(HintRecipe::DefinitionNumeric(
                DefinitionNumericContext::Parameter(0)
            ))
        );
    }
    assert_ne!(digests[0], digests[1]);
}

#[test]
fn conditional_result_families_are_structural_hints_at_every_annotation_site() {
    use crate::ir::op::DecodedLiteral;
    let body = body();
    let binding = body
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Pattern)
        .unwrap()
        .id;
    for floating in [false, true] {
        let hint =
            HintRecipe::DefinitionNumeric(DefinitionNumericContext::ResultOrDefault { floating });
        let literal = if floating {
            LiteralRecipe {
                kind: LiteralKind::Float,
                spelling: "0.0".into(),
                decoded: Some(DecodedLiteral::Float {
                    decimal: "0.0".into(),
                    suffix: None,
                }),
                source: body.span(NodeId(0)).unwrap(),
            }
        } else {
            LiteralRecipe {
                kind: LiteralKind::Integer,
                spelling: "0".into(),
                decoded: Some(DecodedLiteral::Integer {
                    negative: false,
                    radix: 10,
                    magnitude: 0,
                    suffix: None,
                }),
                source: body.span(NodeId(0)).unwrap(),
            }
        };
        for operation in [
            Op::SetHint {
                child: SlotId(0),
                hint: hint.clone(),
            },
            Op::DeclareLocal {
                binding,
                hint: hint.clone(),
                writable: true,
            },
            Op::BindLocal {
                binding,
                source: SlotId(0),
                hint: hint.clone(),
                writable: true,
            },
            Op::Literal {
                literal,
                hint,
                destination: SlotId(0),
            },
        ] {
            let verified = verify(draft(&body, vec![operation]), &body).unwrap();
            assert_eq!(verified.recipe().operations.len(), 1);
        }
    }
}

#[test]
fn definition_parameter_hint_retains_exact_ordinal_and_distinct_tag() {
    for ordinal in [0, 99_999] {
        let mut actual = StableEncoder::new("lrr/v9/definition-parameter-test");
        hint(&mut actual, &HintRecipe::DefinitionParameter(ordinal));
        let mut expected = StableEncoder::new("lrr/v9/definition-parameter-test");
        expected.u32(7);
        expected.u32(ordinal);
        assert_eq!(actual.finish(), expected.finish());
    }
    let hint0 = hint_digest(HintRecipe::DefinitionParameter(0));
    assert_ne!(hint0, hint_digest(HintRecipe::DefinitionParameter(1)));
    assert_ne!(
        hint0,
        hint_digest(HintRecipe::DefinitionNumeric(
            DefinitionNumericContext::Parameter(0)
        ))
    );
    assert_ne!(
        hint0,
        hint_digest(HintRecipe::Parameter(crate::traits::term::ParamKey {
            definition: Digest([0; 32]),
            ordinal: 0
        }))
    );
    assert_ne!(
        hint0,
        hint_digest(HintRecipe::Primitive(PrimitiveType::Bool))
    );
}

#[test]
fn definition_parameter_ordinal_is_accepted_at_every_annotation_site() {
    let body = body();
    let binding = body
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Pattern)
        .unwrap()
        .id;
    for ordinal in [0, 99_999, 100_000, u32::MAX] {
        let hint = HintRecipe::DefinitionParameter(ordinal);
        for operation in [
            Op::SetHint {
                child: SlotId(0),
                hint: hint.clone(),
            },
            Op::DeclareLocal {
                binding,
                hint: hint.clone(),
                writable: true,
            },
            Op::BindLocal {
                binding,
                source: SlotId(0),
                hint: hint.clone(),
                writable: true,
            },
            Op::Literal {
                literal: LiteralRecipe {
                    kind: LiteralKind::Bool,
                    spelling: "true".into(),
                    decoded: None,
                    source: body.span(NodeId(0)).unwrap(),
                },
                hint,
                destination: SlotId(0),
            },
        ] {
            // E-R5-006: no structural ordinal ceiling at any annotation site.
            let result = verify(draft(&body, vec![operation]), &body);
            assert_eq!(result.unwrap().recipe().operations.len(), 1);
        }
    }
}
