//! Source-owned cast expectations from pinned rustc; component/adapter evidence.
//! No rustc execution or complete upstream static diagnostic pass is claimed.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        key::{OutlineKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    engine::run::{RunMachine, RunPoll, ValueSummary},
    ir::{
        op::{HintRecipe, Op, PrimitiveType},
        pc::Phase,
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
const UNSIGNED: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/unary-negation-unsigned-integer-error.rs"
);
const FALLBACK: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/cast/cast-does-fallback.rs");
const RFC_CASTS: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/cast/cast-rfc0401.rs");
fn source(text: &str, edition: u16) -> (Database, PreparedSourceView) {
    let path = SourcePath::new("cast-context.rs").unwrap();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let owner = Digest::of_bytes(b"LRR exact cast operand expectation");
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: owner,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            blob,
            profile,
            cfg,
            crate_key: owner,
            module_path: vec![],
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("captured cast source absent")
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    (db, view)
}
fn hint(text: &str, function: &str, expression: &str, expected: Option<HintRecipe>) {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view) = source(text, edition);
        let handle = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == function)
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        let body = handle.key();
        let start = u32::try_from(text.find(expression).unwrap())
            .unwrap()
            .checked_sub(handle.range.start)
            .unwrap();
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::BodyStructure(body.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::BodyStructure(index) = fetched.value.as_ref() else {
            panic!("entered body variant")
        };
        let node = index
            .nodes()
            .iter()
            .find(|node| {
                node.range.start == start
                    && node.range.end == start + u32::try_from(expression.len()).unwrap()
            })
            .unwrap();
        let pc = index.pc(node.id, Phase::Entry).unwrap();
        let entered_span = index.span(node.id).unwrap();
        let source_revision = db.source_revision();
        let fetched = db
            .fetch(&SyntaxProvider, QueryKey::Thunk(pc.clone()))
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::Thunk(recipe) = fetched.value.as_ref() else {
            panic!("cast entry variant")
        };
        assert_eq!(recipe.recipe().key, pc);
        assert_eq!(recipe.recipe().origin.span, entered_span);
        assert_eq!(recipe.recipe().key.body, body);
        if let Some(expected) = expected.as_ref() {
            let mut hints = recipe.recipe().operations.iter().filter_map(|op| match op {
                Op::SetHint { hint, .. } => Some(hint),
                _ => None,
            });
            assert_eq!(
                hints.next(),
                Some(expected),
                "{expression} edition{edition}"
            );
            assert!(hints.next().is_none());
            assert!(
                !recipe
                    .recipe()
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::Unsupported(_)))
            );
        } else {
            assert!(recipe.recipe().operations.iter().any(|op| matches!(op,
                Op::Unsupported(diag) if diag.message == "cast target requires symbolic type resolution")));
            assert!(
                !recipe
                    .recipe()
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::SetHint { .. }))
            );
        }
        assert_eq!(db.source_revision(), source_revision);
        assert_eq!(db.trace.truncated, 0);
        for event in &db.trace.events {
            match &event.key {
                QueryKey::Thunk(key) => {
                    assert_eq!(key, &pc, "no child/successor thunk is demanded")
                }
                QueryKey::BodyStructure(key) => {
                    assert_eq!(key, &body, "no foreign/cold body query")
                }
                _ => {}
            }
        }
    }
}

#[test]
fn original_unsigned_cast_prefixes_receive_target_context_at_exact_cast_entry_all_editions() {
    for expression in ["-1 as usize", "(-1) as usize"] {
        hint(
            UNSIGNED,
            "main",
            expression,
            Some(HintRecipe::Primitive(PrimitiveType::Usize)),
        );
    }
}

#[test]
fn stock_named_casts_and_disclosed_suffixed_adapters_preserve_source_constraints_all_editions() {
    for expression in [
        "cap as u8",
        "(&42u8 >> 4) as usize",
        "(&42u8 << 4) as usize",
    ] {
        hint(FALLBACK, "main", expression, Some(HintRecipe::None));
    }
    hint(RFC_CASTS, "main", "l as usize", Some(HintRecipe::None));
    let expression = "0x8000000000000000u64 as i64";
    assert!(RFC_CASTS.contains(expression));
    hint(
        &format!("fn main(){{let value={expression};}}"),
        "main",
        expression,
        Some(HintRecipe::None),
    );
    hint(
        "fn main(){let value=(-1i32) as u32;}",
        "main",
        "(-1i32) as u32",
        Some(HintRecipe::None),
    );
}

#[test]
fn matching_literal_families_and_char_source_type_use_pinned_cast_expectation_all_editions() {
    for (expression, expected) in [
        ("4294967295 as u32", PrimitiveType::U32),
        ("18446744073709551615 as u64", PrimitiveType::U64),
        ("9223372036854775807 as i64", PrimitiveType::I64),
        ("!1 as u8", PrimitiveType::U8),
        ("1.0000000596046448 as f32", PrimitiveType::F32),
        ("-1.0 as f32", PrimitiveType::F32),
        ("0x61 as char", PrimitiveType::U8),
        ("1 as f64", PrimitiveType::I32),
        ("1.0 as u64", PrimitiveType::F64),
        ("(1+2) as u64", PrimitiveType::I32),
    ] {
        hint(
            &format!("fn main(){{let value={expression};}}"),
            "main",
            expression,
            Some(HintRecipe::Primitive(expected)),
        );
    }
}

#[test]
fn primitive_shadow_alias_and_rigid_cast_targets_remain_symbolic_without_child_queries() {
    for (text, function, expression) in [
        (
            "struct u32; fn main(){let value=1 as u32;}",
            "main",
            "1 as u32",
        ),
        (
            "mod m{pub type T=i64;} use m::T as u32; fn main(){let value=1 as u32;}",
            "main",
            "1 as u32",
        ),
        (
            "type u32=i64; fn main(){let value=-0 as u32;}",
            "main",
            "-0 as u32",
        ),
        (
            "type Target=u32; fn main(){let value=1 as Target;}",
            "main",
            "1 as Target",
        ),
        (
            "fn value<T>(){let x=1 as T;} fn main(){}",
            "value",
            "1 as T",
        ),
    ] {
        hint(text, function, expression, None);
    }
}

fn value(text: &str, expected: &str, family: &str) {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view) = source(text, edition);
        let body = view
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "value")
            .unwrap()
            .body
            .as_ref()
            .unwrap()
            .key();
        let source_revision = db.source_revision();
        db.begin_run().unwrap();
        let QueryPoll::Ready(Ok(mut machine)) =
            RunMachine::start_with_sources(&mut db, body.clone(), 100_000, vec![view])
        else {
            panic!("entered value startup incomplete")
        };
        match machine.poll(&mut db) {
            RunPoll::Completed(report) => {
                match (&report.value, family) {
                    (ValueSummary::Unsigned(actual), "unsigned")
                    | (ValueSummary::Signed(actual), "signed")
                    | (ValueSummary::F32(actual), "f32") => assert_eq!(actual, expected),
                    (ValueSummary::Char(actual), "char") => {
                        assert_eq!(actual.to_string(), expected)
                    }
                    _ => panic!("unexpected scalar family: {:?}", report.value),
                }
                assert_eq!(report.live_edition, edition);
                assert_eq!(
                    report.opcode_counters.started_by_opcode[12], 1,
                    "actual cast must execute"
                );
                assert!(report.opcode_counters.started_by_opcode[0] > 0);
                assert_eq!(
                    report.opcode_counters.operations_started,
                    report.opcode_counters.operations_completed
                );
                assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
            }
            RunPoll::Fault(error) => panic!("edition{edition} reached value fault: {error}"),
            RunPoll::NeedInput(input) => panic!("entered value suspension: {input:?}"),
            RunPoll::Panicked(report) => panic!("entered value panic: {:?}", report.panic.kind),
        }
        assert_eq!(db.trace.truncated, 0);
        for event in &db.trace.events {
            match &event.key {
                QueryKey::BodyStructure(key) => {
                    assert_eq!(key, &body, "cold main body remains unprepared")
                }
                QueryKey::Thunk(pc) => assert_eq!(pc.body, body, "no callee/future body thunk"),
                _ => {}
            }
        }
        assert_eq!(db.source_revision(), source_revision);
        db.finish_run().unwrap();
    }
}

#[test]
fn supplemental_large_unsigned_and_signed_casts_execute_actual_values_all_editions() {
    for (text, expected, family) in [
        (
            "fn value()->u32{4294967295 as u32} fn main(){}",
            "4294967295",
            "unsigned",
        ),
        (
            "fn value()->u64{18446744073709551615 as u64} fn main(){}",
            "18446744073709551615",
            "unsigned",
        ),
        (
            "fn value()->i64{9223372036854775807 as i64} fn main(){}",
            "9223372036854775807",
            "signed",
        ),
        (
            "fn value()->i64{0x8000000000000000u64 as i64} fn main(){}",
            "-9223372036854775808",
            "signed",
        ),
        (
            "fn value()->u32{(-1i32) as u32} fn main(){}",
            "4294967295",
            "unsigned",
        ),
        (
            "fn value()->u32{let signed:i32=-1;signed as u32} fn main(){}",
            "4294967295",
            "unsigned",
        ),
        (
            "fn value()->u8{let cap=512*512;cap as u8} fn main(){}",
            "0",
            "unsigned",
        ),
    ] {
        value(text, expected, family);
    }
}

#[test]
fn supplemental_cast_family_precision_and_char_source_type_execute_actual_values() {
    value(
        "fn value()->f32{1.0000000596046448 as f32} fn main(){}",
        "1.0000001",
        "f32",
    );
    value("fn value()->char{0x61 as char} fn main(){}", "a", "char");
}

#[test]
fn cast_type_namespace_cfg_and_value_names_remain_distinct_all_editions() {
    for text in [
        "#[cfg(any())] type u32=i64; fn main(){let value=1 as u32;}",
        "fn main(){let u32=3i64;let value=1 as u32;}",
        "fn value<const u32:u32>(){let value=1 as u32;} fn main(){}",
    ] {
        hint(
            text,
            if text.starts_with("fn value") {
                "value"
            } else {
                "main"
            },
            "1 as u32",
            Some(HintRecipe::Primitive(PrimitiveType::U32)),
        );
    }
}
