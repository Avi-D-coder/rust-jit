//! LRR route/cleanup controls supplementing the unchanged upstream range bank.
//! Compilation/execution is owned by the coordinator's sole validator.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{
        pc::{NodeId, Phase},
        recipe::{RecipeExit, VerifiedRecipe},
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{body_index::BodyIndex, parse::ParsedFile},
};

const ORIGINAL: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/match-range.rs");

fn entered(text: &str, edition: u16) -> (ParsedFile, BodyHandle, BodyIndex) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"shared scalar pattern route control"),
            vec![],
            &CfgSet::guest_default(),
        )
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
    (parsed, body, index)
}
fn recipe(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    syntax: &SyntaxNode,
    phase: Phase,
) -> VerifiedRecipe {
    let id = super::super::id_for(index, body, syntax).unwrap();
    let (recipe, lowerings) = observe_lowerings(|| {
        parsed
            .initial_thunk(body, index, &index.pc(id, phase).unwrap())
            .unwrap()
    });
    assert_eq!(
        lowerings, 1,
        "ordinary thunk must enter the common scalar test lowerer"
    );
    recipe
}
fn comparison(
    recipe: &VerifiedRecipe,
) -> (LiteralKind, String, Option<DecodedLiteral>, NumericBinary) {
    let (literal, operator) = (
        recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| match op {
                Op::Literal { literal, .. } => Some(literal),
                _ => None,
            })
            .unwrap(),
        recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| match op {
                Op::NumericBinary { operator, .. } => Some(*operator),
                _ => None,
            })
            .unwrap(),
    );
    (
        literal.kind,
        literal.spelling.clone(),
        literal.decoded.clone(),
        operator,
    )
}
fn condition_phase(index: &BodyIndex, body: &BodyHandle, condition: &ast::LetExpr) -> Phase {
    let id = super::super::id_for(index, body, condition.syntax()).unwrap();
    let source = super::super::id_for(index, body, condition.expr().unwrap().syntax()).unwrap();
    Phase::Operand(
        u16::try_from(
            index.nodes()[id.0 as usize]
                .operands
                .iter()
                .position(|operand| *operand == source)
                .unwrap(),
        )
        .unwrap(),
    )
}

#[test]
fn unchanged_upstream_range_bank_and_condition_adapters_use_one_test_route_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index) = entered(ORIGINAL, edition);
        let matches: Vec<_> = parsed
            .entered_body_root(&body)
            .unwrap()
            .descendants()
            .filter_map(ast::MatchExpr::cast)
            .collect();
        assert_eq!(matches.len(), 11);
        for expression in matches {
            let pattern = expression
                .match_arm_list()
                .unwrap()
                .arms()
                .next()
                .unwrap()
                .pat()
                .unwrap();
            let source = expression.expr().unwrap();
            // Exact input byte slices; adaptations live outside the imported
            // original and are supplemental route evidence, not fixture passes.
            let pattern_range = pattern.syntax().text_range();
            let source_range = source.syntax().text_range();
            let control = format!(
                "fn main(){{if let {}={}{{}}}}",
                &ORIGINAL[u32::from(pattern_range.start()) as usize
                    ..u32::from(pattern_range.end()) as usize],
                &ORIGINAL[u32::from(source_range.start()) as usize
                    ..u32::from(source_range.end()) as usize]
            );
            let (conditional, condition_body, condition_index) = entered(&control, edition);
            let condition = conditional
                .entered_body_root(&condition_body)
                .unwrap()
                .descendants()
                .find_map(ast::LetExpr::cast)
                .unwrap();
            let ordinary_first = recipe(&parsed, &body, &index, pattern.syntax(), Phase::Entry);
            let conditional_first = recipe(
                &conditional,
                &condition_body,
                &condition_index,
                condition.syntax(),
                condition_phase(&condition_index, &condition_body, &condition),
            );
            assert_eq!(comparison(&ordinary_first), comparison(&conditional_first));
            let ordinary_upper =
                recipe(&parsed, &body, &index, pattern.syntax(), Phase::Operand(0));
            let conditional_upper = recipe(
                &conditional,
                &condition_body,
                &condition_index,
                condition.syntax(),
                Phase::Commit,
            );
            assert_eq!(comparison(&ordinary_upper), comparison(&conditional_upper));
            for prepared in [
                &ordinary_first,
                &conditional_first,
                &ordinary_upper,
                &conditional_upper,
            ] {
                // Testing never consumes the scrutinee, creates bindings or
                // acquires leases. Only its expected-literal scratch is retired.
                assert!(!prepared.recipe().operations.iter().any(|op| matches!(
                    op,
                    Op::BindLocal { .. }
                        | Op::ReadPlace { .. }
                        | Op::Borrow { .. }
                        | Op::ValuePlace { .. }
                        | Op::ScopeEnter { .. }
                        | Op::ScopeLeave { .. }
                )));
                assert_eq!(
                    prepared
                        .recipe()
                        .operations
                        .iter()
                        .filter(|op| matches!(op, Op::Literal { .. }))
                        .count(),
                    1
                );
            }
        }
    }
}

#[test]
fn bool_byte_and_open_ranges_have_the_same_match_and_condition_test_semantics() {
    for edition in [2015, 2018, 2021, 2024] {
        for (pattern, source) in [
            ("true", "true"),
            ("false", "true"),
            ("b'a'", "b'a'"),
            ("..3_i32", "2_i32"),
            ("..=3_i32", "2_i32"),
            ("2_i32..", "2_i32"),
        ] {
            let ordinary = format!("fn main(){{match {source}{{{pattern} =>(),_=>()}}}}");
            let conditional = format!("fn main(){{if let {pattern}={source}{{}}}}");
            let (parsed, body, index) = entered(&ordinary, edition);
            let matched = parsed
                .entered_body_root(&body)
                .unwrap()
                .descendants()
                .find_map(ast::MatchExpr::cast)
                .unwrap();
            let pattern = matched
                .match_arm_list()
                .unwrap()
                .arms()
                .next()
                .unwrap()
                .pat()
                .unwrap();
            let ordinary = recipe(&parsed, &body, &index, pattern.syntax(), Phase::Entry);
            assert!(
                matches!(&ordinary.recipe().exit, RecipeExit::Branch { yes, .. }
                if Phase::from_code(yes.phase) == Phase::Commit)
            );
            let (parsed, body, index) = entered(&conditional, edition);
            let condition = parsed
                .entered_body_root(&body)
                .unwrap()
                .descendants()
                .find_map(ast::LetExpr::cast)
                .unwrap();
            let conditional = recipe(
                &parsed,
                &body,
                &index,
                condition.syntax(),
                condition_phase(&index, &body, &condition),
            );
            assert_eq!(comparison(&ordinary), comparison(&conditional));
            assert!(matches!(&conditional.recipe().exit, RecipeExit::Next(next)
                if Phase::from_code(next.phase) == Phase::Exit));
        }
    }
}

#[test]
fn shared_pattern_literal_source_cannot_cross_the_owning_body() {
    let (parsed, body, index) = entered("fn main(){match 2_i32{1_i32..=3_i32=>(),_=>()}}", 2024);
    let pattern = parsed
        .entered_body_root(&body)
        .unwrap()
        .descendants()
        .find_map(ast::RangePat::cast)
        .unwrap();
    let plan = ScalarTestPlan::from_source(&ast::Pat::RangePat(pattern)).unwrap();
    let mut key = index.pc(NodeId(0), Phase::Entry).unwrap();
    key.body.definition = Digest([41; 32]);
    assert!(
        matches!(plan.first(&parsed, &body, &key), Err(error) if error.code == "LRR-E-INTERNAL-SNAPSHOT")
    );
}
