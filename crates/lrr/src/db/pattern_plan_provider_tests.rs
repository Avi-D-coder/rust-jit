//! Query/route controls outside unchanged official originals. Pending execution.
use super::*;
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::{OutlineKey, QueryKey},
        memo::Database,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::{
        op::{Op, PatternOperation},
        pc::Phase,
        recipe::VerifiedRecipe,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        literal_decode::observe_integer_decoding,
        parse::ParseKey,
        pattern_plan::PatternSourcePlan,
    },
};
use ra_ap_syntax::{AstNode, ast};

const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/binding/match-range-infer.rs"
);

fn published(
    text: &str,
    edition: u16,
) -> (
    Database,
    BodyHandle,
    Arc<ParsedFile>,
    Arc<BodyIndex>,
    std::collections::BTreeMap<InputKey, InputValue>,
) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("retained-pattern-plan.rs").unwrap();
    let crate_key = Digest::of_bytes(b"retained common pattern query control");
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source selection absent");
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .clone()
        .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    let parsed = db
        .fetch(
            &SyntaxProvider,
            QueryKey::ParseFile(selection.outline.parse),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        panic!("parser query variant");
    };
    let parsed = Arc::clone(parsed);
    let index = db
        .fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key()))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = index.value.as_ref() else {
        panic!("index query variant");
    };
    (
        db,
        body,
        parsed,
        Arc::clone(index),
        view.prepared_inputs().clone(),
    )
}

fn plan(db: &mut Database, owner: &PcKey) -> Arc<PatternSourcePlan> {
    let result = db
        .fetch(&SyntaxProvider, QueryKey::PatternPlan(owner.clone()))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::PatternPlan(plan) = result.value.as_ref() else {
        panic!("pattern query variant");
    };
    Arc::clone(plan)
}
fn thunk(db: &mut Database, key: &PcKey) -> VerifiedRecipe {
    let result = db
        .fetch(&SyntaxProvider, QueryKey::Thunk(key.clone()))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::Thunk(recipe) = result.value.as_ref() else {
        panic!("ordinary thunk query variant: {:?}", result.value);
    };
    recipe.as_ref().clone()
}

#[test]
fn unchanged_range_owners_reuse_query_plan_and_decode_only_selected_endpoint_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, body, parsed, index, _) = published(ORIGINAL, edition);
        let owners: Vec<_> = index
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Match))
            .map(|node| index.pc(node.id, Phase::Entry).unwrap())
            .collect();
        assert_eq!(owners.len(), 3);
        for owner in &owners {
            let (retained, preparation_decodes) = observe_integer_decoding(|| plan(&mut db, owner));
            assert_eq!(preparation_decodes.attempts, 0);
            assert!(Arc::ptr_eq(retained.parsed(), &parsed));
            assert!(Arc::ptr_eq(retained.index(), &index));
            assert!(retained.metadata_bytes() < 32 * 1024 * 1024);
            let range = index
                .nodes()
                .iter()
                .find(|node| {
                    node.scalar_range_phase
                        && PatternSourcePlan::owner_for(
                            &index,
                            &index.pc(node.id, Phase::Entry).unwrap(),
                        )
                        .unwrap()
                        .as_ref()
                            == Some(owner)
                })
                .unwrap();
            let lower = index.pc(range.id, Phase::Entry).unwrap();
            let upper = index.pc(range.id, Phase::Operand(0)).unwrap();
            let (recipe, decodes) = observe_integer_decoding(|| thunk(&mut db, &lower));
            assert_eq!(decodes.attempts, 1);
            assert!(recipe.recipe().operations.iter().any(|operation| matches!(
                operation,
                Op::Pattern {
                    operation: PatternOperation::Begin { .. }
                }
            )));
            let (_, decodes) = observe_integer_decoding(|| thunk(&mut db, &upper));
            assert_eq!(decodes.attempts, 1);
            let (_, decodes) = observe_integer_decoding(|| {
                thunk(&mut db, &index.pc(range.id, Phase::Exit).unwrap())
            });
            assert_eq!(decodes.attempts, 0);
            assert!(Arc::ptr_eq(&retained, &plan(&mut db, owner)));
            assert_eq!(retained.owner().body, body.key());
        }
        assert_eq!(db.trace.counts.get("PatternPlan:ProviderRun"), Some(&3));
    }
}

#[test]
fn condition_range_uses_same_retained_source_plan_and_owner_body_authority_all_editions() {
    // Ordinary route adaptation of an unchanged official pattern. The original
    // remains byte-for-byte unchanged and this is not its full-fixture receipt.
    let original = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let range = original
        .parsed
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::RangePat::cast)
        .unwrap();
    let source = format!("fn main(){{if let {} = 1 {{}}}}", range.syntax().text());
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, _, _, index, _) = published(&source, edition);
        let owner_node = index
            .nodes()
            .iter()
            .find(|node| node.clause_scope.is_some())
            .unwrap();
        let owner = index.pc(owner_node.id, Phase::Entry).unwrap();
        let (retained, decodes) = observe_integer_decoding(|| plan(&mut db, &owner));
        // The reached source hint may decode the scrutinee literal once; neither
        // pattern endpoint is decoded by structural preparation.
        assert_eq!(decodes.attempts, 1);
        let source_node = owner_node.operands[0];
        assert!(
            PatternSourcePlan::owner_for(&index, &index.pc(source_node, Phase::Entry).unwrap())
                .unwrap()
                .is_none()
        );
        let phase = owner_node
            .operands
            .iter()
            .position(|id| *id == source_node)
            .unwrap();
        let (_, decodes) = observe_integer_decoding(|| {
            thunk(
                &mut db,
                &index
                    .pc(owner_node.id, Phase::Operand(u16::try_from(phase).unwrap()))
                    .unwrap(),
            )
        });
        assert_eq!(decodes.attempts, 1);
        let (_, decodes) = observe_integer_decoding(|| {
            thunk(&mut db, &index.pc(owner_node.id, Phase::Commit).unwrap())
        });
        assert_eq!(decodes.attempts, 1);
        assert!(Arc::ptr_eq(&retained, &plan(&mut db, &owner)));
        assert_eq!(db.trace.counts.get("PatternPlan:ProviderRun"), Some(&1));
        let mut wrong_owner = owner.clone();
        wrong_owner.phase = Phase::Commit.code().unwrap();
        assert!(
            db.fetch(&SyntaxProvider, QueryKey::PatternPlan(wrong_owner))
                .into_ready()
                .unwrap()
                .is_err()
        );
        let mut wrong_body = owner.clone();
        wrong_body.body.definition = Digest([71; 32]);
        assert!(retained.lower(&wrong_body, None).is_err());
    }
}

#[test]
fn retained_pattern_replays_relevant_dependencies_and_accepts_equivalent_index_allocation() {
    let (mut db, body, parsed, index, mut inputs) = published(ORIGINAL, 2024);
    let owner_node = index
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Expression(ExpressionKind::Match))
        .unwrap();
    let owner = index.pc(owner_node.id, Phase::Entry).unwrap();
    let retained = plan(&mut db, &owner);
    let before = db.cache_usage().unwrap();
    assert!(before.memo_bytes >= retained.metadata_bytes());
    assert!(before.pinned_memo_bytes >= retained.metadata_bytes());
    // Unrelated publication changes the database revision, not the actual
    // source/owner dependencies of this immutable pattern plan.
    inputs.insert(
        InputKey::CompileEnv("LRR_UNRELATED_PATTERN_CONTROL".into()),
        InputValue::CompileEnv("value".into()),
    );
    db.publish_inputs(inputs.clone()).unwrap();
    assert!(Arc::ptr_eq(&retained, &plan(&mut db, &owner)));
    assert_eq!(db.trace.counts.get("PatternPlan:ProviderRun"), Some(&1));
    // An equivalent immutable index may be reallocated after memo eviction.
    // Allocation identity is never source authority, and lower() keeps using
    // the dependency-validated plan's original shared root.
    let equivalent_index = Arc::new(parsed.index_body(&body, Default::default()).unwrap());
    assert!(!Arc::ptr_eq(&equivalent_index, &index));
    assert_eq!(equivalent_index.fingerprint(), index.fingerprint());
    let provided = super::thunk(
        &SyntaxProvider,
        &mut db,
        &parsed,
        &body,
        &equivalent_index,
        &owner,
    )
    .into_ready()
    .unwrap()
    .unwrap()
    .unwrap();
    assert!(matches!(provided.value, QueryValue::Thunk(_)));
    assert!(Arc::ptr_eq(retained.index(), &index));
    // A changed actual owning body invalidates the old query. This deliberately
    // inconsistent source batch is a negative currentness control, never a
    // successful Rust runtime-contract witness.
    let mut changed_body = body.clone();
    changed_body.body_hash = Digest([77; 32]);
    inputs.insert(
        InputKey::DefinitionBody(body.def_key),
        InputValue::DefinitionBody(changed_body),
    );
    db.publish_inputs(inputs).unwrap();
    assert!(
        db.fetch(&SyntaxProvider, QueryKey::PatternPlan(owner))
            .into_ready()
            .unwrap()
            .is_err()
    );
}
