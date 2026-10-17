//! Cold original macro declaration facets do not certify selected panic behavior.
use super::*;
use crate::db::dependency::{InputKey, InputValue};
use crate::facade::macros::RuntimeMacroKind;
use crate::frontend::{
    constant_macro_admission::CurrentConstantMacroProof, runtime_macro_catalog::RuntimeMacroCatalog,
};
#[test]
fn unchanged_cold_unreachable_and_unimplemented_have_distinct_declaration_proofs() {
    for edition in [2015, 2018, 2021, 2024] {
        for (name, kind) in [
            ("Y", RuntimeMacroKind::Unreachable),
            ("X", RuntimeMacroKind::Unimplemented),
            ("Y_CORE", RuntimeMacroKind::Unreachable),
            ("X_CORE", RuntimeMacroKind::Unimplemented),
        ] {
            let (db, view, definition) = world(PANIC, name, edition);
            let before = db.trace.counts.clone();
            let admission =
                source_admit(&db, &view, definition, ConstantRequest::Constant).unwrap();
            let handle = admission.plan().owner_body().unwrap();
            let facts = view.entered_macro_facts(&handle.key()).unwrap();
            let invocation = facts
                .invocations
                .iter()
                .find(|i| handle.range.contains(i.range))
                .unwrap();
            let published = db
                .source_view_scope_certificate(
                    view.selection(),
                    &admission.plan().owner_context().scope,
                )
                .unwrap();
            let InputValue::DiscoverySeal(input) = published.as_ref() else {
                panic!("genuine source certificate required")
            };
            let no_item = input
                .runtime_no_item_proofs
                .iter()
                .find(|(_, p)| p.body() == &handle.key() && p.invocation() == invocation.range)
                .map(|(_, p)| p)
                .expect("actual cold declaration proof");
            let catalog =
                RuntimeMacroCatalog::standard(&view.selection().outline.parse.profile).unwrap();
            assert_eq!(catalog.select(no_item.definition()).unwrap(), Some(kind));
            assert_ne!(
                catalog.select(no_item.definition()).unwrap(),
                Some(RuntimeMacroKind::Panic)
            );
            assert!(db.published_input(&InputKey::ExpansionEpoch).is_some());
            let error = CurrentConstantMacroProof::capture_current(
                &db,
                &view,
                &admission,
                &handle.key(),
                invocation.range,
                usize::MAX,
            )
            .unwrap_err();
            assert_eq!(error.code, "LRR-E-CONST-MACRO-SOURCE");
            assert_eq!(db.trace.counts, before);
        }
    }
}
