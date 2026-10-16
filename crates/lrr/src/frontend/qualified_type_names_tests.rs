//! Common source/query route controls using the complete unchanged original.
//! This prepares qualified Type authority, not pattern execution/full main.
use super::*;
use crate::frontend::{
    names::reads::NamespaceRead, qualified_type_names::QualifiedTypeObservation,
    source_body_context::type_occurrences::BodyTypeSegment,
};
const QUALIFIED: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs/module-qualified-struct-destructure.rs"
);
fn observation(db: &mut Database, key: &crate::ir::pc::PcKey) -> Arc<QueryValue> {
    ready(db.fetch(&SyntaxProvider, QueryKey::QualifiedBodyType(key.clone()))).value
}
#[test]
fn unchanged_qualified_type_uses_shared_kernel_certified_read_footprint_and_declaration_all_editions()
 {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source, discovery) = world(QUALIFIED, edition);
        let main = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap();
        let plan = ready(db.prepare_source_local_plan(
            &discovery,
            &[&source],
            &main.body.as_ref().unwrap().key(),
        ));
        let mut count = 0;
        for node in plan.context().source().index().nodes() {
            let Some(occurrence) = ready(plan.context().body_type_occurrence(&mut db, node.id))
            else {
                continue;
            };
            let [BodyTypeSegment::Name(first), BodyTypeSegment::Name(last)] = occurrence.segments()
            else {
                continue;
            };
            if first.name() != "m" || last.name() != "S" {
                continue;
            }
            let key = plan
                .context()
                .source()
                .index()
                .pc(node.id, Phase::Entry)
                .unwrap();
            let from = db.trace.events.len();
            let first_answer = observation(&mut db, &key);
            let second = observation(&mut db, &key);
            assert!(
                Arc::ptr_eq(&first_answer, &second),
                "actual immutable query result reused"
            );
            let QueryValue::QualifiedBodyType(answer) = first_answer.as_ref() else {
                panic!("qualified Type query variant")
            };
            assert_eq!(answer.key(), &key);
            assert_eq!(
                answer.demand(),
                None,
                "unchanged qualified source: {answer:?}"
            );
            assert!(matches!(answer.lookup(), Some(NameLookup::Defined(_))));
            assert!(Arc::ptr_eq(answer.retained_plan(), &plan));
            assert_eq!(
                answer.fingerprint(),
                StableEncode::fingerprint(answer.as_ref())
            );
            assert!(answer.estimated_bytes() >= std::mem::size_of::<QualifiedTypeObservation>());
            let environment = plan.environment().unwrap().macros().source_environment();
            let (root, _) = plan.entered.local_hygiene();
            let context = plan
                .context()
                .source()
                .identifier_context_or_original(first.range(), root)
                .unwrap();
            let parts = [
                crate::syntax::imports::ImportSegment::Name(first.name().into()),
                crate::syntax::imports::ImportSegment::Name(last.name().into()),
            ];
            let site = || crate::frontend::source_locals::OrdinaryNameSite {
                source: plan.entered.parent.source(),
                body: occurrence.body(),
                at: u64::from(first.range().start),
                name: first.name(),
                context,
                namespace: Namespace::Type,
            };
            let plain = plan
                .adapter()
                .lookup_qualified_module(site(), &parts, false, environment.names())
                .unwrap();
            let (observed, reads) = plan
                .adapter()
                .lookup_qualified_module_observed(site(), &parts, false, environment.names())
                .unwrap();
            assert_eq!(plain, observed, "one resolver with optional observation");
            assert_eq!(answer.lookup(), Some(&observed));
            assert!(reads.rows().any(|row| matches!(row, NamespaceRead::Name { namespace: Namespace::Type, name, .. } if name == "m")));
            assert!(reads.rows().any(|row| matches!(row, NamespaceRead::Name { namespace: Namespace::Type, name, .. } if name == "S")));
            assert!(reads.rows().any(|row| matches!(row, NamespaceRead::Scope { namespace: Namespace::Type, scope } if scope == &source.outline().root_scope)));
            // A resolver demand is not an opaque use-site proof. Observe an
            // absent tail through the same actual entered source/caller to
            // verify that negative member reads are retained by the kernel.
            let missing = [
                parts[0].clone(),
                crate::syntax::imports::ImportSegment::Name("missing_type_member".into()),
            ];
            let absent = plan
                .adapter()
                .lookup_qualified_module(site(), &missing, false, environment.names())
                .unwrap();
            let (observed_absent, absence_reads) = plan
                .adapter()
                .lookup_qualified_module_observed(site(), &missing, false, environment.names())
                .unwrap();
            assert_eq!(absent, observed_absent);
            assert!(matches!(absent, NameLookup::Absent));
            assert!(absence_reads.rows().any(|row| matches!(row,
                NamespaceRead::Name { namespace: Namespace::Type, name, .. } if name == "missing_type_member")));
            let source_type = ready_type(plan.prepare_body_type(&mut db, node.id));
            let expected = source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "S")
                .unwrap();
            assert_eq!(source_type.declaration().definition(), &expected.def_key);
            assert_eq!(
                source_type.source_type().source(),
                &TyTerm::Nominal(expected.def_key.fingerprint(), vec![])
            );
            assert!(
                db.memo(&QueryKey::QualifiedBodyType(key.clone()))
                    .unwrap()
                    .dependencies
                    .iter()
                    .any(|stamp| matches!(
                        stamp,
                        DependencyStamp::Query {
                            key: QueryKey::CurrentModuleName(_),
                            ..
                        }
                    ))
            );
            assert!(!db.trace.events[from..].iter().any(|event| matches!(
                event.key,
                QueryKey::Thunk(_) | QueryKey::Constant(_) | QueryKey::FunctionSignature(_)
            )));
            let mut wrong = key.clone();
            wrong.phase = Phase::Commit.code().unwrap();
            assert!(matches!(
                db.fetch(&SyntaxProvider, QueryKey::QualifiedBodyType(wrong)),
                QueryPoll::Ready(Err(_))
            ));
            db.begin_expansion_epoch(2).unwrap();
            assert!(matches!(
                db.fetch(&SyntaxProvider, QueryKey::QualifiedBodyType(key)),
                QueryPoll::Ready(Err(_))
            ));
            count += 1;
            break; // epoch revocation belongs to this actual selected occurrence
        }
        assert_eq!(
            count, 1,
            "whole original has an actual m::S constructor occurrence"
        );
    }
}
