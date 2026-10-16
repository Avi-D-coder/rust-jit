//! Original runtime and source-phase controls; all are pending Rust execution.
use super::*;
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database},
    engine::run::{RunMachine, RunPoll, ValueSummary},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{literal_decode::observe_integer_decoding, parse::ParseKey},
};

const INFERENCE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/match-range-infer.rs"
);
const RANGES: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/match-range.rs");
const NEGATIVE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/match/match-on-negative-integer-ranges.rs"
);

fn entered(text: &str, edition: u16, name: &str) -> (ParsedFile, BodyHandle, BodyIndex) {
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
            Digest::of_bytes(b"original scalar range source control"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
        .body
        .clone()
        .unwrap();
    let index = parsed.index_body(&body, Default::default()).unwrap();
    (parsed, body, index)
}

fn range_recipe(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    range: &ast::RangePat,
    phase: Phase,
) -> VerifiedRecipe {
    let id = super::super::id_for(index, body, range.syntax()).unwrap();
    // Exercise the ordinary captured syntax selector/dispatch as production
    // does; the separate RANGE_PAT wrapper attachment is a real prerequisite.
    parsed
        .initial_thunk(body, index, &index.pc(id, phase).unwrap())
        .unwrap()
}

#[test]
fn unchanged_integer_and_floating_range_whole_mains_require_completion_all_editions() {
    // This is an actual original-main interpreter control, not a synthetic
    // comparator or expected Unsupported outcome. It grants no configured
    // compiletest/full-fixture authority or guest-output capture evidence.
    for (path, original, expected_numeric_ops) in [
        ("tests/ui/binding/match-range-infer.rs", INFERENCE, 6),
        ("tests/ui/binding/match-range.rs", RANGES, 22),
    ] {
        for edition in [2015, 2018, 2021, 2024] {
            let profile = ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            };
            let cfg = CfgSet::guest_default();
            let path = SourcePath::new(path).unwrap();
            let blob = SourceBlob::new(original.as_bytes().to_vec()).unwrap();
            let unit = Digest::of_bytes(b"unchanged match-range-infer original runtime control");
            let selection = SourceSelection {
                path: path.clone(),
                outline: OutlineKey {
                    parse: ParseKey {
                        content: blob.content(),
                        profile: profile.clone(),
                    },
                    crate_key: unit,
                    module_path: vec![],
                    cfg: cfg.fingerprint(),
                },
            };
            let inputs = PreparedSourceInputs::refresh(
                None,
                vec![SourceUnitInput {
                    path,
                    crate_key: unit,
                    module_path: vec![],
                    profile,
                    cfg,
                    blob,
                }],
                Default::default(),
            )
            .unwrap();
            let mut db = Database::default();
            inputs.publish(&mut db).unwrap();
            let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
                panic!("original range inference source absent")
            };
            let main = view
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "main")
                .unwrap()
                .body
                .as_ref()
                .unwrap()
                .key();
            db.begin_run().unwrap();
            let mut run =
                RunMachine::start_with_sources(&mut db, main, 1_000_000, vec![view.share()])
                    .into_ready()
                    .unwrap()
                    .unwrap();
            let outcome = run.poll(&mut db);
            db.finish_run().unwrap();
            let report = match outcome {
                RunPoll::Completed(report) => report,
                RunPoll::Fault(error) => panic!("unchanged range inference failed: {error:?}"),
                RunPoll::Panicked(report) => {
                    panic!("unchanged range inference panicked: {report:?}")
                }
                RunPoll::NeedInput(input) => {
                    panic!("unchanged range inference suspended: {input:?}")
                }
            };
            assert_eq!(report.live_edition, edition);
            assert!(matches!(report.value, ValueSummary::Unit));
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert_eq!(
                report.opcode_counters.started_by_opcode[10],
                expected_numeric_ops
            );
            assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        }
    }
}

#[test]
fn original_range_boundaries_char_and_suffix_context_have_exact_phases_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index) = entered(RANGES, edition, "main");
        let root = parsed.entered_body_root(&body).unwrap();
        let matches: Vec<_> = root
            .descendants()
            .filter_map(ast::MatchExpr::cast)
            .collect();
        // Preserve every original match, including all four floating ranges.
        assert_eq!(matches.len(), 11);
        for (ordinal, matched) in matches.iter().enumerate() {
            let range = matched
                .syntax()
                .descendants()
                .find_map(ast::RangePat::cast)
                .unwrap();
            let id = super::super::id_for(&index, &body, range.syntax()).unwrap();
            let lower = range_recipe(&parsed, &body, &index, &range, Phase::Entry);
            let upper = range_recipe(&parsed, &body, &index, &range, Phase::Operand(0));
            let [
                Op::Pattern {
                    operation:
                        PatternOperation::Begin {
                            source: begin_source,
                            ..
                        },
                },
                Op::Literal {
                    literal: lo,
                    hint: low_hint,
                    destination: expected,
                },
                Op::NumericBinary {
                    left: source,
                    right,
                    destination: test,
                    operator: NumericBinary::Ge,
                },
                Op::Discard { source: discarded },
            ] = lower.recipe().operations.as_ref()
            else {
                panic!("original lower range phase shape")
            };
            assert_eq!(right, expected);
            assert_eq!(begin_source, source);
            assert_eq!(discarded, expected);
            let [
                Op::Discard { source: prior },
                Op::Literal {
                    literal: hi,
                    hint: upper_hint,
                    destination: upper_expected,
                },
                Op::NumericBinary {
                    left: upper_source,
                    right: upper_right,
                    destination: upper_test,
                    operator,
                },
                Op::Discard {
                    source: upper_discarded,
                },
            ] = upper.recipe().operations.as_ref()
            else {
                panic!("original upper range phase shape")
            };
            assert_eq!(prior, test);
            assert_eq!(upper_expected, expected);
            assert_eq!(upper_source, source);
            assert_eq!(upper_right, expected);
            assert_eq!(upper_test, test);
            assert_eq!(upper_discarded, expected);
            assert_eq!(
                *operator,
                if matches!(ordinal, 1 | 3 | 9 | 10) {
                    NumericBinary::Lt
                } else {
                    NumericBinary::Le
                }
            );
            let ty = if ordinal >= 7 {
                PrimitiveType::F64
            } else if ordinal == 5 {
                PrimitiveType::Char
            } else if ordinal == 6 {
                PrimitiveType::I32
            } else {
                PrimitiveType::Usize
            };
            assert_eq!(low_hint, &HintRecipe::Primitive(ty));
            assert_eq!(upper_hint, &HintRecipe::Primitive(ty));
            for literal in [lo, hi] {
                assert_eq!(literal.source.body_key, body.key());
                assert!(literal.source.relative_start >= index.nodes()[id.0 as usize].range.start);
                assert!(literal.source.relative_end <= index.nodes()[id.0 as usize].range.end);
                let original = &RANGES[(body.range.start + literal.source.relative_start) as usize
                    ..(body.range.start + literal.source.relative_end) as usize];
                assert_eq!(original, literal.spelling);
            }
            assert_eq!(
                lower.recipe().exit,
                RecipeExit::Branch {
                    condition: *test,
                    yes: index.pc(id, Phase::Operand(0)).unwrap(),
                    no: index.pc(id, Phase::Exit).unwrap(),
                }
            );
            assert_eq!(
                upper.recipe().exit,
                RecipeExit::Branch {
                    condition: *test,
                    yes: index.pc(id, Phase::Commit).unwrap(),
                    no: index.pc(id, Phase::Exit).unwrap(),
                }
            );
        }
        let (parsed, body, index) = entered(INFERENCE, edition, "main");
        let ranges: Vec<_> = parsed
            .entered_body_root(&body)
            .unwrap()
            .descendants()
            .filter_map(ast::RangePat::cast)
            .collect();
        assert_eq!(ranges.len(), 3);
        for (range, ty) in
            ranges
                .iter()
                .zip([PrimitiveType::I32, PrimitiveType::U16, PrimitiveType::U16])
        {
            let recipe = range_recipe(&parsed, &body, &index, range, Phase::Entry);
            assert!(
                matches!(recipe.recipe().operations.iter().find(|op| matches!(op, Op::Literal { .. })), Some(Op::Literal { hint: HintRecipe::Primitive(actual), .. }) if *actual == ty)
            );
        }
    }
}

#[test]
fn original_macro_ranges_stay_opaque_and_lrr_signed_min_preserves_its_source() {
    for edition in [2015, 2018, 2021, 2024] {
        let (parsed, body, index) = entered(NEGATIVE, edition, "main");
        // These unchanged original matches occur inside assertion token trees.
        // Ordinary body indexing must not manufacture an entered Match body.
        assert!(
            parsed
                .entered_body_root(&body)
                .unwrap()
                .descendants()
                .find_map(ast::RangePat::cast)
                .is_none()
        );
        assert!(index.nodes().iter().all(|node| !node.scalar_range_phase));
        // This separately owned LRR supplement exercises the primitive MIN
        // boundary without pretending to be that original macro operand.
        const SUPPLEMENT: &str = "fn chosen(c:i8){match c{-128i8..=-101i8=>(),_=>()}}";
        let (parsed, body, index) = entered(SUPPLEMENT, edition, "chosen");
        let range = parsed
            .entered_body_root(&body)
            .unwrap()
            .descendants()
            .find_map(ast::RangePat::cast)
            .unwrap();
        let recipe = range_recipe(&parsed, &body, &index, &range, Phase::Entry);
        let Some(Op::Literal {
            literal,
            hint: HintRecipe::Primitive(PrimitiveType::I8),
            ..
        }) = recipe
            .recipe()
            .operations
            .iter()
            .find(|op| matches!(op, Op::Literal { .. }))
        else {
            panic!("LRR direct range signed MIN instruction")
        };
        assert_eq!(literal.kind, LiteralKind::NegativeInteger);
        assert_eq!(literal.spelling, "-128i8");
        assert!(matches!(
            literal.decoded,
            Some(DecodedLiteral::Integer {
                negative: true,
                magnitude: 128,
                suffix: Some(PrimitiveType::I8),
                ..
            })
        ));
        assert_eq!(
            &SUPPLEMENT[(body.range.start + literal.source.relative_start) as usize
                ..(body.range.start + literal.source.relative_end) as usize],
            "-128i8"
        );
    }
}

#[test]
fn later_range_and_upper_magnitude_decode_only_at_their_reached_phase() {
    for edition in [2015, 2018, 2021, 2024] {
        let source = "fn chosen(){match 2u8{2=>(),0..=999999999999999999999999999999999999999999999999999999=>(),_=>()}}";
        let (parsed, body, index) = entered(source, edition, "chosen");
        let root = parsed.entered_body_root(&body).unwrap();
        let matched = root.descendants().find_map(ast::MatchExpr::cast).unwrap();
        let arm = matched.match_arm_list().unwrap().arms().next().unwrap();
        let pattern = arm.pat().unwrap();
        let matched_id = super::super::id_for(&index, &body, matched.syntax()).unwrap();
        let pattern_id = super::super::id_for(&index, &body, pattern.syntax()).unwrap();
        let (_, early) = observe_integer_decoding(|| {
            prepare(
                &parsed,
                &body,
                &index,
                &index.pc(matched_id, Phase::Entry).unwrap(),
                matched.syntax().clone(),
            )
            .unwrap();
            prepare(
                &parsed,
                &body,
                &index,
                &index.pc(pattern_id, Phase::Entry).unwrap(),
                pattern.syntax().clone(),
            )
            .unwrap();
        });
        assert_eq!(early.attempts, 1);
        let range = matched
            .syntax()
            .descendants()
            .find_map(ast::RangePat::cast)
            .unwrap();
        let (_, lower) =
            observe_integer_decoding(|| range_recipe(&parsed, &body, &index, &range, Phase::Entry));
        assert_eq!(lower.attempts, 1);
        assert_eq!(lower.failures, 0);
        let (upper, late) = observe_integer_decoding(|| {
            range_recipe(&parsed, &body, &index, &range, Phase::Operand(0))
        });
        assert_eq!(late.attempts, 1);
        assert_eq!(late.failures, 1);
        assert!(upper.recipe().operations.iter().any(|op| matches!(
            op,
            Op::Literal {
                literal: LiteralRecipe {
                    decoded: Some(DecodedLiteral::InvalidInteger { .. }),
                    ..
                },
                ..
            }
        )));
    }
}

#[test]
fn guarded_range_commits_before_guard_and_keeps_plain_binding_admission_separate() {
    for edition in [2015, 2018, 2021, 2024] {
        let source = "fn chosen(c:u8)->u8{match c{0..=4 if false=>1,v if v==9=>2,_=>3}}";
        let (parsed, body, index) = entered(source, edition, "chosen");
        let matched = parsed
            .entered_body_root(&body)
            .unwrap()
            .descendants()
            .find_map(ast::MatchExpr::cast)
            .unwrap();
        let arms: Vec<_> = matched.match_arm_list().unwrap().arms().collect();
        let ast::Pat::RangePat(range) = arms[0].pat().unwrap() else {
            panic!("range control")
        };
        let commit = range_recipe(&parsed, &body, &index, &range, Phase::Commit);
        let range_id = super::super::id_for(&index, &body, range.syntax()).unwrap();
        let arm_id = index.nodes()[range_id.0 as usize].parent.unwrap();
        assert_eq!(index.nodes()[arm_id.0 as usize].role, NodeRole::MatchArm);
        assert!(
            matches!(commit.recipe().operations.as_ref(), [Op::Discard { .. }, Op::ScopeEnter { scope }, Op::Pattern { operation: PatternOperation::Preview { .. } }, Op::SetHint { hint: HintRecipe::Primitive(PrimitiveType::Bool), .. }] if *scope == index.nodes()[arm_id.0 as usize].scope)
        );
        let guard_false = prepare(
            &parsed,
            &body,
            &index,
            &index.pc(arm_id, Phase::Exit).unwrap(),
            arms[0].syntax().clone(),
        )
        .unwrap();
        assert!(
            matches!(guard_false.recipe().operations.as_ref(), [Op::Discard { .. }, Op::ScopeLeave { scope }] if *scope == index.nodes()[arm_id.0 as usize].scope)
        );
        let pattern = arms[1].pat().unwrap();
        let id = super::super::id_for(&index, &body, pattern.syntax()).unwrap();
        let next = index.nodes()[id.0 as usize].parent.unwrap();
        assert_eq!(index.nodes()[next.0 as usize].role, NodeRole::MatchArm);
        let match_id = super::super::id_for(&index, &body, matched.syntax()).unwrap();
        assert_eq!(
            guard_false.recipe().exit,
            RecipeExit::PatternRetry { owner: match_id }
        );
        let first = range_recipe(&parsed, &body, &index, &range, Phase::Entry);
        assert!(matches!(first.recipe().operations.first(),
            Some(Op::Pattern { operation: PatternOperation::Begin { retry, .. } })
                if *retry == index.pc(next, Phase::Entry).unwrap()));
        let unproven = prepare(
            &parsed,
            &body,
            &index,
            &index.pc(id, Phase::Entry).unwrap(),
            pattern.syntax().clone(),
        )
        .unwrap();
        assert!(
            unproven
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::Unsupported(_)))
        );
        assert!(
            !unproven
                .recipe()
                .operations
                .iter()
                .any(|op| matches!(op, Op::ReadPlace { .. } | Op::BindLocal { .. }))
        );
    }
}

#[test]
fn nonliteral_and_wide_float_ranges_remain_typed_source_frontiers() {
    for edition in [2015, 2018, 2021, 2024] {
        for (pattern, reached_phase) in [
            ("LOW..=4", Phase::Entry),
            ("0..=HIGH", Phase::Operand(0)),
            ("0.0f128..=4.0f128", Phase::Entry),
        ] {
            let source = format!(
                "const LOW:u8=0;const HIGH:u8=4;fn chosen(c:u8){{match c{{{pattern}=>(),_=>()}}}}"
            );
            let (parsed, body, index) = entered(&source, edition, "chosen");
            let range = parsed
                .entered_body_root(&body)
                .unwrap()
                .descendants()
                .find_map(ast::RangePat::cast)
                .unwrap();
            if reached_phase == Phase::Operand(0) {
                let lower = range_recipe(&parsed, &body, &index, &range, Phase::Entry);
                assert!(lower.recipe().operations.iter().any(|op| matches!(
                    op,
                    Op::NumericBinary {
                        operator: NumericBinary::Ge,
                        ..
                    }
                )));
                // A lower mismatch exits without demanding or admitting upper
                // syntax; its continuation must remain preparable as well.
                let exit = range_recipe(&parsed, &body, &index, &range, Phase::Exit);
                assert!(!exit.recipe().operations.iter().any(|op| matches!(
                    op,
                    Op::Literal { .. } | Op::NumericBinary { .. } | Op::Unsupported(_)
                )));
            }
            let refused = range_recipe(&parsed, &body, &index, &range, reached_phase);
            assert!(
                refused.recipe().operations.iter().any(
                    |op| matches!(op, Op::Unsupported(error) if error.message.contains("range"))
                )
            );
            assert!(!refused.recipe().operations.iter().any(|op| matches!(
                op,
                Op::Literal { .. } | Op::NumericBinary { .. } | Op::BindLocal { .. }
            )));
        }
    }
}

#[test]
fn floating_range_source_family_sign_and_suffix_are_exact_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (source, ty, signed, suffix) in [
            (
                "fn chosen(){match -1.5f64{-3.6..=3.6=>(),_=>()}}",
                PrimitiveType::F64,
                "-3.6",
                None,
            ),
            (
                "fn chosen(){match 0.0{0.0..3.5=>(),_=>()}}",
                PrimitiveType::F64,
                "0.0",
                None,
            ),
            (
                "fn chosen(){match 0.0{0.0..=1.0f32=>(),_=>()}}",
                PrimitiveType::F32,
                "0.0",
                None,
            ),
            (
                "fn chosen(){match 0f32{-1f32..=2f32=>(),_=>()}}",
                PrimitiveType::F32,
                "-1",
                Some(PrimitiveType::F32),
            ),
        ] {
            let (parsed, body, index) = entered(source, edition, "chosen");
            let range = parsed
                .entered_body_root(&body)
                .unwrap()
                .descendants()
                .find_map(ast::RangePat::cast)
                .unwrap();
            let recipe = range_recipe(&parsed, &body, &index, &range, Phase::Entry);
            let Some(Op::Literal {
                literal,
                hint: HintRecipe::Primitive(actual),
                ..
            }) = recipe
                .recipe()
                .operations
                .iter()
                .find(|op| matches!(op, Op::Literal { .. }))
            else {
                panic!("source floating range missing actual literal/hint")
            };
            assert_eq!(*actual, ty);
            assert_eq!(literal.kind, LiteralKind::Float);
            assert!(
                matches!(&literal.decoded, Some(DecodedLiteral::Float { decimal, suffix: actual }) if decimal == signed && *actual == suffix)
            );
            assert_eq!(
                &source[(body.range.start + literal.source.relative_start) as usize
                    ..(body.range.start + literal.source.relative_end) as usize],
                literal.spelling
            );
            assert_eq!(literal.source.body_key, body.key());
            let upper = range_recipe(&parsed, &body, &index, &range, Phase::Operand(0));
            assert!(
                matches!(&upper.recipe().operations[1], Op::Literal { hint: HintRecipe::Primitive(actual),.. } if *actual == ty)
            );
        }
        // An integer token never becomes a float merely from an opposite-family
        // expected range suffix. The VM keeps its actual incompatible-source
        // refusal; this source control does not forge successful execution.
        let source = "fn chosen(){match 1{0.0f32..=2.0f32=>(),_=>()}}";
        let (parsed, body, index) = entered(source, edition, "chosen");
        let matched = parsed
            .entered_body_root(&body)
            .unwrap()
            .descendants()
            .find_map(ast::MatchExpr::cast)
            .unwrap();
        let id = super::super::id_for(&index, &body, matched.syntax()).unwrap();
        let recipe = parsed
            .initial_thunk(&body, &index, &index.pc(id, Phase::Entry).unwrap())
            .unwrap();
        assert!(recipe.recipe().operations.iter().any(|op| matches!(
            op,
            Op::SetHint {
                hint: HintRecipe::Primitive(PrimitiveType::I32),
                ..
            }
        )));
    }
}
