//! Selected unchanged typed borrows; no raw pointer/unsizing/full fixture claim.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{
        op::{HintRecipe, Op},
        pc::Phase,
        recipe::{RecipeDraft, VerifiedRecipe},
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{outline::BodyHandle, parse::ParsedFile, scope_fixture_facts},
    vm::step::{StepOutcome, step},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/cast/coercion-as-explicit-cast.rs"
);
const UNTYPED: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/methods/call_method_unknown_referent.rs"
);
struct Context {
    parsed: ParsedFile,
    handle: BodyHandle,
    frame: Frame,
    store: GuestStore,
    binding: NodeId,
    borrow: NodeId,
    operand: NodeId,
    expected: u64,
}
fn context(mutable: bool) -> Context {
    let facts = scope_fixture_facts::original_reference_contexts(ORIGINAL);
    assert_eq!(facts.len(), 2);
    let facts = facts.iter().find(|fact| fact.mutable == mutable).unwrap();
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-borrow-operand-context"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .clone()
        .unwrap();
    let body = parsed.index_body(&handle, Default::default()).unwrap();
    let id = |range| {
        body.nodes()
            .iter()
            .find(|node| node.range == range)
            .unwrap()
            .id
    };
    let statement = id(facts.statement);
    let (binding, borrow, operand) = (id(facts.binding), id(facts.borrow), id(facts.operand));
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(
        body.clone(),
        body.pc(statement, Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    let mut scopes = vec![];
    let mut scope = Some(body.nodes()[statement.0 as usize].scope);
    while let Some(id) = scope {
        scopes.push(id);
        scope = body.scopes()[id.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    let recipe = parsed.initial_thunk(&handle, &body, frame.pc()).unwrap();
    assert!(matches!(
        step(&mut frame, &mut store, &recipe).unwrap(),
        StepOutcome::Next(_)
    ));
    assert_eq!(frame.pc(), &body.pc(borrow, Phase::Entry).unwrap());
    Context {
        parsed,
        handle,
        frame,
        store,
        binding,
        borrow,
        operand,
        expected: facts.value,
    }
}
fn recipe(c: &Context) -> VerifiedRecipe {
    c.parsed
        .initial_thunk(&c.handle, &c.frame.body, c.frame.pc())
        .unwrap()
}
// The original statement recipe can suspend its bounded ScopeLeave. A
// Dispatch is retained progress, never completion or a swallowed guest error.
fn finish_recipe(c: &mut Context, r: &VerifiedRecipe) {
    let pc = c.frame.pc.clone();
    let before = c.frame.counters.started_by_opcode[5];
    let scope_entries = r
        .recipe()
        .operations
        .iter()
        .filter(|op| matches!(op, Op::ScopeLeave { .. }))
        .count() as u64;
    let mut last: Option<(usize, u64, u64)> = None;
    for _ in 0..4096 {
        match step(&mut c.frame, &mut c.store, r).unwrap() {
            StepOutcome::Dispatch => {
                assert_eq!(c.frame.pc, pc);
                assert!(c.frame.has_cleanup_activation());
                assert!(!c.frame.has_copy_activation());
                let index = c.frame.resume_operation_index();
                assert!(matches!(
                    r.recipe().operations.get(index),
                    Some(Op::ScopeLeave { .. })
                ));
                let started = c.frame.counters.operations_started;
                let scope_started = c.frame.counters.started_by_opcode[5];
                if let Some((previous, previous_started, previous_scope_started)) = last {
                    assert!(index >= previous, "cleanup replayed the recipe prefix");
                    if index == previous {
                        assert_eq!(started, previous_started);
                        assert_eq!(scope_started, previous_scope_started);
                    }
                }
                last = Some((index, started, scope_started));
            }
            StepOutcome::Next(next) => {
                let crate::ir::recipe::RecipeExit::Next(expected) = &r.recipe().exit else {
                    panic!("original typed-borrow recipe lost its Next exit")
                };
                assert_eq!(&next, expected);
                assert_eq!(c.frame.pc(), expected);
                assert!(!c.frame.has_cleanup_activation());
                assert_eq!(
                    c.frame.counters.started_by_opcode[5],
                    before + scope_entries
                );
                return;
            }
            other => panic!("unexpected typed-borrow recipe outcome {other:?}"),
        }
    }
    panic!("original typed-borrow recipe exceeded bounded cleanup progress");
}
fn complete(c: &mut Context) {
    let parent = c.frame.body.nodes()[c.borrow.0 as usize].value.unwrap();
    let child = c.frame.body.nodes()[c.operand.0 as usize].value.unwrap();
    let initial = recipe(c);
    assert!(
        matches!(initial.recipe().operations.as_ref(), [Op::SetHint { child: actual, hint: HintRecipe::BorrowTarget { reference } }] if *actual == child && *reference == parent)
    );
    assert!(matches!(
        step(&mut c.frame, &mut c.store, &initial).unwrap(),
        StepOutcome::Next(_)
    ));
    // The context is present before the actual operand runs. Scalar write
    // consumes this contextual source view; the reference result separately
    // retains its written source annotation through binding.
    assert_eq!(
        c.frame.hint(child).unwrap(),
        Some(crate::runtime::shape::TypeId(11))
    );
    assert_eq!(
        c.frame.source_view(child).unwrap(),
        Some(crate::traits::term::TyTerm::Primitive(
            crate::ir::op::PrimitiveType::U32
        ))
    );
    assert!(c.frame.cell(child).is_err());
    for _ in 0..16 {
        let r = recipe(c);
        finish_recipe(c, &r);
        if c.frame.binding(c.binding).is_ok() {
            return;
        }
    }
    panic!("original typed borrow statement did not finish");
}
fn check(mutable: bool) {
    let mut c = context(mutable);
    complete(&mut c);
    let node = &c.frame.body.nodes()[c.operand.0 as usize];
    let scope = node.extended_to.as_ref().unwrap().scope;
    let target = c.frame.cell(node.value.unwrap()).unwrap();
    assert_eq!(c.frame.slots[node.value.unwrap().0 as usize].scope, scope);
    assert!(matches!(&c.frame.binding_sources[c.binding.0 as usize],
            Some(crate::traits::term::TyTerm::Ref(_, access, target))
            if *access == mutable && target.as_ref() == &crate::traits::term::TyTerm::Primitive(crate::ir::op::PrimitiveType::U32)));
    let reference = c
        .store
        .reference_from_cell(c.frame.binding(c.binding).unwrap())
        .unwrap();
    let value = c.store.read_reference(reference).unwrap();
    assert_eq!(value.type_id, 11);
    assert_eq!(value.payload, c.expected);
    assert_eq!(c.frame.counters.started_by_opcode[0], 1);
    assert_eq!(c.frame.counters.started_by_opcode[31], 1);
    assert_eq!(c.store.read_scalar(target).unwrap(), value);
    c.frame.leave(scope, &mut c.store).unwrap();
    assert_eq!(c.store.state(target).unwrap(), CellState::Dropped);
    assert!(c.store.read_reference(reference).is_err());
}
#[test]
fn rustc_explicit_shared_borrow_context_materializes_original_unsuffixed_operand() {
    check(false);
}
#[test]
fn rustc_explicit_mutable_borrow_context_materializes_original_unsuffixed_operand() {
    check(true);
}
#[test]
fn rustc_borrow_target_hint_verifier_rejects_foreign_parent_child_and_annotation_use() {
    let c = context(false);
    let r = recipe(&c);
    let child = c.frame.body.nodes()[c.operand.0 as usize].value.unwrap();
    let parent = c.frame.body.nodes()[c.borrow.0 as usize].value.unwrap();
    let foreign = c
        .frame
        .body
        .nodes()
        .iter()
        .filter_map(|node| node.value)
        .find(|value| *value != child && *value != parent)
        .unwrap();
    for operation in [
        Op::SetHint {
            child,
            hint: HintRecipe::BorrowTarget { reference: foreign },
        },
        Op::SetHint {
            child: foreign,
            hint: HintRecipe::BorrowTarget { reference: parent },
        },
        Op::BindLocal {
            binding: c.binding,
            source: child,
            hint: HintRecipe::BorrowTarget { reference: parent },
            writable: false,
        },
    ] {
        let old = r.recipe();
        let draft = RecipeDraft {
            key: old.key.clone(),
            origin: old.origin.clone(),
            operations: vec![operation],
            exits: vec![old.exit.clone()],
            dependencies: old.dependencies.to_vec(),
        };
        assert_eq!(
            crate::ir::validate::verify(draft, &c.frame.body)
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-IR"
        );
    }
}
#[test]
fn rustc_borrow_target_hint_without_context_preserves_original_suffixed_type() {
    // The original first let in a has &1u32 and no annotation; a missing context
    // must propagate None, not manufacture a numeric width or reference source.
    let facts = scope_fixture_facts::original_temporary_borrow(UNTYPED, "a");
    let parsed = ParsedFile::parse(
        &SourceBlob::new(UNTYPED.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-suffixed-borrow-context"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let handle = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "a")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let body = parsed.index_body(handle, Default::default()).unwrap();
    let borrow = body
        .nodes()
        .iter()
        .find(|node| node.range == facts.borrow)
        .unwrap();
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(
        body.clone(),
        body.pc(borrow.id, Phase::Entry).unwrap(),
        &mut store,
    )
    .unwrap();
    let mut scopes = vec![];
    let mut scope = Some(borrow.scope);
    while let Some(id) = scope {
        scopes.push(id);
        scope = body.scopes()[id.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    let r = parsed.initial_thunk(handle, &body, frame.pc()).unwrap();
    assert!(matches!(
        r.recipe().operations.as_ref(),
        [Op::SetHint {
            hint: HintRecipe::BorrowTarget { .. },
            ..
        }]
    ));
    assert!(matches!(
        step(&mut frame, &mut store, &r).unwrap(),
        StepOutcome::Next(_)
    ));
    let child = body.nodes()[borrow.operands[0].0 as usize].value.unwrap();
    assert_eq!(frame.hint(child).unwrap(), None);
    assert_eq!(frame.source_view(child).unwrap(), None);
    let literal = parsed.initial_thunk(handle, &body, frame.pc()).unwrap();
    assert!(matches!(
        step(&mut frame, &mut store, &literal).unwrap(),
        StepOutcome::Next(_)
    ));
    assert_eq!(
        store
            .read_scalar(frame.cell(child).unwrap())
            .unwrap()
            .type_id,
        11
    );
}
