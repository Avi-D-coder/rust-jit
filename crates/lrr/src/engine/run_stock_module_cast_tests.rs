//! Full unchanged official module-callee/cast runtime witness. No selected body
//! replacement, extra owning-plan observer or accepted unsupported outcome.
use super::*;
use crate::syntax::{ast_adapter::range, spans::SourceRange};
use ra_ap_syntax::{AstNode, ast};
use std::collections::BTreeSet;

const SHIFT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/shift.rs"
);
fn runs(h: &Harness, key: &QueryKey) -> usize {
    h.db.trace
        .events
        .iter()
        .filter(|event| event.key == *key && event.action == Action::ProviderRun)
        .count()
}
fn whole_shift_main(edition: u16) {
    // This invokes the existing whole-source discovery, real entry selection
    // and retained-entry preparation. Required core/macro gaps fail normally.
    let mut h = start(SHIFT, edition, "main");
    let main = h.run.frame.body.body().clone();
    let calls = h
        .run
        .frame
        .body
        .nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
        .map(|node| h.run.frame.body.pc(node.id, Phase::Entry).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        3,
        "unchanged main calls all three actual module functions"
    );
    assert_eq!(h.run.opcode_counters().unwrap().operations_started, 0);
    let module = h
        .parent
        .context()
        .source()
        .module_source()
        .expect("actual module source lease");
    let definitions = ["test_misc", "test_expr", "test_const"].map(|name| {
        let mut items = module.outline().items.iter().filter(|item| {
            item.def_key.declared_name == name
                && item.def_key.item_kind == crate::source::manifest::ItemKind::Function
        });
        let item = items.next().expect("unchanged module function absent");
        assert!(
            items.next().is_none(),
            "unchanged module function ambiguous"
        );
        (name, item.def_key.fingerprint())
    });
    assert!(
        h.db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) => body == &main,
            QueryKey::Thunk(pc) => pc.body == main,
            _ => true,
        }),
        "all actual module callee executable bodies stay cold before their reached call"
    );

    let report = completed(&mut h);
    assert!(matches!(report.value, ValueSummary::Unit));
    let counts = &report.opcode_counters;
    assert_eq!(
        counts.started_by_opcode[19], 3,
        "three ordinary zero-argument CallSeal operations"
    );
    assert_eq!(
        counts.started_by_opcode[26], 0,
        "assertions require common expanded source, never legacy Assert"
    );
    assert_eq!(counts.operations_started, counts.operations_completed);
    assert!(counts.return_scopes_cleaned >= 4);
    assert_eq!(counts.panic_scopes_cleaned, 0);
    assert_eq!(h.run.peak_guest_frames, 2);
    assert!(h.run.calls.is_empty() && h.run.pending.is_empty());
    assert!(h.run.frame.active_scopes().is_empty());
    assert!(h.run.terminal);
    for site in calls {
        assert_eq!(
            runs(&h, &QueryKey::EnteredCall(site)),
            1,
            "actual main call selection must run once"
        );
    }

    let mut children = BTreeSet::new();
    for (name, definition) in definitions {
        let source =
            h.db.input(&InputKey::DefinitionSource(definition))
                .expect("actual reached module source publication");
        let InputValue::DefinitionSource(source) = source.as_ref() else {
            panic!("module source variant differs")
        };
        let locator =
            h.db.input(&InputKey::DefinitionBody(definition))
                .expect("actual reached module body publication");
        let InputValue::DefinitionBody(locator) = locator.as_ref() else {
            panic!("module body variant differs")
        };
        let child = locator.key();
        assert!(
            children.insert(child.clone()),
            "three distinct actual source functions"
        );
        // Establish reached execution BEFORE observer queries can populate any
        // metadata. No unused SourceLocalPlan demand is added by this test.
        assert_eq!(runs(&h, &QueryKey::BodyStructure(child.clone())), 1);
        assert!(h.db.trace.events.iter().any(|event| matches!(&event.key,
            QueryKey::Thunk(pc) if pc.body == child && event.action == Action::ProviderRun)));
        let structure = ready(h.db.fetch(&SyntaxProvider, QueryKey::BodyStructure(child.clone())));
        let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
            panic!("actual child index variant differs")
        };
        assert_eq!(index.body(), &child);
        assert_eq!(index.profile().edition, edition);
        let parsed = ready(h.db.fetch(&SyntaxProvider, QueryKey::ParseFile(source.clone())));
        let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
            panic!("actual child parse variant differs")
        };
        assert_eq!(parsed.key(), *source);
        let block = parsed
            .syntax_at(locator.range, |node| ast::BlockExpr::can_cast(node.kind()))
            .unwrap()
            .expect("actual callee source block");
        if name == "test_expr" {
            let mut casts = 0usize;
            for cast in block.descendants().filter_map(ast::CastExpr::cast) {
                let target = cast.ty().expect("actual explicit cast Type");
                let absolute = range(target.syntax().text_range());
                let relative = SourceRange::checked(
                    absolute.start.checked_sub(locator.range.start).unwrap(),
                    absolute.end.checked_sub(locator.range.start).unwrap(),
                )
                .unwrap();
                let node = index
                    .nodes_at_range(relative)
                    .find(|node| node.role == NodeRole::Syntax)
                    .expect("actual cast target canonical indexed Syntax node");
                let site = index.pc(node.id, Phase::Entry).unwrap();
                let goal_key = QueryKey::EnteredType(site.clone());
                let plan_key = QueryKey::PreparedTypePlan(site.clone());
                assert_eq!(
                    runs(&h, &goal_key),
                    1,
                    "reached cast target must use genuine caller Type goal"
                );
                assert_eq!(
                    runs(&h, &plan_key),
                    1,
                    "reached cast target must retain its actual Type plan"
                );
                let prepared = ready(h.db.fetch(&SyntaxProvider, plan_key));
                let QueryValue::PreparedTypePlan(prepared) = prepared.value.as_ref() else {
                    panic!("actual cast Type plan variant differs")
                };
                prepared.guard().require_current(&h.db).unwrap();
                let ty = prepared.goal().source_for(index, &site, absolute).unwrap();
                assert!(
                    ty.primitive().is_some(),
                    "unchanged written cast target resolves through source Type authority"
                );
                casts += 1;
            }
            assert!(
                casts >= 15,
                "all fifteen direct let initializer casts remain actual callee syntax"
            );
        }
    }
    assert_eq!(children.len(), 3);
    h.db.finish_run().unwrap();
}
#[test]
fn unchanged_shift_whole_main_module_casts_2015() {
    whole_shift_main(2015);
}
#[test]
fn unchanged_shift_whole_main_module_casts_all_editions() {
    for edition in EDITIONS {
        whole_shift_main(edition);
    }
}
