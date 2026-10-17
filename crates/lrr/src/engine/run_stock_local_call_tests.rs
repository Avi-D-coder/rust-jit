//! Whole official root, ordinary zero-argument local call and cold generic bodies.
//! Runtime-contract controls only; no native or full compiletest acceptance.
use super::*;
use ra_ap_syntax::{AstNode, ast};

const SHADOWING: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/resolve/type-param-local-var-shadowing.rs"
);

// Keep real crate attributes, including inner documentation, at crate position.
// The exact predecessor adapter was rejected by rustc with E0753 because it
// emitted inner documentation as items; this changes only that LRR adapter.
// Every remaining source
// byte, including the cold Foo/Box impl and original main, enters the emitter.
fn emitted_shadowing(edition: u16, wrapper: bool) -> String {
    let edition = match edition {
        2015 => ra_ap_syntax::Edition::Edition2015,
        2018 => ra_ap_syntax::Edition::Edition2018,
        2021 => ra_ap_syntax::Edition::Edition2021,
        2024 => ra_ap_syntax::Edition::Edition2024,
        _ => panic!("unsupported stock local-call edition"),
    };
    let parsed = ra_ap_syntax::SourceFile::parse(SHADOWING, edition);
    assert!(parsed.errors().is_empty());
    let mut attributes = String::new();
    let mut items = String::new();
    let mut previous = 0;
    // The pinned frontend represents documentation as DocComment nodes,
    // exposed alongside ordinary attributes by AnyAttr (not direct tokens).
    for attribute in parsed
        .tree()
        .syntax()
        .children()
        .filter_map(ast::AnyAttr::cast)
        .filter(|attribute| attribute.kind() == ast::AttrKind::Inner)
    {
        let span = crate::syntax::ast_adapter::range(attribute.syntax().text_range());
        let start = span.start as usize;
        let end = span.end as usize;
        items.push_str(&SHADOWING[previous..start]);
        attributes.push_str(&SHADOWING[start..end]);
        attributes.push('\n');
        previous = end;
    }
    items.push_str(&SHADOWING[previous..]);
    format!(
        "{attributes}macro_rules! emit{{()=>{{{items}}}}}{}",
        if wrapper {
            "macro_rules! wrap{()=>{emit!();}}wrap!();"
        } else {
            "emit!();"
        }
    )
}

fn assert_shadowing_main(text: &str, edition: u16, generated: bool) {
    let mut h = start(text, edition, "main");
    let main = h.run.frame.body.body().clone();
    let parent_identity = h.parent.source_identity();
    let source = h.parent.context().source();
    assert_eq!(source.body().key(), main);
    assert_eq!(source.index().body(), &main);
    assert_eq!(source.parsed().key().profile.edition, edition);
    assert_eq!(h.run.frame.body.profile().edition, edition);
    assert_eq!(source.generated_source().is_some(), generated);
    assert_eq!(source.generated_boundary().is_some(), generated);
    let parent_parse = source.parsed().key();
    let children = child_bodies(&h);
    assert_eq!(
        children.len(),
        1,
        "actual main contains only the inner f body"
    );
    let child = children[0].clone();
    let rows = h
        .facts
        .items
        .iter()
        .filter(|row| {
            row.item
                .body
                .as_ref()
                .is_some_and(|body| body.key() == child)
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].item.def_key.declared_name, "f");
    let selected_header = rows[0].item.header.clone();
    let selected_scope = rows[0].scope;
    let call_nodes = h
        .run
        .frame
        .body
        .nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
        .collect::<Vec<_>>();
    assert_eq!(
        call_nodes.len(),
        1,
        "full original main must keep the one actual f() call"
    );
    let call_site = h.run.frame.body.pc(call_nodes[0].id, Phase::Entry).unwrap();
    assert_children_cold(&h);
    assert!(
        h.db.published_input(&InputKey::DefinitionBody(child.definition))
            .is_none()
    );
    assert!(h.db.memo(&QueryKey::BodyStructure(child.clone())).is_none());
    let source_revision = h.db.source_revision();
    let report = completed(&mut h);
    assert!(matches!(report.value, ValueSummary::Unit));
    let counts = &report.opcode_counters;
    assert_eq!(counts.started_by_opcode[19], 1); // one ordinary CallSeal
    // The ordinary block lowerer emits CheckParameters only for nonzero arity.
    // Both actual main and f have zero parameters; neither is a surrogate body.
    assert_eq!(counts.started_by_opcode[20], 0);
    assert_eq!(counts.started_by_opcode[21], 0); // no ReadParameter
    assert_eq!(counts.started_by_opcode[22], 0); // no argument CallPrepare
    assert_eq!(counts.started_by_opcode[23], 0); // no argument CallAdvance
    assert_eq!(counts.started_by_opcode[26], 0); // no legacy Assert
    assert_eq!(counts.operations_started, counts.operations_completed);
    assert!(counts.return_scopes_cleaned >= 2);
    assert_eq!(counts.panic_scopes_cleaned, 0);
    assert_eq!(h.db.source_revision(), source_revision);
    assert_eq!(h.run.peak_guest_frames, 2);
    assert!(h.run.calls.is_empty() && h.run.pending.is_empty());
    assert_eq!(h.run.call_bytes, 0);
    assert_eq!(h.run.pending_bytes, 0);
    assert!(h.run.prepared_call.is_none());
    assert!(h.run.returning.is_none() && h.run.unwinding.is_none());
    assert!(h.run.frame.active_scopes().is_empty());
    assert!(h.run.terminal);

    let actual =
        h.db.input(&InputKey::BodyEnvironment(child.clone()))
            .expect("actually entered inner f owns a published environment");
    let InputValue::BodyEnvironment(environment) = actual.as_ref() else {
        panic!("actual child environment variant differs")
    };
    assert_eq!(environment.body(), &child);
    assert_eq!(environment.source().body().key(), child);
    assert_eq!(environment.source().header(), &selected_header);
    assert_eq!(environment.source().parsed().key(), parent_parse);
    assert_eq!(
        environment
            .source()
            .local_parent()
            .unwrap()
            .source_identity(),
        parent_identity
    );
    let prepared = ready(h.db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(child.clone())));
    let QueryValue::SourceLocalPlan(plan) = prepared.value.as_ref() else {
        panic!("actual child source plan variant differs")
    };
    assert_eq!(
        plan.environment().unwrap().identity(),
        environment.identity()
    );
    assert!(Arc::ptr_eq(
        plan.context().source().index(),
        environment.source().index()
    ));
    let selected = ready(h.db.fetch(&SyntaxProvider, QueryKey::EnteredCall(call_site.clone())));
    let QueryValue::EnteredCall(goal) = selected.value.as_ref() else {
        panic!("actual local call goal variant differs")
    };
    goal.require_current(&h.db).unwrap();
    let crate::frontend::entered_call_goals::EnteredCallSelection::Local(proof) = goal.selection()
    else {
        panic!("inner item call resolved as an outer binding or module function")
    };
    assert_eq!(proof.item().def_key.fingerprint(), child.definition);
    assert_eq!(proof.item().body.as_ref().unwrap().key(), child);
    assert_eq!(proof.parent().source_identity(), parent_identity);
    assert_eq!(proof.site(), &call_site);
    assert_eq!(proof.scope(), selected_scope);

    assert!(
        h.db.trace.events.iter().any(|event| matches!(&event.key,
            QueryKey::Thunk(pc) if pc.body == child && event.action == Action::ProviderRun
        )),
        "actual selected inner f body must enter the ordinary child thunk route"
    );

    for event in &h.db.trace.events {
        match &event.key {
            QueryKey::BodyStructure(body) => assert!(
                body == &main || body == &child,
                "cold generic Foo/Box impl or another body was prepared: {body:?}"
            ),
            QueryKey::Thunk(pc) => assert!(
                pc.body == main || pc.body == child,
                "cold generic Foo/Box impl or another body was lowered: {pc:?}"
            ),
            _ => {}
        }
    }
    for key in [
        QueryKey::BodyStructure(child.clone()),
        QueryKey::SourceLocalPlan(child),
    ] {
        assert_eq!(
            h.db.trace
                .events
                .iter()
                .filter(|event| { event.key == key && event.action == Action::ProviderRun })
                .count(),
            1,
            "actual child preparation must be retained: {key:?}"
        );
    }
    assert_eq!(
        h.db.trace
            .events
            .iter()
            .filter(|event| {
                event.key.kind() == "EnteredCall" && event.action == Action::ProviderRun
            })
            .count(),
        1,
        "actual local item must use the common entered call goal once"
    );
    h.db.finish_run().unwrap();
}

#[test]
fn unchanged_shadowing_main_calls_inner_item_and_keeps_generic_impl_cold_all_editions() {
    for edition in EDITIONS {
        assert_shadowing_main(SHADOWING, edition, false);
    }
}

#[test]
fn upstream_shadowing_user_macro_uses_ordinary_local_call_all_editions() {
    for edition in EDITIONS {
        assert_shadowing_main(&emitted_shadowing(edition, false), edition, true);
    }
}

#[test]
fn upstream_shadowing_wrapper_macro_uses_ordinary_local_call_all_editions() {
    for edition in EDITIONS {
        assert_shadowing_main(&emitted_shadowing(edition, true), edition, true);
    }
}
