//! Intrinsic source identity from genuine ordinary pinned literal recipes.
//! Original initializer/binding progress is component evidence, not full fixtures.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        call_contracts::{ContractPreparation, PreparedCallContracts, prepare_contracts},
        callables::{CallResolver, CallSelection, SourceCallable},
        names::NameNamespaces,
    },
    ir::{
        op::{LiteralKind, Op, PrimitiveType},
        pc::Phase,
        recipe::{RecipeDraft, VerifiedRecipe},
        validate::verify,
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
    traits::term::{ParamKey, TyTerm},
    vm::{
        context::GenericEnv,
        step::{StepOutcome, step},
    },
};
const SHADOW: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/resolve/shadow-primitives.rs"
);
const CHAR: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/match/issue-26251.rs");
fn source(text: &str, edition: u16) -> (Database, PreparedSourceView, NameNamespaces) {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("return-context.rs").unwrap();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let unit = Digest::of_bytes(b"LRR deferred return source world");
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
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    (db, view, graph)
}
fn contracts(
    db: &mut Database,
    view: &PreparedSourceView,
    graph: &NameNamespaces,
    name: &str,
) -> (Box<SourceCallable>, PreparedCallContracts) {
    let caller = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .def_key
        .fingerprint();
    let sources = [view];
    let resolver = CallResolver::new(graph, &sources).unwrap();
    let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
        resolver.module_function(db, caller, name)
    else {
        panic!("callee unavailable")
    };
    let QueryPoll::Ready(Ok(ContractPreparation::Ready(contracts))) =
        prepare_contracts(db, &callee, graph, &sources, &[], None, &[])
    else {
        panic!("contract unresolved")
    };
    (callee, contracts)
}

struct LiteralContext {
    db: Database,
    frame: Frame,
    store: GuestStore,
    recipe: VerifiedRecipe,
    destination: SlotId,
}
fn literal_context(
    text: &str,
    edition: u16,
    kind: LiteralKind,
    fuel: u64,
    handles: u32,
    bytes: usize,
) -> LiteralContext {
    literal_context_in(text, edition, kind, fuel, handles, bytes, "main")
}
fn literal_context_in(
    text: &str,
    edition: u16,
    kind: LiteralKind,
    fuel: u64,
    handles: u32,
    bytes: usize,
    function: &str,
) -> LiteralContext {
    let (mut db, view, _) = source(text, edition);
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == function)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let indexed = db
        .fetch(&SyntaxProvider, QueryKey::BodyStructure(body))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!()
    };
    let mut selected = None;
    for node in index.nodes().iter().filter(|node| {
        node.role
            == crate::syntax::body_index::NodeRole::Expression(
                crate::syntax::body_index::ExpressionKind::Literal,
            )
            || node.negative_integer_literal
    }) {
        // New string controls select actual expression literals rather than
        // constant pattern endpoints, whose matching semantics stay separate.
        if kind == LiteralKind::String
            && node.parent.is_some_and(|parent| {
                index.nodes()[parent.0 as usize].role
                    == crate::syntax::body_index::NodeRole::Pattern
            })
        {
            continue;
        }
        let pc = index.pc(node.id, Phase::Entry).unwrap();
        let outcome = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!()
        };
        if let Some(destination) = recipe.recipe().operations.iter().find_map(|op| match op {
            Op::Literal {
                literal,
                destination,
                ..
            } if literal.kind == kind => Some(*destination),
            _ => None,
        }) {
            selected = Some((pc, recipe.as_ref().clone(), destination));
            break;
        }
    }
    let (pc, recipe, destination) = selected.expect("selected ordinary source literal absent");
    let mut store = GuestStore::new(1, handles, bytes).unwrap();
    let mut frame = Frame::new(std::sync::Arc::clone(index), pc.clone(), &mut store).unwrap();
    frame.set_fuel_limit(fuel).unwrap();
    // Keep the exact recipe's charged scope prefix; enter only outer ancestors.
    let node = &index.nodes()[pc.node as usize];
    let mut scope = match recipe.recipe().operations.first() {
        Some(Op::ScopeEnter { scope }) => index.scopes()[scope.0 as usize].parent,
        _ => Some(node.scope),
    };
    let mut ancestors = vec![];
    while let Some(id) = scope {
        ancestors.push(id);
        scope = index.scopes()[id.0 as usize].parent;
    }
    for id in ancestors.into_iter().rev() {
        frame.enter(id).unwrap();
    }
    LiteralContext {
        db,
        frame,
        store,
        recipe,
        destination,
    }
}
fn run(c: &mut LiteralContext) -> Result<StepOutcome, Diagnostic> {
    step(&mut c.frame, &mut c.store, &c.recipe)
}
fn footprint(c: &LiteralContext) -> (u32, usize) {
    (c.store.budget().handles(), c.store.budget().bytes())
}
fn prefix(c: &LiteralContext) -> u64 {
    u64::try_from(
        c.recipe
            .recipe()
            .operations
            .iter()
            .position(|op| matches!(op, Op::Literal { .. }))
            .unwrap(),
    )
    .unwrap()
}
fn assert_untouched(c: &LiteralContext, before: (u32, usize), source: Option<TyTerm>) {
    assert_eq!(footprint(c), before);
    assert!(c.frame.cell(c.destination).is_err());
    assert_eq!(c.frame.source_view(c.destination).unwrap(), source);
    assert_eq!(c.frame.counters.started_by_opcode[0], 0);
    assert_eq!(c.frame.fuel_snapshot().used, prefix(c));
}
#[test]
fn unchanged_official_bool_char_initializers_and_bindings_keep_intrinsic_source_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (original, kind, primitive, value) in [
            (
                SHADOW,
                LiteralKind::Bool,
                PrimitiveType::Bool,
                ValueRepr::boolean(true),
            ),
            (
                CHAR,
                LiteralKind::Char,
                PrimitiveType::Char,
                ValueRepr::character('a'),
            ),
        ] {
            let mut c = literal_context(original, edition, kind, 100, 1000, 1_000_000);
            assert!(matches!(run(&mut c).unwrap(), StepOutcome::Next(_)));
            assert_eq!(
                c.store
                    .read_scalar(c.frame.cell(c.destination).unwrap())
                    .unwrap(),
                value
            );
            assert_eq!(
                c.frame.source_view(c.destination).unwrap(),
                Some(TyTerm::Primitive(primitive))
            );
            assert_eq!(c.frame.counters.started_by_opcode[0], 1);
            let fetched =
                c.db.fetch(&SyntaxProvider, QueryKey::Thunk(c.frame.pc().clone()))
                    .into_ready()
                    .unwrap()
                    .unwrap();
            let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
                panic!()
            };
            let binding = recipe
                .recipe()
                .operations
                .iter()
                .find_map(|op| match op {
                    Op::BindLocal { binding, .. } => Some(*binding),
                    _ => None,
                })
                .expect("actual next statement must bind literal");
            assert!(matches!(
                step(&mut c.frame, &mut c.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            assert_eq!(
                c.frame.binding_sources[binding.0 as usize],
                Some(TyTerm::Primitive(primitive))
            );
            let cell = c.frame.bindings[binding.0 as usize].unwrap();
            assert_eq!(c.store.read_scalar(cell).unwrap(), value);
            assert_eq!(c.frame.counters.started_by_opcode[7], 1);
        }
    }
}
#[test]
fn unannotated_local_returns_through_source_normalized_alias_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for (target, literal, primitive, value) in [
            (
                "bool",
                "true",
                PrimitiveType::Bool,
                ValueRepr::boolean(true),
            ),
            (
                "char",
                "'🦀'",
                PrimitiveType::Char,
                ValueRepr::character('🦀'),
            ),
        ] {
            let text = format!(
                "type i64={target};fn main(){{}}fn chosen()->i64{{let value={literal};value}}"
            );
            let (mut db, view, graph) = source(&text, edition);
            let (callee, prepared) = contracts(&mut db, &view, &graph, "chosen");
            let context = GenericEnv::capture_current(&mut db, &callee, &prepared).unwrap();
            let fetched = db
                .fetch(
                    &SyntaxProvider,
                    QueryKey::BodyStructure(callee.body().unwrap().key()),
                )
                .into_ready()
                .unwrap()
                .unwrap();
            let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
                panic!()
            };
            let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
            let mut frame = Frame::new(
                index.as_ref().clone(),
                index.pc(NodeId(0), Phase::Entry).unwrap(),
                &mut store,
            )
            .unwrap();
            frame.install_context(context).unwrap();
            frame.set_fuel_limit(100).unwrap();
            let mut returned = None;
            let mut variable_seen = false;
            for _ in 0..64 {
                let fetched = db
                    .fetch(&SyntaxProvider, QueryKey::Thunk(frame.pc().clone()))
                    .into_ready()
                    .unwrap()
                    .unwrap();
                let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
                    panic!()
                };
                let read = recipe.recipe().operations.iter().find_map(|op| match op {
                    Op::ReadLocal { destination, .. } => Some(*destination),
                    _ => None,
                });
                match step(&mut frame, &mut store, recipe).unwrap() {
                    StepOutcome::Next(_) => {}
                    StepOutcome::Return(ticket) => {
                        returned = Some(ticket);
                        break;
                    }
                    other => panic!("{other:?}"),
                }
                if let Some(destination) = read {
                    variable_seen = true;
                    assert_eq!(
                        frame.source_view(destination).unwrap(),
                        Some(TyTerm::Primitive(primitive))
                    );
                }
            }
            assert!(variable_seen, "return must read actual unannotated local");
            assert_eq!(
                store
                    .peek_ticket(returned.expect("actual chosen body did not return"))
                    .unwrap(),
                value
            );
            assert_eq!(frame.return_source, Some(TyTerm::Primitive(primitive)));
            assert_eq!(frame.counters.started_by_opcode[0], 1);
        }
    }
}
#[test]
fn ordinary_literals_reject_rigid_nominal_or_wrong_primitive_source_before_entry() {
    for (text, kind, primitive) in [
        (
            "fn main(){let value=true;}",
            LiteralKind::Bool,
            PrimitiveType::Bool,
        ),
        (
            "fn main(){let value='a';}",
            LiteralKind::Char,
            PrimitiveType::Char,
        ),
        (
            "fn main(){let value=7u8;}",
            LiteralKind::Integer,
            PrimitiveType::U8,
        ),
        (
            "fn main(){let value=b'a';}",
            LiteralKind::Byte,
            PrimitiveType::U8,
        ),
        (
            "fn main(){let value=1.5f32;}",
            LiteralKind::Float,
            PrimitiveType::F32,
        ),
    ] {
        for bad in [0, 1, 2] {
            let mut c = literal_context(text, 2024, kind, 100, 1000, 1_000_000);
            let source = match bad {
                0 => TyTerm::Param(ParamKey {
                    definition: c.frame.body.body().definition,
                    ordinal: 0,
                }),
                1 => TyTerm::Nominal(
                    Digest::of_bytes(b"official primitive-looking source shadow"),
                    vec![],
                ),
                _ => TyTerm::Primitive(PrimitiveType::I64),
            };
            c.frame
                .set_source_view(c.destination, Some(source.clone()))
                .unwrap();
            let descriptor = crate::runtime::shape::PrimitiveIdentity::from_recipe(primitive)
                .unwrap()
                .id();
            c.frame
                .declared_hint(c.destination, Some(descriptor))
                .unwrap();
            let before = footprint(&c);
            assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-TYPE-CONTEXT");
            assert_untouched(&c, before, Some(source));
        }
        let mut c = literal_context(text, 2024, kind, 100, 1000, 1_000_000);
        c.frame
            .set_source_view(c.destination, Some(TyTerm::Primitive(primitive)))
            .unwrap();
        assert!(matches!(run(&mut c).unwrap(), StepOutcome::Next(_)));
        assert_eq!(
            c.frame.source_view(c.destination).unwrap(),
            Some(TyTerm::Primitive(primitive))
        );
    }
}
#[test]
fn wrong_parent_descriptor_refuses_without_recovering_source() {
    let mut c = literal_context(
        "fn main(){let value=true;}",
        2024,
        LiteralKind::Bool,
        100,
        1000,
        1_000_000,
    );
    c.frame.set_hint(c.destination, Some(TypeId(7))).unwrap();
    let before = footprint(&c);
    // Slots begin with a strict storage hint. The existing descriptor guard
    // refuses this mismatch before the intrinsic source guard is reached.
    let error = run(&mut c).unwrap_err();
    assert_eq!(error.code, "LRR-E-OWNERSHIP");
    assert_eq!(
        error.message,
        "literal violates declared destination descriptor"
    );
    assert_eq!(c.frame.hint(c.destination).unwrap(), Some(TypeId(7)));
    assert!(c.frame.fuel_snapshot().stop.is_none());
    assert_untouched(&c, before, None);
}
#[test]
fn forged_literal_source_span_and_stale_body_recipe_refuse_before_entry() {
    let mut c = literal_context(
        "fn main(){let value=true;}",
        2024,
        LiteralKind::Bool,
        100,
        1000,
        1_000_000,
    );
    let r = c.recipe.recipe();
    let mut draft = RecipeDraft {
        key: r.key.clone(),
        origin: r.origin.clone(),
        operations: r.operations.to_vec(),
        exits: vec![r.exit.clone()],
        dependencies: r.dependencies.to_vec(),
    };
    let Op::Literal { literal, .. } = draft
        .operations
        .iter_mut()
        .find(|op| matches!(op, Op::Literal { .. }))
        .unwrap()
    else {
        panic!()
    };
    literal.source.relative_start += 1;
    c.recipe = verify(draft, &c.frame.body).unwrap();
    let before = footprint(&c);
    assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-INTERNAL-IR");
    assert_untouched(&c, before, None);
    let mut fresh = literal_context(
        "fn main(){let value=false;}",
        2024,
        LiteralKind::Bool,
        100,
        1000,
        1_000_000,
    );
    let old = literal_context(
        "fn main(){let value=true;}",
        2024,
        LiteralKind::Bool,
        100,
        1000,
        1_000_000,
    );
    fresh.recipe = old.recipe;
    let before = footprint(&fresh);
    assert_eq!(run(&mut fresh).unwrap_err().code, "LRR-E-INTERNAL-IR");
    assert_untouched(&fresh, before, None);
}
#[test]
fn literal_fuel_refusal_keeps_exact_prefix_and_single_write_no_replay() {
    let text = "fn main(){let value=true;}";
    let probe = literal_context(text, 2024, LiteralKind::Bool, 100, 1000, 1_000_000);
    let prefix = prefix(&probe);
    assert_eq!(prefix, 0);
    for limit in 0..=prefix + 1 {
        let mut c = literal_context(text, 2024, LiteralKind::Bool, limit, 1000, 1_000_000);
        let before = footprint(&c);
        let error = run(&mut c).unwrap_err();
        assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
        assert_eq!(error.message, "guest opcode fuel exhausted");
        assert_eq!(c.frame.status(), FrameStatus::Faulted);
        assert_eq!(
            c.frame.fuel_snapshot().stop,
            Some(crate::vm::fuel::FuelStop::Exhausted)
        );
        assert_eq!(c.frame.fuel_snapshot().used, limit);
        assert_eq!(c.frame.fuel_snapshot().remaining, 0);
        assert_eq!(c.frame.counters.operations_started, limit);
        assert_eq!(c.frame.counters.operations_completed, limit);
        assert_eq!(c.frame.counters.exits_completed, 0);
        if limit <= prefix {
            assert_untouched(&c, before, None)
        } else {
            assert_eq!(c.frame.counters.started_by_opcode[0], 1);
            assert_eq!(
                c.frame.source_view(c.destination).unwrap(),
                Some(TyTerm::Primitive(PrimitiveType::Bool))
            );
        }
        let counters = serde_json::to_value(c.frame.counters()).unwrap();
        let fuel = c.frame.fuel_snapshot();
        let after = footprint(&c);
        assert!(run(&mut c).is_err());
        assert_eq!(footprint(&c), after);
        assert_eq!(c.frame.fuel_snapshot(), fuel);
        assert_eq!(serde_json::to_value(c.frame.counters()).unwrap(), counters);
    }
}
#[test]
fn literal_whole_ticket_cell_quota_refuses_without_an_unowned_ticket() {
    let text = "fn main(){let value=true;}";
    let probe = literal_context(text, 2024, LiteralKind::Bool, 100, 1000, 1_000_000);
    let baseline = probe.store.budget().handles();
    assert_eq!(prefix(&probe), 0);
    for free in [0, 1] {
        let mut c = literal_context(
            text,
            2024,
            LiteralKind::Bool,
            100,
            baseline + free,
            1_000_000,
        );
        let before = footprint(&c);
        assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
        assert_untouched(&c, before, None);
    }
    // Arena reservation growth must be checked together before cell or ticket.
    let baseline_bytes = probe.store.budget().bytes();
    let mut c = literal_context(text, 2024, LiteralKind::Bool, 100, 1000, baseline_bytes);
    let before = footprint(&c);
    assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
    assert_untouched(&c, before, None);
}
#[test]
fn byte_source_is_intrinsic_but_unsuffixed_integer_does_not_invent_source_metadata() {
    for edition in [2015, 2018, 2021, 2024] {
        for (text, kind, expected) in [
            (
                "fn main(){let value=b'a';}",
                LiteralKind::Byte,
                Some(TyTerm::Primitive(PrimitiveType::U8)),
            ),
            ("fn main(){let value=1;}", LiteralKind::Integer, None),
        ] {
            let mut c = literal_context(text, edition, kind, 100, 1000, 1_000_000);
            assert!(matches!(run(&mut c).unwrap(), StepOutcome::Next(_)));
            assert_eq!(c.frame.source_view(c.destination).unwrap(), expected);
        }
    }
}

// Only the genuine first initializer and its binding are exercised here. The
// original files remain byte-identical; this is not a full-fixture pass.
#[test]
fn official_suffixed_numeric_initializers_retain_written_source_through_binding_all_editions() {
    const RADIX: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-radix.rs"
    );
    const FLOAT: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/floatlits.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for (original, kind, primitive, expected) in [
            (
                RADIX,
                LiteralKind::Integer,
                PrimitiveType::Isize,
                ValueRepr::signed(TypeId(8), 48879).unwrap(),
            ),
            (
                RADIX,
                LiteralKind::NegativeInteger,
                PrimitiveType::Isize,
                ValueRepr::signed(TypeId(8), -48879).unwrap(),
            ),
            (
                FLOAT,
                LiteralKind::Float,
                PrimitiveType::F64,
                ValueRepr::f64(4.999999999999),
            ),
        ] {
            let mut c = literal_context(original, edition, kind, 100, 1000, 1_000_000);
            assert!(matches!(run(&mut c).unwrap(), StepOutcome::Next(_)));
            assert_eq!(
                c.store
                    .read_scalar(c.frame.cell(c.destination).unwrap())
                    .unwrap(),
                expected
            );
            assert_eq!(
                c.frame.source_view(c.destination).unwrap(),
                Some(TyTerm::Primitive(primitive))
            );
            assert_eq!(c.frame.counters.started_by_opcode[0], 1);
            let query =
                c.db.fetch(&SyntaxProvider, QueryKey::Thunk(c.frame.pc().clone()))
                    .into_ready()
                    .unwrap()
                    .unwrap();
            let QueryValue::Thunk(next) = query.value.as_ref() else {
                panic!()
            };
            let binding = next
                .recipe()
                .operations
                .iter()
                .find_map(|op| match op {
                    Op::BindLocal { binding, .. } => Some(*binding),
                    _ => None,
                })
                .expect("original initializer's next statement must bind its value");
            // ScopeLeave can yield while scanning this original body's
            // reserved macro temporaries. Resume that exact recipe; only its
            // real Next exit completes the binding observation.
            let mut finished = false;
            for _ in 0..10_000 {
                match step(&mut c.frame, &mut c.store, next).unwrap() {
                    StepOutcome::Dispatch => {
                        assert_eq!(c.frame.pc(), &next.recipe().key);
                        assert_eq!(c.frame.counters.started_by_opcode[0], 1);
                    }
                    StepOutcome::Next(_) => {
                        finished = true;
                        break;
                    }
                    outcome => panic!("original binding produced unexpected outcome: {outcome:?}"),
                }
            }
            assert!(
                finished,
                "original binding did not finish its bounded cleanup"
            );
            assert_eq!(
                c.frame.binding_sources[binding.0 as usize],
                Some(TyTerm::Primitive(primitive))
            );
            let bound = c.frame.bindings[binding.0 as usize].unwrap();
            assert_eq!(c.store.read_scalar(bound).unwrap(), expected);
            assert_eq!(c.frame.counters.started_by_opcode[7], 1);
        }
    }
}

#[test]
fn suffixed_numeric_source_attachment_preserves_exact_fuel_prefix_and_no_replay() {
    for (text, kind, primitive) in [
        (
            "fn main(){let value=7u8;}",
            LiteralKind::Integer,
            PrimitiveType::U8,
        ),
        (
            "fn main(){let value=1.5f32;}",
            LiteralKind::Float,
            PrimitiveType::F32,
        ),
    ] {
        let probe = literal_context(text, 2024, kind, 100, 1000, 1_000_000);
        let prefix = prefix(&probe);
        for limit in 0..=prefix + 1 {
            let mut c = literal_context(text, 2024, kind, limit, 1000, 1_000_000);
            let before = footprint(&c);
            let error = run(&mut c).unwrap_err();
            assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
            assert_eq!(error.message, "guest opcode fuel exhausted");
            assert_eq!(c.frame.fuel_snapshot().used, limit);
            assert_eq!(c.frame.counters.operations_started, limit);
            assert_eq!(c.frame.counters.operations_completed, limit);
            assert_eq!(c.frame.counters.exits_completed, 0);
            if limit <= prefix {
                assert_untouched(&c, before, None);
            } else {
                assert_eq!(c.frame.counters.started_by_opcode[0], 1);
                assert_eq!(
                    c.frame.source_view(c.destination).unwrap(),
                    Some(TyTerm::Primitive(primitive))
                );
            }
            let stopped = footprint(&c);
            let started = c.frame.counters.operations_started;
            assert!(run(&mut c).is_err());
            assert_eq!(footprint(&c), stopped);
            assert_eq!(c.frame.counters.operations_started, started);
        }
    }
}

#[test]
fn byte_literal_guards_source_descriptor_origin_and_stale_body_before_entry_all_editions() {
    const TEXT: &str = "fn main(){let value=b'a';}";
    for edition in [2015, 2018, 2021, 2024] {
        for bad in [
            TyTerm::Nominal(Digest::of_bytes(b"byte source rigid nominal"), vec![]),
            TyTerm::Primitive(PrimitiveType::Char),
        ] {
            let mut c = literal_context(TEXT, edition, LiteralKind::Byte, 100, 1000, 1_000_000);
            c.frame
                .set_source_view(c.destination, Some(bad.clone()))
                .unwrap();
            let u8_type = crate::runtime::shape::PrimitiveIdentity::from_recipe(PrimitiveType::U8)
                .unwrap()
                .id();
            c.frame.declared_hint(c.destination, Some(u8_type)).unwrap();
            let before = footprint(&c);
            assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-TYPE-CONTEXT");
            assert_untouched(&c, before, Some(bad));
        }
        let mut c = literal_context(TEXT, edition, LiteralKind::Byte, 100, 1000, 1_000_000);
        let param = TyTerm::Param(ParamKey {
            definition: c.frame.body.body().definition,
            ordinal: 0,
        });
        c.frame
            .set_source_view(c.destination, Some(param.clone()))
            .unwrap();
        let before = footprint(&c);
        assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-TYPE-CONTEXT");
        assert_untouched(&c, before, Some(param));

        let mut c = literal_context(TEXT, edition, LiteralKind::Byte, 100, 1000, 1_000_000);
        c.frame.set_hint(c.destination, Some(TypeId(7))).unwrap();
        let before = footprint(&c);
        let error = run(&mut c).unwrap_err();
        assert_eq!(error.code, "LRR-E-OWNERSHIP");
        assert_eq!(
            error.message,
            "literal violates declared destination descriptor"
        );
        assert_untouched(&c, before, None);

        let mut c = literal_context(TEXT, edition, LiteralKind::Byte, 100, 1000, 1_000_000);
        let r = c.recipe.recipe();
        let mut draft = RecipeDraft {
            key: r.key.clone(),
            origin: r.origin.clone(),
            operations: r.operations.to_vec(),
            exits: vec![r.exit.clone()],
            dependencies: r.dependencies.to_vec(),
        };
        let Op::Literal { literal, .. } = draft
            .operations
            .iter_mut()
            .find(|op| matches!(op, Op::Literal { .. }))
            .unwrap()
        else {
            panic!()
        };
        literal.source.relative_start += 1;
        c.recipe = verify(draft, &c.frame.body).unwrap();
        let before = footprint(&c);
        assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-INTERNAL-IR");
        assert_untouched(&c, before, None);

        let mut fresh = literal_context(
            "fn main(){let value=b'b';}",
            edition,
            LiteralKind::Byte,
            100,
            1000,
            1_000_000,
        );
        fresh.recipe =
            literal_context(TEXT, edition, LiteralKind::Byte, 100, 1000, 1_000_000).recipe;
        let before = footprint(&fresh);
        assert_eq!(run(&mut fresh).unwrap_err().code, "LRR-E-INTERNAL-IR");
        assert_untouched(&fresh, before, None);
    }
}
#[test]
fn byte_literal_fuel_and_combined_ticket_cell_quota_refuse_without_source_invention_all_editions() {
    const TEXT: &str = "fn main(){let value=b'a';}";
    for edition in [2015, 2018, 2021, 2024] {
        let probe = literal_context(TEXT, edition, LiteralKind::Byte, 100, 1000, 1_000_000);
        assert_eq!(prefix(&probe), 0);
        for limit in [0, 1] {
            let mut c = literal_context(TEXT, edition, LiteralKind::Byte, limit, 1000, 1_000_000);
            let before = footprint(&c);
            let error = run(&mut c).unwrap_err();
            assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
            assert_eq!(error.message, "guest opcode fuel exhausted");
            assert_eq!(c.frame.fuel_snapshot().used, limit);
            assert_eq!(c.frame.counters.operations_started, limit);
            assert_eq!(c.frame.counters.operations_completed, limit);
            if limit == 0 {
                assert_untouched(&c, before, None)
            } else {
                assert_eq!(
                    c.frame.source_view(c.destination).unwrap(),
                    Some(TyTerm::Primitive(PrimitiveType::U8))
                );
                let cell = c.frame.cell(c.destination).unwrap();
                let value = c.store.read_scalar(cell).unwrap();
                let u8_type =
                    crate::runtime::shape::PrimitiveIdentity::from_recipe(PrimitiveType::U8)
                        .unwrap()
                        .id();
                assert_eq!(value, ValueRepr::unsigned(u8_type, 97).unwrap());
            }
            let stopped = footprint(&c);
            let fuel = c.frame.fuel_snapshot();
            let counters = serde_json::to_value(c.frame.counters()).unwrap();
            assert!(run(&mut c).is_err());
            assert_eq!(footprint(&c), stopped);
            assert_eq!(c.frame.fuel_snapshot(), fuel);
            assert_eq!(serde_json::to_value(c.frame.counters()).unwrap(), counters);
        }
        let handles = probe.store.budget().handles();
        for free in [0, 1] {
            let mut c = literal_context(
                TEXT,
                edition,
                LiteralKind::Byte,
                100,
                handles + free,
                1_000_000,
            );
            let before = footprint(&c);
            assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
            assert_untouched(&c, before, None);
        }
        let mut c = literal_context(
            TEXT,
            edition,
            LiteralKind::Byte,
            100,
            1000,
            probe.store.budget().bytes(),
        );
        let before = footprint(&c);
        assert_eq!(run(&mut c).unwrap_err().code, "LRR-E-RUNTIME-RESOURCE");
        assert_untouched(&c, before, None);
    }
}

#[path = "string_literal_source_tests.rs"]
mod string_literal_source_tests;
