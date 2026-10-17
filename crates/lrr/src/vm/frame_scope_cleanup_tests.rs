//! The unchanged first `drop(noImpl);` statement supplies its entire recipe.
//! Prepared intrinsic fields replace Rc/Cell/Field and expand to cross a chunk;
//! the source Drop call/destructors and whole upstream program remain unexecuted.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    ir::{op::Op, pc::Phase, recipe::VerifiedRecipe},
    runtime::shape::TypeKey,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{body_index::NodeRole, parse::ParsedFile, scope_fixture_facts},
    vm::step::{StepOutcome, step},
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/drop/struct-field-drop-order.rs"
);
struct Context {
    frame: Frame,
    store: GuestStore,
    recipe: VerifiedRecipe,
    root: CellId,
    owner: SlotId,
    fields: Vec<CellId>,
    scope: ScopeId,
}
fn context() -> Context {
    let (statement, argument) = scope_fixture_facts::original_record_drop_statement(ORIGINAL);
    let members = scope_fixture_facts::original_no_drop_record_members(ORIGINAL);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"stock-cleanup-scope"),
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
        .as_ref()
        .unwrap();
    let body = parsed.index_body(handle, Default::default()).unwrap();
    let node = body
        .nodes()
        .iter()
        .find(|node| node.range == statement && node.role == NodeRole::Statement)
        .unwrap();
    let owner = body
        .nodes()
        .iter()
        .find(|node| node.range == argument)
        .unwrap()
        .value
        .unwrap();
    let pc = body.pc(node.id, Phase::Commit).unwrap();
    let recipe = parsed.initial_thunk(handle, &body, &pc).unwrap();
    let [Op::Discard { source }, Op::ScopeLeave { scope }, ..] =
        recipe.recipe().operations.as_ref()
    else {
        panic!("stock statement did not produce discard then ScopeLeave")
    };
    let source = *source;
    let scope = *scope;
    assert_ne!(source, owner);
    let mut store = GuestStore::for_epoch(1).unwrap();
    let mut frame = Frame::new(body.clone(), pc, &mut store).unwrap();
    let mut ancestry = Vec::new();
    let mut current = Some(scope);
    while let Some(id) = current {
        ancestry.push(id);
        current = body.scopes()[id.0 as usize].parent;
    }
    for id in ancestry.into_iter().rev() {
        frame.enter(id).unwrap();
    }
    assert_eq!(frame.slots[owner.0 as usize].scope, scope);
    // The actual source call is outside this component. Its completed unit
    // result and owning argument storage are the explicitly prepared frontier.
    let unit = store.ticket(ValueRepr::unit()).unwrap();
    frame.write(source, unit, &mut store).unwrap();
    let first = store.string(&members[0]).unwrap();
    let element = TypeId(store.peek_ticket(first).unwrap().type_id);
    let count = 257;
    let ty = store
        .intern_type(TypeKey::Array {
            element,
            len: count,
        })
        .unwrap();
    let guard = store
        .begin_construction(&vec![element; count as usize])
        .unwrap();
    store.construction_push(guard, 0, first).unwrap();
    for ordinal in 1..count {
        let ticket = store
            .string(&members[ordinal as usize % members.len()])
            .unwrap();
        store.construction_push(guard, ordinal, ticket).unwrap();
    }
    let ticket = store.commit_construction(guard, ty).unwrap();
    let fields = (0..count)
        .map(|ordinal| store.aggregate_field(ticket, ordinal).unwrap())
        .collect();
    frame.write(owner, ticket, &mut store).unwrap();
    let root = frame.cell(owner).unwrap();
    Context {
        frame,
        store,
        recipe,
        root,
        owner,
        fields,
        scope,
    }
}
fn run(c: &mut Context) -> Result<StepOutcome, Diagnostic> {
    step(&mut c.frame, &mut c.store, &c.recipe)
}
fn reach_partial(c: &mut Context) {
    for _ in 0..32 {
        assert!(matches!(run(c).unwrap(), StepOutcome::Dispatch));
        if c.store.state(c.root).unwrap() == CellState::Dropping
            && c.fields
                .iter()
                .any(|field| c.store.state(*field).unwrap() == CellState::Dropped)
            && c.fields
                .iter()
                .any(|field| matches!(c.store.state(*field).unwrap(), CellState::Live(_)))
        {
            return;
        }
    }
    panic!("stock cleanup never exposed a partial bounded continuation");
}
#[test]
fn rustc_record_scope_leave_dispatch_preserves_owner_and_charges_opcode_once() {
    let mut c = context();
    let pc = c.frame.pc.clone();
    reach_partial(&mut c);
    assert_eq!(c.frame.pc, pc);
    assert_eq!(c.frame.cell(c.owner).unwrap(), c.root);
    assert_eq!(c.frame.active_scopes.last(), Some(&c.scope));
    assert_eq!(c.frame.counters.operations_started, 2);
    assert_eq!(c.frame.counters.operations_completed, 1);
    assert_eq!(c.frame.counters.started_by_opcode[5], 1);
    assert_eq!(
        c.store.begin_drop(c.root).unwrap_err().code,
        "LRR-E-OWNERSHIP"
    );
    let mut completed = false;
    for _ in 0..32 {
        match run(&mut c).unwrap() {
            StepOutcome::Dispatch => assert_eq!(c.frame.pc, pc),
            StepOutcome::Next(_) => {
                completed = true;
                break;
            }
            outcome => panic!("unexpected statement completion {outcome:?}"),
        }
        assert_eq!(c.frame.counters.started_by_opcode[5], 1);
    }
    assert!(completed);
    let operations = c.recipe.recipe().operations.len() as u64;
    assert_eq!(c.frame.counters.operations_started, operations);
    assert_eq!(c.frame.counters.operations_completed, operations);
    assert_eq!(c.frame.counters.exits_completed, 1);
    assert_eq!(c.frame.counters.started_by_opcode[5], 1);
    assert_eq!(
        c.frame.counters.cleanup_elements_completed,
        c.fields.len() as u64 + 1
    );
    assert_eq!(
        c.frame.counters.fuel_used,
        operations + c.fields.len() as u64 + 2
    );
    assert!(!c.frame.has_cleanup_activation());
    assert!(!c.frame.active_scopes.contains(&c.scope));
    assert!(c.frame.slots[c.owner.0 as usize].cell.is_none());
    assert_eq!(c.store.state(c.root).unwrap(), CellState::Dropped);
    assert!(
        c.fields
            .iter()
            .all(|field| c.store.state(*field).unwrap() == CellState::Dropped)
    );
}
