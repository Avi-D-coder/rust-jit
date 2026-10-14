//! LRR-specific exhaustion/exit regressions over unchanged upstream syntax.
//! Forcing a typed exit at an indexed source node is not full fixture execution.
use lrr::{
    core::{
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    ir::{
        op::{DiagnosticTemplate, LRIR_RECIPE_SCHEMA, Op},
        pc::{NodeId, Phase},
        recipe::{Origin, Recipe, RecipeDraft, RecipeExit},
        validate::verify,
    },
    runtime::cell::GuestStore,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole, ScopeId},
        parse::ParsedFile,
    },
    vm::{
        frame::{Frame, FrameStatus},
        step::step,
    },
};

const ORIGINAL: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/binding/expr-match.rs");

fn body(edition: u16) -> BodyIndex {
    let blob = SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let parsed = ParsedFile::parse(&blob, profile).unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"LRR typed match exit fixture"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let item = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "test_basic")
        .unwrap();
    parsed
        .index_body(item.body.as_ref().unwrap(), Default::default())
        .unwrap()
}

fn draft(body: &BodyIndex) -> RecipeDraft {
    let node = body
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Expression(ExpressionKind::Match))
        .unwrap();
    RecipeDraft {
        key: body.pc(node.id, Phase::Exit).unwrap(),
        origin: Origin {
            span: body.span(node.id).unwrap(),
            expansion_ancestry: vec![],
        },
        operations: vec![],
        exits: vec![RecipeExit::MatchNoArm],
        dependencies: vec![],
    }
}

#[test]
fn match_no_arm_is_exact_source_indexed_zero_op_exit() {
    for edition in [2015, 2018, 2021, 2024] {
        let body = body(edition);
        let draft = draft(&body);
        let expected_pc = draft.key.clone();
        let expected_origin = draft.origin.clone();
        let verified = verify(draft, &body).unwrap();
        assert_eq!(verified.recipe().key, expected_pc);
        assert_eq!(verified.recipe().origin, expected_origin);
        assert!(verified.recipe().operations.is_empty());
        assert_eq!(verified.recipe().exit, RecipeExit::MatchNoArm);
        assert!(verified.recipe().exit.requires_dispatch_boundary());
    }
}

#[test]
fn match_no_arm_verifier_rejects_wrong_owner_phase_origin_and_operations() {
    let body = body(2024);
    let initial = draft(&body);
    let mut bad = vec![];
    let mut wrong = initial.clone();
    wrong.key = body.pc(NodeId(0), Phase::Exit).unwrap();
    wrong.origin.span = body.span(NodeId(0)).unwrap();
    bad.push(wrong);
    let mut wrong = initial.clone();
    wrong.key.phase = Phase::Entry.code().unwrap();
    bad.push(wrong);
    let mut wrong = initial.clone();
    wrong.origin.span.relative_end -= 1;
    bad.push(wrong);
    let mut wrong = initial.clone();
    wrong.operations.push(Op::ScopeEnter { scope: ScopeId(0) });
    bad.push(wrong);
    let mut wrong = initial.clone();
    wrong.origin.expansion_ancestry.push(Digest([7; 32]));
    bad.push(wrong);
    let mut wrong = initial;
    wrong.exits.push(RecipeExit::MatchNoArm);
    bad.push(wrong);
    for wrong in bad {
        assert_eq!(verify(wrong, &body).unwrap_err().code, "LRR-E-INTERNAL-IR");
    }
}

#[test]
fn match_no_arm_charges_one_exit_and_preserves_storage() {
    for edition in [2015, 2018, 2021, 2024] {
        let body = body(edition);
        let verified = verify(draft(&body), &body).unwrap();
        let mut store = GuestStore::new(1, 100, 1024 * 1024).unwrap();
        let mut frame = Frame::new(body, verified.recipe().key.clone(), &mut store).unwrap();
        frame.set_fuel_limit(1).unwrap();
        let before = (
            store.budget().epoch(),
            store.budget().handles(),
            store.budget().bytes(),
        );
        let error = step(&mut frame, &mut store, &verified).unwrap_err();
        assert_eq!(error.code, "LRR-E-MATCH-NO-ARM");
        assert_eq!(frame.status(), FrameStatus::Faulted);
        assert_eq!(frame.pc(), &verified.recipe().key);
        assert_eq!(frame.fuel_snapshot().used, 1);
        assert_eq!(frame.counters().fuel_used, 1);
        assert_eq!(frame.counters().operations_started, 0);
        assert_eq!(frame.counters().operations_completed, 0);
        assert_eq!(frame.counters().exits_completed, 0);
        assert_eq!(frame.counters().cleanup_elements_completed, 0);
        assert!(frame.active_scopes().is_empty());
        assert_eq!(
            (
                store.budget().epoch(),
                store.budget().handles(),
                store.budget().bytes()
            ),
            before
        );
        // A terminal fault must not re-charge or retry the terminal recipe.
        assert_eq!(
            step(&mut frame, &mut store, &verified).unwrap_err().code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(frame.fuel_snapshot().used, 1);
        assert_eq!(
            (
                store.budget().epoch(),
                store.budget().handles(),
                store.budget().bytes()
            ),
            before
        );
    }
}

#[test]
fn match_no_arm_low_fuel_precedes_fault_without_effects() {
    let body = body(2024);
    let verified = verify(draft(&body), &body).unwrap();
    let mut store = GuestStore::new(1, 100, 1024 * 1024).unwrap();
    let mut frame = Frame::new(body, verified.recipe().key.clone(), &mut store).unwrap();
    frame.set_fuel_limit(0).unwrap();
    let before = (store.budget().handles(), store.budget().bytes());
    assert_eq!(
        step(&mut frame, &mut store, &verified).unwrap_err().code,
        "LRR-E-RUNTIME-RESOURCE"
    );
    assert_eq!(
        frame.fuel_snapshot().stop,
        Some(lrr::vm::fuel::FuelStop::Exhausted)
    );
    assert_eq!(frame.fuel_snapshot().used, 0);
    assert_eq!(frame.counters().exits_completed, 0);
    assert_eq!((store.budget().handles(), store.budget().bytes()), before);
}

#[test]
fn match_no_arm_is_stable_typed_encoding_distinct_from_unsupported() {
    let body = body(2024);
    let mut draft = draft(&body);
    let verified = verify(draft.clone(), &body).unwrap();
    let recipe = verified.recipe();
    let mut expected = StableEncoder::new(Recipe::DOMAIN);
    expected.record(&recipe.key);
    expected.record(&recipe.origin.span.body_key);
    expected.u32(recipe.origin.span.relative_start);
    expected.u32(recipe.origin.span.relative_end);
    expected.u64(0); // expansion ancestors
    expected.u64(0); // operations
    expected.u32(7); // fixed wire discriminant, not Rust enum layout
    expected.u64(0); // dependencies
    assert_eq!(recipe.fingerprint(), expected.finish());
    const { assert!(LRIR_RECIPE_SCHEMA == 6) };
    assert_ne!(Recipe::DOMAIN, "lrr/v4/lrir-recipe");
    assert_ne!(Recipe::DOMAIN, "lrr/v5/lrir-recipe");
    draft.exits = vec![RecipeExit::Fault(DiagnosticTemplate {
        message: "not a match exhaustion diagnostic".into(),
    })];
    let unsupported = verify(draft, &body).unwrap();
    assert_ne!(unsupported.recipe().fingerprint(), recipe.fingerprint());
    let mut store = GuestStore::new(1, 100, 1024 * 1024).unwrap();
    let mut frame = Frame::new(body, unsupported.recipe().key.clone(), &mut store).unwrap();
    assert_eq!(
        step(&mut frame, &mut store, &unsupported).unwrap_err().code,
        "LRR-E-UNSUPPORTED"
    );
}
