//! Source controls only. Whole unchanged main acceptance is the stock adapter.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{
        op::{AssignKind, HintRecipe, Op},
        pc::Phase,
    },
    source::snapshot::{CfgSet, SourceBlob},
};

fn entered(source: &str, edition: u16, name: &str) -> (ParsedFile, BodyHandle, BodyIndex) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([197; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|i| i.def_key.declared_name == name)
        .unwrap()
        .body
        .clone()
        .unwrap();
    let index = parsed.index_body(&body, Default::default()).unwrap();
    (parsed, body, index)
}
fn selected(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    ordinal: usize,
) -> Option<DefinitionNumericContext> {
    let statement = parsed
        .entered_body_root(body)
        .unwrap()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .nth(ordinal)
        .unwrap();
    let ast::Pat::IdentPat(pattern) = statement.pat().unwrap() else {
        panic!("test binding")
    };
    inferred_context(parsed, index, body, &statement, &pattern).unwrap()
}

#[test]
fn unchanged_mixed_loop_uses_owned_result_for_literal_and_compound_all_editions() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/for-loop-while/label_break_value.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index) = entered(ORIGINAL, edition, "label_break_mixed");
        // The written `u32` result is a strong same-family proof: the
        // module's builtin inert `#![allow]`/`#[allow]` attributes cannot
        // introduce a type name, so the exact Result relation is selected.
        assert_eq!(
            selected(&parsed, &body, &index, 0),
            Some(DefinitionNumericContext::Result)
        );
        let root = parsed.entered_body_root(&body).unwrap();
        let statement = root.descendants().find_map(ast::LetStmt::cast).unwrap();
        let node = id_for(&index, &body, statement.syntax()).unwrap();
        let recipe = parsed
            .initial_thunk(&body, &index, &index.pc(node, Phase::Entry).unwrap())
            .unwrap();
        assert!(recipe.recipe().operations.iter().any(|op| matches!(
            op,
            Op::SetHint {
                hint: HintRecipe::DefinitionNumeric(DefinitionNumericContext::Result),
                ..
            }
        )));
        let mut compounds = 0;
        for binary in root
            .descendants()
            .filter_map(ast::BinExpr::cast)
            .filter(|b| b.op_token().is_some_and(|t| t.text() == "+="))
        {
            let node = id_for(&index, &body, binary.syntax()).unwrap();
            let recipe = parsed
                .initial_thunk(&body, &index, &index.pc(node, Phase::Entry).unwrap())
                .unwrap();
            assert!(recipe.recipe().operations.iter().any(|op| matches!(
                op,
                Op::CompoundHint {
                    kind: AssignKind::DefinitionNumeric {
                        context: DefinitionNumericContext::Result,
                        right: None
                    },
                    ..
                }
            )));
            compounds += 1;
        }
        assert_eq!(compounds, 2);
    }
}

#[test]
fn exact_parameter_relation_ignores_cold_unknown_and_preserves_shadow_identity() {
    let source = "fn chosen(v:u32,b:bool){let mut r=0;if false {let _:Unknown=r;let _=b==r;}let _=v==r;{let r=0;let _=r==1;}r+=1;}fn cold(){let _:Unknown=missing;}";
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index) = entered(source, edition, "chosen");
        assert_eq!(
            selected(&parsed, &body, &index, 0),
            Some(DefinitionNumericContext::Parameter(0))
        );
        let statements: Vec<_> = parsed
            .entered_body_root(&body)
            .unwrap()
            .descendants()
            .filter_map(ast::LetStmt::cast)
            .collect();
        let shadow = statements
            .iter()
            .find(|s| s.syntax().text() == "let r=0;")
            .unwrap();
        let ast::Pat::IdentPat(pattern) = shadow.pat().unwrap() else {
            panic!("test binding")
        };
        assert_eq!(
            inferred_context(&parsed, &index, &body, shadow, &pattern).unwrap(),
            None
        );
    }
}

#[test]
fn incomplete_parameter_primitive_authority_and_opaque_children_mint_no_context() {
    for source in [
        "type u32=bool;fn chosen(v:u32){let r=0;let _=v==r;}",
        "fn chosen<u32>(v:u32){let r=0;let _=v==r;}",
        "fn chosen(v:u32){let r=0;let _=||{return r;};}",
        "fn chosen(v:u32){let r=0;let _=async {return r;};}",
        "fn chosen(v:u32){let r=0;fn cold()->u32{let r=1;r}let _=r==1;}",
    ] {
        let (parsed, body, index) = entered(source, 2024, "chosen");
        assert_eq!(selected(&parsed, &body, &index, 0), None, "{source}");
    }
}

#[test]
fn suffix_shift_and_reached_literal_validity_are_not_reparsed_into_source_authority() {
    let (parsed, body, index) = entered("fn chosen()->u32{let r=1u8;r}", 2024, "chosen");
    assert_eq!(selected(&parsed, &body, &index, 0), None);
    let root = parsed.entered_body_root(&body).unwrap();
    let literal = root.descendants().find_map(ast::Literal::cast).unwrap();
    assert_eq!(
        compound_right(&ast::Expr::Literal(literal), false).unwrap(),
        Some(Some(PrimitiveType::U8))
    );
    let (parsed, body, _) = entered("fn chosen(){let r=1;}", 2024, "chosen");
    let literal = parsed
        .entered_body_root(&body)
        .unwrap()
        .descendants()
        .find_map(ast::Literal::cast)
        .unwrap();
    assert_eq!(
        compound_right(&ast::Expr::Literal(literal.clone()), false).unwrap(),
        Some(None)
    );
    assert_eq!(
        compound_right(&ast::Expr::Literal(literal), true).unwrap(),
        Some(Some(PrimitiveType::I32))
    );
}

#[test]
fn incompatible_cold_numeric_family_cannot_replace_compatible_parameter_relation() {
    let (parsed, body, index) = entered(
        "fn chosen(i:i32,f:f64){let r=0.0;if false{let _=i==r;}let _=f==r;}",
        2024,
        "chosen",
    );
    assert_eq!(
        selected(&parsed, &body, &index, 0),
        Some(DefinitionNumericContext::Parameter(1))
    );
}

#[test]
fn cold_incompatible_return_retains_only_conditional_source_relation_all_editions() {
    // LRR source guards: several programs are compiler-negative. These checks
    // assert only reached source-plan selection, not a diagnostic or runtime pass.
    for (source, floating) in [
        ("fn chosen()->bool{let n=0;if false{return n;}true}", false),
        ("fn chosen()->f64{let n=0;if false{return n;}1.0}", false),
        ("fn chosen()->i32{let n=0.0;if false{return n;}1}", true),
        (
            "fn chosen()->Unknown{let n=0;if false{return n;}loop{}}",
            false,
        ),
        (
            "type u32=bool;fn chosen()->u32{let n=0;if false{return n;}true}",
            false,
        ),
        (
            "fn chosen<u32>()->u32{let n=0;if false{return n;}loop{}}",
            false,
        ),
    ] {
        for edition in [2015, 2018, 2021, 2024] {
            let (parsed, body, index) = entered(source, edition, "chosen");
            assert_eq!(
                selected(&parsed, &body, &index, 0),
                Some(DefinitionNumericContext::ResultOrDefault { floating }),
                "{edition}: {source}"
            );
            let root = parsed.entered_body_root(&body).unwrap();
            let statement = root.descendants().find_map(ast::LetStmt::cast).unwrap();
            let node = id_for(&index, &body, statement.syntax()).unwrap();
            let recipe = parsed
                .initial_thunk(&body, &index, &index.pc(node, Phase::Entry).unwrap())
                .unwrap();
            assert!(
                recipe.recipe().operations.iter().any(|op| matches!(
                    op,
                    Op::SetHint {
                        hint: HintRecipe::DefinitionNumeric(DefinitionNumericContext::ResultOrDefault { floating: family }),
                        ..
                    } if *family == floating
                )),
                "{edition}: {source}"
            );
            assert!(
                !recipe.recipe().operations.iter().any(|op| matches!(
                    op,
                    Op::SetHint {
                        hint: HintRecipe::DefinitionNumeric(DefinitionNumericContext::Result),
                        ..
                    }
                )),
                "{edition}: {source}"
            );
        }
    }
}

#[test]
fn result_tail_preserves_strong_or_conditional_source_relation() {
    for (source, floating) in [
        ("fn chosen()->bool{let n=0;panic!();n}", false),
        ("fn chosen()->f64{let n=0;panic!();n}", false),
        ("fn chosen()->i32{let n=0.0;panic!();n}", true),
        ("fn chosen()->Unknown{let n=0;panic!();n}", false),
        ("type u32=bool;fn chosen()->u32{let n=0;panic!();n}", false),
        ("fn chosen<u32>()->u32{let n=0;panic!();n}", false),
    ] {
        for edition in [2015, 2018, 2021, 2024] {
            let (parsed, body, index) = entered(source, edition, "chosen");
            assert_eq!(
                selected(&parsed, &body, &index, 0),
                Some(DefinitionNumericContext::ResultOrDefault { floating }),
                "{edition}: {source}"
            );
        }
    }
    for source in [
        "fn chosen()->u32{let n=0;n}",
        "fn chosen()->f64{let n=0.0;n}",
        "fn chosen()->u32{let n=0;if false{return n;}1}",
        "fn chosen()->f64{let n=0.0;if false{return n;}1.0}",
    ] {
        for edition in [2015, 2018, 2021, 2024] {
            let (parsed, body, index) = entered(source, edition, "chosen");
            assert_eq!(
                selected(&parsed, &body, &index, 0),
                Some(DefinitionNumericContext::Result),
                "{edition}: {source}"
            );
        }
    }
}

#[test]
fn incompatible_cold_result_does_not_override_compatible_parameter() {
    for source in [
        "fn chosen(v:u32)->bool{let n=0;if false{return n;}v==n}",
        "fn chosen(v:u32)->f64{let n=0;if false{return n;}let _=v==n;1.0}",
        "fn chosen(v:f64)->u32{let n=0.0;if false{return n;}let _=v==n;1}",
    ] {
        for edition in [2015, 2018, 2021, 2024] {
            let (parsed, body, index) = entered(source, edition, "chosen");
            assert_eq!(
                selected(&parsed, &body, &index, 0),
                Some(DefinitionNumericContext::Parameter(0)),
                "{edition}: {source}"
            );
        }
    }
}

#[test]
fn conditional_family_requires_exact_owning_result_binding() {
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index) = entered(
            // An unexpanded item macro may introduce `f32`; only the conditional
            // owning-result relation remains (a builtin inert attribute cannot).
            "fn chosen()->f32{let n=0.0;n}m!{}",
            edition,
            "chosen",
        );
        assert_eq!(
            selected(&parsed, &body, &index, 0),
            Some(DefinitionNumericContext::ResultOrDefault { floating: true })
        );
        let (parsed, body, index) = entered(
            "fn chosen()->bool{let n=0;{let n=0;if false{return n;}}true}",
            edition,
            "chosen",
        );
        assert_eq!(selected(&parsed, &body, &index, 0), None);
    }
}

#[test]
fn unchanged_borrowed_compound_original_uses_written_binding_and_reference_sources_all_editions() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/binop/compound-assign-by-ref.rs"
    );
    let expected = [
        (PrimitiveType::I8, PrimitiveType::I8),
        (PrimitiveType::I16, PrimitiveType::I16),
        (PrimitiveType::F32, PrimitiveType::F32),
        (PrimitiveType::F64, PrimitiveType::F64),
        (PrimitiveType::I64, PrimitiveType::I64),
        (PrimitiveType::U8, PrimitiveType::U8),
        (PrimitiveType::U16, PrimitiveType::U16),
        (PrimitiveType::U32, PrimitiveType::U32),
        (PrimitiveType::U64, PrimitiveType::U32),
        (PrimitiveType::U64, PrimitiveType::I16),
    ];
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index) = entered(ORIGINAL, edition, "main");
        let root = parsed.entered_body_root(&body).unwrap();
        let compounds: Vec<_> = root
            .descendants()
            .filter_map(ast::BinExpr::cast)
            .filter(|binary| {
                binary.op_token().is_some_and(|token| {
                    matches!(
                        token.text(),
                        "+=" | "-=" | "*=" | "/=" | "%=" | "|=" | "&=" | "^=" | "<<=" | ">>="
                    )
                })
            })
            .collect();
        assert_eq!(compounds.len(), expected.len());
        for (binary, (left, right)) in compounds.into_iter().zip(expected) {
            let node = id_for(&index, &body, binary.syntax()).unwrap();
            let pc = index.pc(node, Phase::Entry).unwrap();
            let recipe = parsed.initial_thunk(&body, &index, &pc).unwrap();
            let operations: Vec<_> = recipe
                .recipe()
                .operations
                .iter()
                .filter(|op| !matches!(op, Op::ScopeEnter { .. }))
                .cloned()
                .collect();
            let [
                Op::CompoundHint {
                    child,
                    kind:
                        AssignKind::Primitive {
                            left: actual_left,
                            right: actual_right,
                        },
                    ..
                },
                Op::SetHint {
                    child: reference_slot,
                    hint:
                        HintRecipe::Reference {
                            region,
                            mutable: false,
                            target,
                        },
                },
                Op::SetHint {
                    hint: HintRecipe::Primitive(computed),
                    ..
                },
            ] = operations.as_slice()
            else {
                panic!(
                    "edition {edition}: original compound lost its source preparation: {operations:?}"
                );
            };
            assert_eq!(*actual_left, left);
            assert_eq!(*actual_right, right);
            assert_eq!(*target, right);
            assert_eq!(*computed, left);
            assert_eq!(child, reference_slot);
            let reference = binary
                .rhs()
                .and_then(|expression| super::super::compound_shared_rhs(expression).unwrap())
                .unwrap();
            let range = reference.syntax().text_range();
            assert_eq!(
                *region,
                crate::traits::term::RegionTerm::Local {
                    definition: body.def_key,
                    start: u32::from(range.start()),
                    end: u32::from(range.end()),
                }
            );
            // This preparation cannot evaluate or borrow the operand itself.
            assert!(recipe.recipe().operations.iter().all(|op| matches!(
                op,
                Op::ScopeEnter { .. } | Op::CompoundHint { .. } | Op::SetHint { .. }
            )));
            let rhs = binary.rhs().unwrap();
            let rhs_id = id_for(&index, &body, rhs.syntax()).unwrap();
            assert_eq!(
                recipe.recipe().exit,
                crate::ir::recipe::RecipeExit::Next(index.pc(rhs_id, Phase::Entry).unwrap())
            );
        }
    }
}

#[test]
fn borrowed_compound_does_not_invent_source_for_unknown_or_nested_reference_operands() {
    // A local without a primitive or exact definition-numeric proof for both
    // operands is the updated place itself: no source is invented before
    // operands; the reached operand types select the builtin compound
    // operation or `AddAssign::add_assign(&mut x, rhs)`. Programs rustc
    // rejects (`x += &&2i8` has no `AddAssign<&&i8>` impl) are not checked
    // statically; they fault at run time when the operator selects no method.
    for source in [
        "fn main(){let mut x=missing();x+=&1i8;}",
        "fn main(){let mut x=1i8;x+=&&2i8;}",
        "fn main(){let mut x=1i8;x+=&mut 2i8;}",
        "fn main(){let mut x=1i8;x+=&raw const 2i8;}",
        "fn main(){let mut x=1i8;x+=&missing();}",
    ] {
        let (parsed, body, index) = entered(source, 2024, "main");
        let binary = parsed
            .entered_body_root(&body)
            .unwrap()
            .descendants()
            .filter_map(ast::BinExpr::cast)
            .find(|binary| binary.op_token().is_some_and(|token| token.text() == "+="))
            .unwrap();
        let node = id_for(&index, &body, binary.syntax()).unwrap();
        let recipe = parsed
            .initial_thunk(&body, &index, &index.pc(node, Phase::Entry).unwrap())
            .unwrap_or_else(|error| panic!("{source}: {error:?}"));
        assert!(recipe.recipe().operations.is_empty(), "{source}");
        let rhs = id_for(&index, &body, binary.rhs().unwrap().syntax()).unwrap();
        assert_eq!(
            recipe.recipe().exit,
            crate::ir::recipe::RecipeExit::Next(index.pc(rhs, Phase::Entry).unwrap()),
            "{source}"
        );
    }
}
