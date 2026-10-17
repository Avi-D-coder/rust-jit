//! Executes the original first let in `a`, not the stock method-error body.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{op::Op, pc::Phase, recipe::VerifiedRecipe},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{outline::BodyHandle, parse::ParsedFile, scope_fixture_facts},
    vm::step::{StepOutcome, step},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/methods/call_method_unknown_referent.rs"
);
struct Context {
    parsed: ParsedFile,
    handle: BodyHandle,
    frame: Frame,
    store: GuestStore,
    statement: NodeId,
    binding: NodeId,
    borrow: NodeId,
    operand: NodeId,
}
fn context() -> Context {
    let facts = scope_fixture_facts::original_temporary_borrow(ORIGINAL, "a");
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-temporary-borrow"),
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
    let (statement, binding, borrow, operand) = (
        id(facts.statement),
        id(facts.binding),
        id(facts.borrow),
        id(facts.operand),
    );
    let entry = body.pc(statement, Phase::Entry).unwrap();
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(body.clone(), entry, &mut store).unwrap();
    let mut scopes = vec![];
    let mut scope = Some(body.nodes()[statement.0 as usize].scope);
    while let Some(id) = scope {
        scopes.push(id);
        scope = body.scopes()[id.0 as usize].parent;
    }
    for scope in scopes.into_iter().rev() {
        frame.enter(scope).unwrap();
    }
    Context {
        parsed,
        handle,
        frame,
        store,
        statement,
        binding,
        borrow,
        operand,
    }
}
fn recipe(c: &Context) -> VerifiedRecipe {
    c.parsed
        .initial_thunk(&c.handle, &c.frame.body, c.frame.pc())
        .unwrap()
}
fn run(c: &mut Context, r: &VerifiedRecipe) -> Result<StepOutcome, Diagnostic> {
    step(&mut c.frame, &mut c.store, r)
}
fn reach_borrow(c: &mut Context) -> VerifiedRecipe {
    for _ in 0..16 {
        let r = recipe(c);
        if r.recipe()
            .operations
            .iter()
            .any(|op| matches!(op, Op::Borrow { .. }))
        {
            assert_eq!(c.frame.pc().node, c.borrow.0);
            assert_eq!(c.frame.pc().phase, Phase::Commit.code().unwrap());
            return r;
        }
        assert!(matches!(run(c, &r).unwrap(), StepOutcome::Next(_)));
    }
    panic!("original initializer did not reach its borrow")
}
#[test]
fn rustc_literal_temporary_borrow_is_promoted_once_and_survives_statement_and_block_cleanup() {
    let mut c = context();
    let r = reach_borrow(&mut c);
    // `&1u32` borrows a constant expression: rvalue static promotion
    // publishes the literal once as the site's run-long place, and the
    // borrow addresses that place instead of a frame temporary.
    assert_eq!(c.frame.counters.started_by_opcode[0], 1);
    assert_eq!(c.frame.counters.started_by_opcode[66], 1);
    assert_eq!(c.frame.counters.started_by_opcode[38], 0);
    assert_eq!(c.frame.counters.started_by_opcode[31], 0);
    let node = &c.frame.body.nodes()[c.operand.0 as usize];
    let extension = node.extended_to.as_ref().unwrap();
    assert!(extension.constructors.is_empty());
    let block = extension.scope;
    let statement_scope = c.frame.body.nodes()[c.statement.0 as usize]
        .statement_scope
        .unwrap();
    assert_ne!(block, statement_scope);
    assert!(matches!(run(&mut c, &r).unwrap(), StepOutcome::Next(_)));
    assert_eq!(c.frame.pc().node, c.statement.0);
    let r = recipe(&c);
    assert!(matches!(run(&mut c, &r).unwrap(), StepOutcome::Next(_)));
    assert!(!c.frame.active_scopes().contains(&statement_scope));
    let owner = c.frame.binding(c.binding).unwrap();
    let reference = c.store.reference_from_cell(owner).unwrap();
    let value = c.store.read_reference(reference).unwrap();
    assert_eq!((value.type_id, value.payload), (11, 1));
    assert_eq!(c.frame.counters.started_by_opcode[0], 1);
    assert_eq!(c.frame.counters.started_by_opcode[66], 1);
    assert_eq!(c.frame.counters.started_by_opcode[31], 1);
    assert_eq!(c.frame.counters.started_by_opcode[7], 1);
    assert_eq!(c.frame.counters.started_by_opcode[8], 0);
    // The block teardown retires the binding; the promoted place is
    // run-long storage no frame scope owns, so the reference stays valid
    // (`&'static`).
    c.frame.leave(block, &mut c.store).unwrap();
    assert_eq!(c.store.state(owner).unwrap(), CellState::Dropped);
    assert_eq!(c.store.read_reference(reference).unwrap(), value);
}
