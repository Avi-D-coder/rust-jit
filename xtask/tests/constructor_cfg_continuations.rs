//! M017 pure-query constructor continuation acceptance on unchanged rustc cfg source.
//! No guest execution or full stock main pass is claimed by these recipe checks.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::{Database, Outcome},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::{
        op::Op,
        pc::{PcKey, Phase},
        recipe::{RecipeExit, VerifiedRecipe},
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{constructors::ConstructorKind, parse::ParseKey},
};
const STOCK: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/cfg/cfg_stmt_expr.rs");
fn fetch(db: &mut Database, key: QueryKey) -> Outcome {
    match db.fetch(&SyntaxProvider, key) {
        QueryPoll::Ready(Ok(outcome)) => outcome,
        other => panic!("original source query must complete: {other:?}"),
    }
}
fn thunk(db: &mut Database, pc: PcKey) -> VerifiedRecipe {
    let outcome = fetch(db, QueryKey::Thunk(pc));
    let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
        panic!("expected thunk")
    };
    recipe.as_ref().clone()
}
fn next(recipe: &VerifiedRecipe) -> &PcKey {
    let RecipeExit::Next(next) = &recipe.recipe().exit else {
        panic!("expected structural continuation")
    };
    next
}
#[test]
fn unchanged_cfg_constructors_translate_sparse_return_ordinals_to_dense_fields() {
    for edition in [2015, 2018, 2021, 2024] {
        let input = SourceUnitInput {
            path: SourcePath::new("src/main.rs").unwrap(),
            crate_key: Digest([81; 32]),
            module_path: vec![],
            profile: ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            },
            cfg: CfgSet::guest_default(),
            blob: SourceBlob::new(STOCK.as_bytes().to_vec()).unwrap(),
        };
        let selection = SourceSelection {
            path: input.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: input.blob.content(),
                    profile: input.profile.clone(),
                },
                crate_key: input.crate_key,
                module_path: vec![],
                cfg: input.cfg.fingerprint(),
            },
        };
        let prepared =
            PreparedSourceInputs::refresh(None, vec![input], Default::default()).unwrap();
        let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
            panic!("source absent")
        };
        let body = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let mut db = Database::default();
        db.trace.detailed = true;
        prepared.publish(&mut db).unwrap();
        let structure = fetch(&mut db, QueryKey::BodyStructure(body.key()));
        let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
            panic!("body absent")
        };
        for (needle, expected, expected_raw) in [
            ("(#[cfg(false)] 341,)", 0, &[][..]),
            ("(1, #[cfg(false)] 3, 4)", 2, &[0usize, 2][..]),
            ("[#[cfg(false)] 126]", 0, &[][..]),
            ("[#[cfg(false)] 1, 2, 6]", 2, &[1usize, 2][..]),
        ] {
            let node = index
                .nodes()
                .iter()
                .find(|node| {
                    STOCK.get(
                        (body.range.start + node.range.start) as usize
                            ..(body.range.start + node.range.end) as usize,
                    ) == Some(needle)
                })
                .unwrap();
            let entry = index.pc(node.id, Phase::Entry).unwrap();
            let planned = fetch(&mut db, QueryKey::ConstructorPlan(entry.clone()));
            let QueryValue::ConstructorPlan(plan) = planned.value.as_ref() else {
                panic!("plan absent")
            };
            let fields = match plan.kind() {
                ConstructorKind::Tuple(fields) | ConstructorKind::Array(fields) => fields,
                _ => panic!("unexpected constructor"),
            };
            assert_eq!(fields.len(), expected);
            assert_eq!(
                node.operands.len(),
                expected + 1,
                "disabled operand identity must remain in the sparse structural list"
            );
            assert_eq!(
                fields
                    .iter()
                    .map(|field| node
                        .operands
                        .iter()
                        .position(|operand| *operand == field.node)
                        .unwrap())
                    .collect::<Vec<_>>(),
                expected_raw
            );
            let recipe = thunk(&mut db, entry);
            assert!(
                recipe
                    .recipe()
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::ConstructStart { .. }))
            );
            let commit = index.pc(node.id, Phase::Commit).unwrap();
            assert_eq!(
                next(&recipe),
                &fields.first().map_or_else(
                    || commit.clone(),
                    |field| index.pc(field.node, Phase::Entry).unwrap()
                )
            );
            for (written, field) in fields.iter().enumerate() {
                let raw = node
                    .operands
                    .iter()
                    .position(|operand| *operand == field.node)
                    .unwrap();
                let returned = index
                    .pc(node.id, Phase::Operand(u16::try_from(raw).unwrap()))
                    .unwrap();
                let child = thunk(&mut db, index.pc(field.node, Phase::Entry).unwrap());
                assert_eq!(
                    next(&child),
                    &returned,
                    "original child must return through sparse ordinal {raw}"
                );
                let continuation = thunk(&mut db, returned);
                assert!(continuation.recipe().operations.iter().any(|op| matches!(op,
                    Op::ConstructField { field: ordinal, source, .. } if *ordinal == u32::try_from(written).unwrap() && *source == field.value
                )), "constructor must write dense field {written} from sparse operand {raw}");
                assert_eq!(
                    next(&continuation),
                    &fields.get(written + 1).map_or_else(
                        || commit.clone(),
                        |next| index.pc(next.node, Phase::Entry).unwrap()
                    )
                );
            }
            for (raw, child) in node
                .operands
                .iter()
                .enumerate()
                .filter(|(_, child)| !fields.iter().any(|field| field.node == **child))
            {
                let entry = index.pc(*child, Phase::Entry).unwrap();
                assert!(
                    db.memo(&QueryKey::Thunk(entry.clone())).is_none(),
                    "disabled original child must stay cold"
                );
                let disabled = index
                    .pc(node.id, Phase::Operand(u16::try_from(raw).unwrap()))
                    .unwrap();
                let QueryPoll::Ready(Err(error)) =
                    db.fetch(&SyntaxProvider, QueryKey::Thunk(disabled))
                else {
                    panic!("disabled child must have no valid written-field return")
                };
                assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
                assert_eq!(
                    error.message,
                    "constructor phase is not an enabled written field"
                );
                assert!(db.memo(&QueryKey::Thunk(entry)).is_none());
            }
            let finish = thunk(&mut db, commit);
            assert!(
                finish
                    .recipe()
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::ConstructFinish { .. }))
            );
            assert_eq!(finish.recipe().origin.span, index.span(node.id).unwrap());
        }
    }
}
