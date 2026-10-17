//! Original-receipt regression control. Register as a child of existing
//! lexical_inputs_tests to reuse its actual source/discovery setup before any
//! retained-entry adapter. This is metadata authority, not stock VM acceptance.
use super::*;
use crate::{
    frontend::callables::{CallSelection, EnteredLocalCall, prepare_entered_local_function},
    syntax::body_index::{ExpressionKind, NodeRole},
};
const SHADOW_ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/resolve/type-param-local-var-shadowing.rs"
);
#[test]
fn original_source_local_body_without_syntax_receipt_has_genuine_execution_lease_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source, discovery) = setup(SHADOW_ORIGINAL, edition);
        db.begin_expansion_epoch(1).unwrap();
        discovery.publish(&mut db).unwrap();
        let CallSelection::Source(main) = ready(discovery.entry_function(&mut db, "main")) else {
            panic!("actual original main selection absent")
        };
        let parent = ready(db.prepare_source_local_plan(
            &discovery,
            &[&source],
            &main.body().unwrap().key(),
        ));
        let original = parent.context().source();
        let parse = original.parsed().key();
        assert_eq!(parse, source.selection().outline.parse);
        assert!(original.original_source().is_some());
        assert!(original.generated_source().is_none());
        assert!(original.parsed().retained_syntax_identity().is_none());
        assert!(db.input(&InputKey::RetainedSyntax(parse.content)).is_none());
        assert!(
            db.input(&InputKey::GeneratedSyntax(parse.content))
                .is_none()
        );
        assert!(matches!(
            db.input(&InputKey::SourceBlob(parse.content)).as_deref(),
            Some(InputValue::SourceBlob(_))
        ));
        let index = original.index();
        let mut calls = index
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call));
        let call = calls.next().expect("actual original inner f call");
        assert!(calls.next().is_none());
        let site = index.pc(call.id, Phase::Entry).unwrap();
        let EnteredLocalCall::Local(proof) = ready(prepare_entered_local_function(&mut db, &site))
        else {
            panic!("actual original f must select the common local proof")
        };
        let child = proof.item().body.as_ref().unwrap().key();
        db.begin_run().unwrap();
        db.publish_ordinary_local_function(&proof).unwrap();
        let callee = proof.callable(&mut db).unwrap();
        let structure = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(child.clone())));
        let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
            panic!("actual original child index absent")
        };
        let entry = index.pc(NodeId(0), Phase::Entry).unwrap();
        let admitted = Arc::new(ready(source_body_context::capture_local(
            &mut db,
            Arc::clone(&parent),
            &callee,
            index,
            &entry,
        )));
        assert!(Arc::ptr_eq(admitted.local_parent().unwrap(), &parent));
        assert!(admitted.original_source().is_some());
        assert!(admitted.parsed().retained_syntax_identity().is_none());
        db.finish_run().unwrap();
        ready(db.publish_body_environment(
            Arc::clone(&admitted),
            Arc::clone(parent.environment().unwrap().macros()),
        ));
        assert!(
            matches!(db.input(&InputKey::LocalDeclarationOwner(child.definition)).as_deref(), Some(InputValue::LocalDeclarationOwner(owner)) if owner == parent.context().source().index().body())
        );
        let trace = db.trace.events.len();
        // Before the correction, actual original local receiptNone populated
        // slot5 with LocalDeclarationOwner while the original root had zero.
        // This source-kind-specific false refusal must not return.
        ready(admitted.require_execution_readiness(&mut db, &child, &parse));
        let lease = ready(db.pin_guest_body(&child));
        assert_eq!(lease.body(), &child);
        ready(lease.require_current(&mut db));
        assert!(
            db.trace.events[trace..]
                .iter()
                .all(|event| event.action != crate::db::trace::Action::ProviderRun)
        );
        assert!(
            db.trace
                .events
                .iter()
                .all(|event| !matches!(event.key, QueryKey::Thunk(_)))
        );
        assert!(
            db.input(&InputKey::RetainedSyntax(parse.content)).is_none(),
            "control never mints a retained permit"
        );
        drop(lease);
        db.require_body_unowned(child.definition).unwrap();
    }
}
