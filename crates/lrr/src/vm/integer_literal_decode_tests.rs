//! CA027 acceptance DRAFT. Requires the proposed Integer payload and real scoped
//! syntax decode observer; no test here certifies a complete upstream fixture.
use super::*;
use crate::{
    core::{digest::Digest, profile::ProfileIdentity},
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::names::NameNamespaces,
    ir::{
        op::{DecodedLiteral, Op, PrimitiveType},
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
    syntax::{literal_decode::observe_integer_decoding, parse::ParseKey},
    vm::step::{StepOutcome, step},
};
const RADIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-radix.rs"
);
const SUFFIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
);
const WIDE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/i128-min-literal-parses.rs"
);
const TOO_LARGE: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/parser/int-literal-too-large-span.rs"
);
const INVALID_SUFFIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/parser/bad-lit-suffixes.rs"
);
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

struct Prepared {
    db: Database,
    index: BodyIndex,
    recipe: VerifiedRecipe,
    destination: SlotId,
}
fn prepare(text: &str, edition: u16, marker: &str, spelling: &str) -> Prepared {
    let (mut db, view, _) = source(text, edition);
    db.trace.detailed = true;
    let handle = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let absolute = text.find(marker).unwrap() + marker.find(spelling).unwrap();
    let relative = u32::try_from(absolute)
        .unwrap()
        .checked_sub(handle.range.start)
        .unwrap();
    let fetched = db
        .fetch(&SyntaxProvider, QueryKey::BodyStructure(handle.key()))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
        panic!()
    };
    let node = index
        .nodes()
        .iter()
        .find(|node| {
            node.range.start == relative
                && node.range.end == relative + u32::try_from(spelling.len()).unwrap()
        })
        .expect("exact original literal/prefix node");
    let pc = index.pc(node.id, Phase::Entry).unwrap();
    let fetched = db
        .fetch(&SyntaxProvider, QueryKey::Thunk(pc))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
        panic!()
    };
    let destination = recipe
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::Literal { destination, .. } => Some(*destination),
            _ => None,
        })
        .unwrap();
    Prepared {
        db,
        index: index.as_ref().clone(),
        recipe: recipe.as_ref().clone(),
        destination,
    }
}
fn literal(p: &Prepared) -> &crate::ir::op::LiteralRecipe {
    p.recipe
        .recipe()
        .operations
        .iter()
        .find_map(|op| match op {
            Op::Literal { literal, .. } => Some(literal),
            _ => None,
        })
        .unwrap()
}
fn frame(p: &Prepared, store: &mut GuestStore, fuel: u64) -> Frame {
    let pc = p.recipe.recipe().key.clone();
    let mut frame = Frame::new(p.index.clone(), pc.clone(), store).unwrap();
    frame.set_fuel_limit(fuel).unwrap();
    let node = &p.index.nodes()[pc.node as usize];
    let mut scope = match p.recipe.recipe().operations.first() {
        Some(Op::ScopeEnter { scope }) => p.index.scopes()[scope.0 as usize].parent,
        _ => Some(node.scope),
    };
    let mut ancestors = vec![];
    while let Some(id) = scope {
        ancestors.push(id);
        scope = p.index.scopes()[id.0 as usize].parent;
    }
    for id in ancestors.into_iter().rev() {
        frame.enter(id).unwrap();
    }
    frame
}
fn draft(p: &Prepared) -> RecipeDraft {
    let r = p.recipe.recipe();
    RecipeDraft {
        key: r.key.clone(),
        origin: r.origin.clone(),
        operations: r.operations.to_vec(),
        exits: vec![r.exit.clone()],
        dependencies: r.dependencies.to_vec(),
    }
}

#[test]
fn store_session_normal_dispatch_refuses_same_bit_foreign_store_before_opcode_entry() {
    for edition in [2015, 2018, 2021, 2024] {
        let p = prepare(RADIX, edition, "let a = 0xBEEF_isize;", "0xBEEF_isize");
        let mut sa = GuestStore::new(1, 10000, 4 << 20).unwrap();
        let mut sb = GuestStore::new(1, 10000, 4 << 20).unwrap();
        let mut a = frame(&p, &mut sa, 100);
        let b = frame(&p, &mut sb, 100);
        assert_eq!(a.constant_frame_anchor(), b.constant_frame_anchor());
        let pc = a.pc().clone();
        let before = (sb.budget().handles(), sb.budget().bytes());
        assert_eq!(
            step(&mut a, &mut sb, &p.recipe).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(a.status, FrameStatus::Running);
        assert_eq!(a.pc(), &pc);
        assert_eq!(a.fuel.snapshot().used, 0);
        assert_eq!(a.counters.operations_started, 0);
        assert_eq!(before, (sb.budget().handles(), sb.budget().bytes()));
        assert!(a.cell(p.destination).is_err());
        assert!(step(&mut a, &mut sa, &p.recipe).is_ok());
        let crate::runtime::cell::CellState::Live(value) =
            sa.state(a.cell(p.destination).unwrap()).unwrap()
        else {
            panic!("original literal did not publish in its owning store")
        };
        assert_eq!(value.payload, 48879);
    }
}
#[test]
fn official_radices_separators_suffixes_and_signed_minima_have_exact_immutable_payload_all_editions()
 {
    for edition in [2015, 2018, 2021, 2024] {
        for (source, marker, spelling, negative, radix, magnitude, suffix) in [
            (
                RADIX,
                "let a = 0xBEEF_isize;",
                "0xBEEF_isize",
                false,
                16,
                48879u128,
                Some(PrimitiveType::Isize),
            ),
            (
                RADIX,
                "let b = 0o755_isize;",
                "0o755_isize",
                false,
                8,
                493,
                Some(PrimitiveType::Isize),
            ),
            (
                RADIX,
                "let c = 0b10101_isize;",
                "0b10101_isize",
                false,
                2,
                21,
                Some(PrimitiveType::Isize),
            ),
            (
                RADIX,
                "let d = -0xBEEF_isize;",
                "-0xBEEF_isize",
                true,
                16,
                48879,
                Some(PrimitiveType::Isize),
            ),
            (
                RADIX,
                "let e = -0o755_isize;",
                "-0o755_isize",
                true,
                8,
                493,
                Some(PrimitiveType::Isize),
            ),
            (
                RADIX,
                "let f = -0b10101_isize;",
                "-0b10101_isize",
                true,
                2,
                21,
                Some(PrimitiveType::Isize),
            ),
            (SUFFIX, "let j = -32_768;", "-32_768", true, 10, 32768, None),
            (
                SUFFIX,
                "let j = -9_223_372_036_854_775_808;",
                "-9_223_372_036_854_775_808",
                true,
                10,
                1u128 << 63,
                None,
            ),
            (
                SUFFIX,
                "let j = 18_446_744_073_709_551_615;",
                "18_446_744_073_709_551_615",
                false,
                10,
                u128::from(u64::MAX),
                None,
            ),
            (
                WIDE,
                "-0x8000_0000_0000_0000_0000_0000_0000_0000i128",
                "-0x8000_0000_0000_0000_0000_0000_0000_0000i128",
                true,
                16,
                1u128 << 127,
                Some(PrimitiveType::I128),
            ),
        ] {
            let (p, counts) =
                observe_integer_decoding(|| prepare(source, edition, marker, spelling));
            assert_eq!(counts.attempts, 1);
            assert_eq!(counts.successes, 1);
            assert_eq!(counts.failures, 0);
            assert_eq!(p.index.profile().edition, edition);
            assert_eq!(literal(&p).spelling, spelling);
            assert!(
                matches!(literal(&p).decoded,Some(DecodedLiteral::Integer{negative:n,radix:r,magnitude:m,suffix:s}) if (n,r,m,s)==(negative,radix,magnitude,suffix))
            );
        }
    }
}
#[test]
fn same_prepared_stock_recipe_memo_and_vm_repetition_do_no_repeated_lexical_decode() {
    let (_, counts) = observe_integer_decoding(|| {
        let mut p = prepare(RADIX, 2024, "let a = 0xBEEF_isize;", "0xBEEF_isize");
        for _ in 0..4 {
            let fetched =
                p.db.fetch(
                    &SyntaxProvider,
                    QueryKey::Thunk(p.recipe.recipe().key.clone()),
                )
                .into_ready()
                .unwrap()
                .unwrap();
            let QueryValue::Thunk(reused) = fetched.value.as_ref() else {
                panic!()
            };
            assert_eq!(reused.recipe(), p.recipe.recipe());
        }
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        for _ in 0..16 {
            let mut f = frame(&p, &mut store, 100);
            assert!(matches!(
                step(&mut f, &mut store, &p.recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            assert_eq!(
                store.read_scalar(f.cell(p.destination).unwrap()).unwrap(),
                ValueRepr::signed(TypeId(8), 48879).unwrap()
            );
            assert_eq!(f.counters.started_by_opcode[0], 1);
            assert_eq!(
                f.counters.operations_started,
                f.counters.operations_completed
            );
        }
    });
    assert_eq!(counts.attempts, 1);
    assert_eq!(counts.successes, 1);
    assert_eq!(counts.failures, 0);
}
#[test]
fn repeated_unsuffixed_stock_recipe_owns_fresh_binding_per_evaluation_but_copies_share() {
    let (_, counts) = observe_integer_decoding(|| {
        let p = prepare(SUFFIX, 2024, "let j = -128;", "-128");
        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let mut a = frame(&p, &mut store, 100);
        let mut b = frame(&p, &mut store, 100);
        assert!(matches!(
            step(&mut a, &mut store, &p.recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        assert!(matches!(
            step(&mut b, &mut store, &p.recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        let a_cell = a.cell(p.destination).unwrap();
        let b_cell = b.cell(p.destination).unwrap();
        let CellState::Live(a_value) = store.state(a_cell).unwrap() else {
            panic!()
        };
        let CellState::Live(b_value) = store.state(b_cell).unwrap() else {
            panic!()
        };
        assert_eq!((a_value.kind, a_value.type_id), (11, 0));
        assert_eq!((b_value.kind, b_value.type_id), (11, 0));
        assert_ne!(a_value.payload, b_value.payload);
        let a_copy = store.copy_from(a_cell).unwrap();
        let a_other = store.copy_from(a_cell).unwrap();
        let b_copy = store.copy_from(b_cell).unwrap();
        assert_eq!(
            store.peek_ticket(a_copy).unwrap().payload,
            store.peek_ticket(a_other).unwrap().payload
        );
        let bound_a = store.materialize_ticket(a_copy, TypeId(4)).unwrap();
        assert_eq!(
            store.peek_ticket(bound_a).unwrap(),
            ValueRepr::signed(TypeId(4), -128).unwrap()
        );
        assert_eq!(
            store
                .materialize_ticket(a_other, TypeId(7))
                .unwrap_err()
                .code,
            "LRR-E-OWNERSHIP"
        );
        let bound_b = store.materialize_ticket(b_copy, TypeId(7)).unwrap();
        assert_eq!(
            store.peek_ticket(bound_b).unwrap(),
            ValueRepr::signed(TypeId(7), -128).unwrap()
        );
    });
    assert_eq!(counts.attempts, 1);
    assert_eq!(counts.successes, 1);
}
#[test]
fn missing_legacy_integer_metadata_and_wrong_sign_are_structurally_refused() {
    let p = prepare(RADIX, 2024, "let a = 0xBEEF_isize;", "0xBEEF_isize");
    for wrong_sign in [false, true] {
        let mut d = draft(&p);
        let Op::Literal { literal, .. } = d
            .operations
            .iter_mut()
            .find(|op| matches!(op, Op::Literal { .. }))
            .unwrap()
        else {
            panic!()
        };
        if wrong_sign {
            let Some(DecodedLiteral::Integer { negative, .. }) = &mut literal.decoded else {
                panic!()
            };
            *negative = true;
        } else {
            literal.decoded = None;
        }
        assert_eq!(verify(d, &p.index).unwrap_err().code, "LRR-E-INTERNAL-IR");
    }
    // This is raw legacy metadata refusal, not a claimed persisted checkpoint restore.
}
#[test]
fn original_i128_minimum_lexical_payload_does_not_admit_guest_i128_execution() {
    let spelling = "-0x8000_0000_0000_0000_0000_0000_0000_0000i128";
    let p = prepare(WIDE, 2024, spelling, spelling);
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let mut f = frame(&p, &mut store, 100);
    let handles = store.budget().handles();
    let error = step(&mut f, &mut store, &p.recipe).unwrap_err();
    assert_eq!(error.code, "LRR-E-UNSUPPORTED");
    assert_eq!(
        error.message,
        "128-bit guest integers are outside the live profile"
    );
    assert_eq!(f.counters.started_by_opcode[0], 0);
    assert_eq!(store.budget().handles(), handles);
    assert!(f.cell(p.destination).is_err());
}
#[test]
fn unchanged_oversized_literal_fault_is_retained_at_exact_reached_source_boundary() {
    let spelling = TOO_LARGE
        .lines()
        .find(|line| line.trim_start().starts_with("999999999"))
        .unwrap()
        .trim();
    let (p, counts) = observe_integer_decoding(|| prepare(TOO_LARGE, 2024, spelling, spelling));
    assert_eq!(counts.attempts, 1);
    assert_eq!(counts.failures, 1);
    assert_eq!(counts.successes, 0);
    assert!(matches!(
        literal(&p).decoded,
        Some(DecodedLiteral::InvalidInteger { .. })
    ));
    let expected_source = literal(&p).source.clone();
    assert_eq!(expected_source, p.recipe.recipe().origin.span);
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let mut f = frame(&p, &mut store, 100);
    let before = (store.budget().handles(), store.budget().bytes());
    let error = step(&mut f, &mut store, &p.recipe).unwrap_err();
    assert_eq!(error.code, "LRR-E-INTERNAL-IR");
    assert_eq!(
        error.message,
        "invalid integer literal spelling or magnitude"
    );
    assert_eq!((store.budget().handles(), store.budget().bytes()), before);
    assert_eq!(f.counters.started_by_opcode[0], 0);
    assert!(f.cell(p.destination).is_err());
    assert_eq!(
        f.fuel_snapshot().used,
        u64::try_from(
            p.recipe
                .recipe()
                .operations
                .iter()
                .position(|op| matches!(op, Op::Literal { .. }))
                .unwrap()
        )
        .unwrap()
    );
}
#[test]
fn supplemental_cold_body_with_official_invalid_literal_is_not_lexically_decoded() {
    // This is an LRR laziness adaptation outside the originals: valid official
    // main stays unchanged, and only the invalid fixture's function name changes.
    let text = format!("{RADIX}\n{}", TOO_LARGE.replace("fn main()", "fn cold()"));
    let (p, counts) =
        observe_integer_decoding(|| prepare(&text, 2024, "let a = 0xBEEF_isize;", "0xBEEF_isize"));
    assert_eq!(counts.attempts, 1);
    assert_eq!(counts.failures, 0);
    assert!(p.db.trace.events.iter().all(|event|!matches!(&event.key,QueryKey::Thunk(key) if key.body.definition!=p.index.body().definition)));
}

#[test]
fn unchanged_invalid_numeric_suffix_is_a_cached_reached_fault_without_type_identity() {
    let (p, counts) =
        observe_integer_decoding(|| prepare(INVALID_SUFFIX, 2024, "1234suffix;", "1234suffix"));
    assert_eq!(counts.attempts, 1);
    assert_eq!(counts.failures, 1);
    assert_eq!(counts.successes, 0);
    assert!(matches!(
        literal(&p).decoded,
        Some(DecodedLiteral::InvalidInteger {
            negative: false,
            radix: 10,
            suffix: None,
            reason: crate::ir::op::IntegerLexicalError::InvalidSuffix,
        })
    ));
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let mut f = frame(&p, &mut store, 100);
    let before = (store.budget().handles(), store.budget().bytes());
    let error = step(&mut f, &mut store, &p.recipe).unwrap_err();
    assert_eq!(error.code, "LRR-E-INTERNAL-IR");
    assert_eq!(
        error.message,
        "invalid integer literal spelling or magnitude"
    );
    assert_eq!((store.budget().handles(), store.budget().bytes()), before);
    assert_eq!(f.counters.started_by_opcode[0], 0);
    assert!(f.cell(p.destination).is_err());
    assert_eq!(
        f.fuel_snapshot().used,
        u64::try_from(
            p.recipe
                .recipe()
                .operations
                .iter()
                .position(|op| matches!(op, Op::Literal { .. }))
                .unwrap()
        )
        .unwrap()
    );
}
