//! Cold source controls registered under declaration_field_type_tests. The
//! unchanged complete official program is not executed by these observations.
use super::*;
use crate::{
    frontend::source_body_context::type_occurrences::{
        BodyTypeOrigin, BodyTypeSegment, BodyTypeUse,
    },
    ir::pc::NodeId,
};
const AUTO: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/auto-traits/auto-is-contextual.rs"
);
#[test]
fn unchanged_auto_original_type_survives_retained_macro_output_in_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for retained in [false, true] {
            let (mut db, plan) = world(AUTO, edition, retained);
            let fingerprint = plan.context().body_type_occurrences().fingerprint();
            let start = db.trace.events.len();
            let mut count = 0;
            for node in plan.context().source().index().nodes() {
                let Some(occurrence) = ready(plan.context().body_type_occurrence(&mut db, node.id))
                else {
                    continue;
                };
                if occurrence.usage() != BodyTypeUse::TypeSyntax {
                    continue;
                }
                let [BodyTypeSegment::Name(name)] = occurrence.segments() else {
                    continue;
                };
                if name.name() != "u8" {
                    continue;
                }
                assert_eq!(name.edition(), edition);
                assert_eq!(occurrence.origin(), BodyTypeOrigin::Original);
                if retained {
                    assert_eq!(
                        std::ops::Deref::deref(&occurrence).origin(),
                        BodyTypeOrigin::NeedRetainedOccurrence,
                        "cold observation mutated the immutable syntax row"
                    );
                }
                let again = ready(plan.context().body_type_occurrence(&mut db, node.id)).unwrap();
                assert_eq!(again.path_range(), occurrence.path_range());
                assert_eq!(again.origin(), occurrence.origin());
                count += 1;
            }
            assert_eq!(count, 1, "unchanged official cast Type occurrence missing");
            assert_eq!(
                plan.context().body_type_occurrences().fingerprint(),
                fingerprint
            );
            assert!(
                !db.trace.events[start..].iter().any(|event| matches!(
                    &event.key,
                    QueryKey::Thunk(_)
                        | QueryKey::BodyStructure(_)
                        | QueryKey::FunctionSignature(_)
                        | QueryKey::TypeSchema(_)
                        | QueryKey::Constant(_)
                )),
                "source occurrence proof prepared executable or type/member authority"
            );
        }
    }
}
#[test]
fn retained_type_occurrence_cannot_refresh_after_real_epoch_change_or_use_foreign_node() {
    let (mut db, plan) = world(AUTO, 2024, true);
    let mut selected = None;
    for node in plan.context().source().index().nodes() {
        let Some(occurrence) = ready(plan.context().body_type_occurrence(&mut db, node.id)) else {
            continue;
        };
        if matches!(occurrence.segments(), [BodyTypeSegment::Name(name)] if name.name() == "u8") {
            selected = Some(node.id);
            assert_eq!(occurrence.origin(), BodyTypeOrigin::Original);
        }
    }
    let node = selected.unwrap();
    match plan
        .context()
        .body_type_occurrence(&mut db, NodeId(u32::MAX))
    {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-TYPE-SCHEMA"),
        other => panic!("foreign Type occurrence node accepted: {other:?}"),
    }
    db.begin_expansion_epoch(2).unwrap();
    match plan.context().body_type_occurrence(&mut db, node) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.code, "LRR-E-EXPANSION-INPUT"),
        other => panic!("stale retained Type occurrence accepted: {other:?}"),
    }
}
