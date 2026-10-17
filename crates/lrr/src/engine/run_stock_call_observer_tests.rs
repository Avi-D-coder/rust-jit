//! Whole unchanged same-target nested-call runtime witness. No selected-body
//! replacement, accepted unsupported result or native execution claim.
use super::*;
use std::collections::BTreeSet;

const COMPOSITION: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/generics/issue-94923.rs");

fn whole_composition_main(edition: u16) {
    let path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/upstream/rustc-checkout/tests/ui/generics/issue-94923.rs"
    ));
    assert_eq!(std::fs::read(path).unwrap(), COMPOSITION.as_bytes());
    let mut h = start(COMPOSITION, edition, "main");
    // The original executes 256 * 1000 loop iterations. Set one bounded run
    // budget before its first operation; nested frames share this same fuel.
    h.run.frame.set_fuel_limit(64_000_000).unwrap();
    h.run.enable_runtime_capture().unwrap();
    let main = h.run.frame.body.body().clone();
    assert_eq!(h.parent.context().source().body().key(), main);
    assert_eq!(h.parent.context().source().index().body(), &main);
    assert_eq!(h.run.frame.body.profile().edition, edition);
    assert_eq!(h.run.opcode_counters().unwrap().operations_started, 0);
    let definitions = {
        let module = h
            .parent
            .context()
            .source()
            .module_source()
            .expect("actual original module source lease");
        assert_eq!(module.selection().outline.parse.profile.edition, edition);
        (0..=8)
            .map(|ordinal| {
                let name = format!("f{ordinal}");
                let mut items = module.outline().items.iter().filter(|item| {
                    item.def_key.declared_name == name
                        && item.def_key.item_kind == crate::source::manifest::ItemKind::Function
                });
                let item = items.next().expect("actual module function absent");
                assert!(items.next().is_none(), "actual module function ambiguous");
                assert!(item.body.is_some());
                (name, item.def_key.fingerprint())
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(definitions.len(), 9);
    assert!(
        h.db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) => body == &main,
            QueryKey::Thunk(pc) => pc.body == main,
            _ => true,
        }),
        "all module executable bodies must stay cold before the first reached call"
    );

    // The actual upstream main computes y and compares it with 348 using its
    // own assert_eq!. A fault, panic or missing input fails through completed.
    let report = completed(&mut h);
    assert!(matches!(report.value, ValueSummary::Unit));
    let counts = &report.opcode_counters;
    // f8 is called once, f7 twice, ... f0 256 times: 511 explicit source calls.
    // Each has one runtime argument, including calls selected before evaluating
    // an inner call to the very same canonical function with different Types.
    // Ordinary iterator operations may contribute further source calls. Do not
    // constrain their implementation or mistake aggregate counts for a proof
    // that every individual argument effect ran once.
    assert!(counts.started_by_opcode[19] >= 511); // CallSeal
    assert!(counts.started_by_opcode[20] >= 511); // CheckParameters
    assert!(counts.started_by_opcode[22] >= 511); // CallPrepare
    assert!(counts.started_by_opcode[23] >= 511); // CallAdvance
    assert_eq!(
        counts.started_by_opcode[26], 0,
        "the upstream result assertion must use ordinary expanded source"
    );
    assert_eq!(counts.operations_started, counts.operations_completed);
    assert!(counts.return_scopes_cleaned >= 512);
    assert_eq!(counts.panic_scopes_cleaned, 0);
    assert!(h.run.peak_guest_frames >= 10);
    assert!(h.run.calls.is_empty() && h.run.pending.is_empty());
    assert_eq!(h.run.call_bytes, 0);
    assert_eq!(h.run.pending_bytes, 0);
    assert!(h.run.prepared_call.is_none());
    assert!(h.run.returning.is_none() && h.run.unwinding.is_none());
    assert!(h.run.frame.active_scopes().is_empty());
    assert!(h.run.terminal);
    assert!(h.run.runtime_capture_facts().unwrap().complete());
    assert_eq!(h.db.trace.truncated, 0);

    let mut reached = BTreeSet::new();
    for (name, definition) in definitions {
        let locator =
            h.db.input(&InputKey::DefinitionBody(definition))
                .expect("actually reached module body publication");
        let InputValue::DefinitionBody(locator) = locator.as_ref() else {
            panic!("module body publication variant differs")
        };
        assert_eq!(locator.def_key, definition);
        let body = locator.key();
        assert!(
            reached.insert(body.clone()),
            "distinct canonical {name} body"
        );
        // These checks precede any observer query that could prepare a body.
        assert!(
            h.db.trace.events.iter().any(|event| {
                event.key == QueryKey::BodyStructure(body.clone())
                    && event.action == Action::ProviderRun
            }),
            "actual {name} body was never structurally entered"
        );
        assert!(
            h.db.trace.events.iter().any(|event| {
                matches!(&event.key, QueryKey::Thunk(pc)
                if pc.body == body && event.action == Action::ProviderRun)
            }),
            "actual {name} body never used ordinary executable thunks"
        );
    }
    assert_eq!(reached.len(), 9);
    h.db.finish_run().unwrap();
    assert_eq!(std::fs::read(path).unwrap(), COMPOSITION.as_bytes());
}

#[test]
fn unchanged_composition_whole_main_same_target_arguments_2015() {
    whole_composition_main(2015);
}

// The sole validator runs this selector only after the 2015 baseline passes.
#[test]
fn unchanged_composition_whole_main_same_target_arguments_later_editions() {
    for edition in [2018, 2021, 2024] {
        whole_composition_main(edition);
    }
}
