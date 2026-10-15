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
        assert_eq!(
            selected(&parsed, &body, &index, 0),
            Some(DefinitionNumericContext::ResultOrDefault { floating: false })
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
                hint: HintRecipe::DefinitionNumeric(DefinitionNumericContext::ResultOrDefault {
                    floating: false
                }),
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
                        context: DefinitionNumericContext::ResultOrDefault { floating: false },
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
            .find(|s| s.syntax().text().to_string() == "let r=0;")
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
            "fn chosen()->f32{let n=0.0;n}#[allow(dead_code)]fn cold(){}",
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
