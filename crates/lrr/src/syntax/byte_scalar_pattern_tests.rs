//! Genuine original source phases and scoped execution of its two byte matches.
//! The earlier byte-string setup and later static/slice/Vec obligations are NOT
//! executed or counted as a whole-original-main runtime-contract pass.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    runtime::{cell::GuestStore, value::ValueRepr},
    source::snapshot::{CfgSet, SourceBlob},
    vm::{
        frame::Frame,
        step::{StepOutcome, step},
    },
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/byte-literals.rs"
);
fn entered(text: &str, edition: u16) -> (ParsedFile, BodyHandle, BodyIndex, Vec<ast::MatchExpr>) {
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
            Digest::of_bytes(b"LRR unchanged byte original component"),
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
    let matches = parsed
        .entered_body_root(&body)
        .unwrap()
        .descendants()
        .filter_map(ast::MatchExpr::cast)
        .collect();
    (parsed, body, index, matches)
}
fn id(index: &BodyIndex, body: &BodyHandle, syntax: &SyntaxNode) -> NodeId {
    super::super::id_for(index, body, syntax).unwrap()
}
fn phase(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    syntax: &SyntaxNode,
    phase: Phase,
) -> VerifiedRecipe {
    parsed
        .initial_thunk(
            body,
            index,
            &index.pc(id(index, body, syntax), phase).unwrap(),
        )
        .unwrap()
}
fn bytes(recipe: &VerifiedRecipe) -> Vec<(String, u8)> {
    recipe
        .recipe()
        .operations
        .iter()
        .filter_map(|op| match op {
            Op::Literal {
                literal,
                hint: HintRecipe::Primitive(PrimitiveType::U8),
                ..
            } => {
                let Some(DecodedLiteral::Byte(value)) = &literal.decoded else {
                    panic!("byte phase lost pinned decoded payload")
                };
                assert_eq!(literal.kind, LiteralKind::Byte);
                Some((literal.spelling.clone(), *value))
            }
            _ => None,
        })
        .collect()
}
#[test]
fn unchanged_original_byte_equality_and_range_have_exact_u8_thunks_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index, matches) = entered(ORIGINAL, edition);
        // The third ordinary match is byte-string/slice, an explicit remaining
        // feature. The fourth is inside assert_eq's opaque token tree.
        assert_eq!(matches.len(), 3);
        for (ordinal, expected) in [
            (0, vec![("b'*'", 42)]),
            (1, vec![("b'a'", 97), ("b'z'", 122)]),
        ] {
            let matched = &matches[ordinal];
            let entry = phase(&parsed, &body, &index, matched.syntax(), Phase::Entry);
            assert!(entry.recipe().operations.iter().any(|op| matches!(
                op,
                Op::SetHint {
                    hint: HintRecipe::Primitive(PrimitiveType::U8),
                    ..
                }
            )));
            let pattern = matched
                .match_arm_list()
                .unwrap()
                .arms()
                .next()
                .unwrap()
                .pat()
                .unwrap();
            let lower = phase(&parsed, &body, &index, pattern.syntax(), Phase::Entry);
            let mut written = bytes(&lower);
            assert!(lower.recipe().operations.iter().any(|op| matches!(op,
                Op::NumericBinary{operator,..} if *operator==if ordinal==0{NumericBinary::Eq}else{NumericBinary::Ge})));
            if ordinal == 1 {
                let upper = phase(&parsed, &body, &index, pattern.syntax(), Phase::Operand(0));
                assert!(matches!(
                    upper.recipe().operations.first(),
                    Some(Op::Discard { .. })
                ));
                assert!(upper.recipe().operations.iter().any(|op| matches!(
                    op,
                    Op::NumericBinary {
                        operator: NumericBinary::Le,
                        ..
                    }
                )));
                written.extend(bytes(&upper));
            }
            assert_eq!(
                written,
                expected
                    .into_iter()
                    .map(|(s, v)| (s.to_string(), v))
                    .collect::<Vec<_>>()
            );
            for recipe in [&lower] {
                for op in &recipe.recipe().operations {
                    if let Op::Literal { literal, .. } = op {
                        let start = body.range.start + literal.source.relative_start;
                        let end = body.range.start + literal.source.relative_end;
                        assert_eq!(&ORIGINAL[start as usize..end as usize], literal.spelling);
                    }
                }
            }
        }
    }
}
struct Component {
    frame: Frame,
    store: GuestStore,
    visited: Vec<PcKey>,
}
fn component(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    matched: &ast::MatchExpr,
) -> Result<Component, Diagnostic> {
    let matched_id = id(index, body, matched.syntax());
    let start = index.pc(matched_id, Phase::Entry)?;
    let first = parsed.initial_thunk(body, index, &start)?;
    let mut store = GuestStore::new(1, 10_000, 16 << 20)?;
    let mut frame = Frame::new(index.clone(), start, &mut store)?;
    frame.set_fuel_limit(10_000)?;
    // Component setup enters only real original ancestor scopes. No source
    // declaration, initializer, scrutinee, hint, cell or recipe is substituted.
    let node = &index.nodes()[matched_id.0 as usize];
    let mut cursor = match first.recipe().operations.first() {
        Some(Op::ScopeEnter { scope }) => index.scopes()[scope.0 as usize].parent,
        _ => Some(node.scope),
    };
    let mut ancestors = Vec::new();
    while let Some(scope) = cursor {
        ancestors.push(scope);
        cursor = index.scopes()[scope.0 as usize].parent;
    }
    for scope in ancestors.into_iter().rev() {
        frame.enter(scope)?;
    }
    let continuation = return_pc(index, body, matched.syntax())?;
    let mut visited = Vec::new();
    for _ in 0..128 {
        let pc = frame.pc().clone();
        let selected = &index.nodes()[pc.node as usize];
        if pc == continuation {
            return Ok(Component {
                frame,
                store,
                visited,
            });
        }
        assert!(
            selected.range.start >= node.range.start && selected.range.end <= node.range.end,
            "component escaped the actual Match before its original continuation"
        );
        visited.push(pc.clone());
        let recipe = parsed.initial_thunk(body, index, &pc)?;
        match step(&mut frame, &mut store, &recipe)? {
            StepOutcome::Next(_) | StepOutcome::Dispatch => {}
            other => panic!("actual byte component produced unexpected boundary: {other:?}"),
        }
        for op in &recipe.recipe().operations {
            if let Op::Literal {
                literal,
                destination,
                ..
            } = op
                && matches!(
                    literal.decoded,
                    Some(DecodedLiteral::Integer { suffix: None, .. })
                )
            {
                assert!(
                    frame.source_view(*destination)?.is_none(),
                    "context constrained integer incorrectly acquired Byte source identity"
                );
                let descriptor =
                    crate::runtime::shape::PrimitiveIdentity::from_recipe(PrimitiveType::U8)?.id();
                assert_eq!(
                    store.read_scalar(frame.cell(*destination)?)?.type_id,
                    descriptor.0,
                    "actual Byte pattern did not constrain its original integer scrutinee"
                );
            }
        }
    }
    panic!("actual byte component exceeded bounded source phases")
}
#[test]
fn unchanged_original_byte_matches_execute_only_their_actual_scoped_components_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index, matches) = entered(ORIGINAL, edition);
        for (ordinal, comparisons) in [(0, 1), (1, 2)] {
            let matched = &matches[ordinal];
            let done = component(&parsed, &body, &index, matched).unwrap();
            let matched_id = id(&index, &body, matched.syntax());
            let destination = index.nodes()[matched_id.0 as usize].value.unwrap();
            assert_eq!(
                done.store
                    .read_scalar(done.frame.cell(destination).unwrap())
                    .unwrap(),
                ValueRepr::unit()
            );
            assert_eq!(done.frame.counters.started_by_opcode[10], comparisons);
            assert_eq!(done.frame.counters.started_by_opcode[0], comparisons + 1);
            assert_eq!(done.frame.counters.started_by_opcode[25], 0);
            assert_eq!(done.frame.counters.started_by_opcode[26], 0);
            assert_eq!(
                done.frame.counters.operations_started,
                done.frame.counters.operations_completed
            );
            let scrutinee = id(&index, &body, matched.expr().unwrap().syntax());
            assert!(
                done.frame
                    .cell(index.nodes()[scrutinee.0 as usize].value.unwrap())
                    .is_err()
            );
            for temp in index.nodes()[matched_id.0 as usize]
                .runtime_temporaries
                .into_iter()
                .flatten()
            {
                assert!(
                    done.frame.cell(temp).is_err(),
                    "byte comparison temporary retained after success"
                );
            }
            let first_arm = matched.match_arm_list().unwrap().arms().next().unwrap();
            let arm_range = first_arm.syntax().text_range();
            let arm = index
                .nodes()
                .iter()
                .find(|node| {
                    node.role == NodeRole::MatchArm
                        && node.range.start + body.range.start == u32::from(arm_range.start())
                        && node.range.end + body.range.start == u32::from(arm_range.end())
                })
                .unwrap();
            assert!(
                !done.frame.active_scopes().contains(&arm.scope),
                "actual selected arm scope was not retired"
            );
            assert!(
                done.visited
                    .iter()
                    .all(|pc| index.nodes()[pc.node as usize].role
                        != NodeRole::Expression(ExpressionKind::Macro))
            );
        }
    }
}
#[test]
fn byte_range_unknown_or_wrong_family_upper_is_cold_until_lower_matches() {
    for edition in [2015, 2018, 2021, 2024] {
        for upper in ["UNKNOWN", "'x'"] {
            let text = format!("fn main(){{match 1u8{{b'z'..={upper}=>{{}},_=>{{}}}}}}");
            let (parsed, body, index, matches) = entered(&text, edition);
            let pattern = matches[0]
                .match_arm_list()
                .unwrap()
                .arms()
                .next()
                .unwrap()
                .pat()
                .unwrap();
            let pattern_id = id(&index, &body, pattern.syntax());
            let done = component(&parsed, &body, &index, &matches[0]).unwrap();
            assert_eq!(done.frame.counters.started_by_opcode[10], 1);
            assert!(
                !done
                    .visited
                    .contains(&index.pc(pattern_id, Phase::Operand(0)).unwrap())
            );
            let upper = phase(&parsed, &body, &index, pattern.syntax(), Phase::Operand(0));
            let Some(Op::Unsupported(error)) = upper.recipe().operations.first() else {
                panic!("upper frontier was not retained")
            };
            assert_eq!(error.reached_unsupported().code, "LRR-E-UNSUPPORTED");
            assert_eq!(
                error.message,
                if text.contains("UNKNOWN") {
                    "reached range endpoint requires an original integer, floating, char or byte literal"
                } else {
                    "range endpoints require the same supported literal family"
                }
            );
        }
        for text in [
            "fn main(){match 97u8{-b'a'=>{},_=>{}}}",
            "fn main(){match 97u8{-b'a'..=b'z'=>{},_=>{}}}",
        ] {
            let parsed = ParsedFile::parse(
                &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
                ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            // Pinned is_literal_pat_start accepts '-' only before INT/FLOAT.
            // Preserve its real error rather than constructing a fake -Byte AST.
            assert!(
                !parsed.errors.is_empty(),
                "negative byte was silently accepted as a source pattern"
            );
            for pattern in parsed
                .parsed
                .tree()
                .syntax()
                .descendants()
                .filter_map(ast::LiteralPat::cast)
            {
                if pattern
                    .literal()
                    .is_some_and(|literal| literal.token().kind() == SyntaxKind::BYTE)
                {
                    assert!(
                        pattern.minus_token().is_none(),
                        "parser minted unsupported negative Byte evidence"
                    );
                }
            }
        }
    }
}
