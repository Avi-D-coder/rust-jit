//! Genuine source Type controls over an unchanged official &str header. Storage,
//! call transport and comparison execution are separate contracts.
use super::*;
use crate::frontend::{names::NameLookup, signature_names::SignatureName};
const ORIGINAL_STR: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/match/match-borrowed-str.rs"
);
#[test]
fn original_str_pointee_and_external_alias_join_the_same_opaque_source_type_all_editions() {
    let alias = ORIGINAL_STR.replace("x: &str, y: &str", "x: &LrrStr, y: &LrrStr")
        + "\ntype LrrStr = str;\n";
    for edition in [2015, 2018, 2021, 2024] {
        for text in [ORIGINAL_STR, alias.as_str()] {
            let (mut db, source, graph) = prepare_source(text, edition);
            let callee = callable(&mut db, &source, &graph, "compare");
            // A `str` pointee is an unsized buffer, not a location: the
            // reference capture declines it and the parameter binds through
            // the ordinary structural route as `&str`.
            assert!(
                ClosedReferenceParameter::capture(&mut db, &callee, &graph, &[&source], 0)
                    .unwrap()
                    .is_none()
            );
            let hints = match prepare_argument_hint(&mut db, &callee, &graph, &[&source], 0) {
                QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(hints))) => hints,
                QueryPoll::Ready(Ok(other)) => {
                    panic!("structural &str parameter hint: {} ({edition})", other.describe())
                }
                QueryPoll::Ready(Err(error)) => panic!("{error} ({edition})"),
                QueryPoll::NeedInput(_) => panic!("structural &str hint needs input"),
            };
            let [hint] = hints.as_slice() else { panic!() };
            assert!(
                matches!(&hint.source, TyTerm::Ref(_, false, target) if target.as_ref() == &TyTerm::Str)
            );
            assert!(hint.closed_reference.is_none());
            let name = if text == ORIGINAL_STR {
                "str"
            } else {
                "LrrStr"
            };
            let QueryPoll::Ready(Ok(NamedTypePreparation::Ready(common))) =
                prepare_named(&mut db, &graph, &[&source], callee.signature(), name)
            else {
                panic!("actual source Str lookup failed")
            };
            assert_eq!(common.source(), &TyTerm::Str);
            let store = GuestStore::new(1, 1000, 1_000_000).unwrap();
            assert_eq!(common.find_descriptor(&store), Some(TypeId(16)));
        }
    }
}
#[test]
fn actual_type_shadow_and_pending_producer_do_not_mint_builtin_str() {
    for edition in [2015, 2018, 2021, 2024] {
        let shadow = format!("{ORIGINAL_STR}\ntype str = u8;\n");
        let (mut db, source, graph) = prepare_source(&shadow, edition);
        let callee = callable(&mut db, &source, &graph, "compare");
        let proof =
            ClosedReferenceParameter::capture(&mut db, &callee, &graph, &[&source], 0).unwrap();
        // The existing primitive-alias integration may remain indeterminate;
        // it must never reinterpret a real Type definition as builtin str.
        if let Some(proof) = proof {
            assert_ne!(proof.pointee_source(), &TyTerm::Str);
        }
        let QueryPoll::Ready(Ok(prepared)) =
            prepare_named(&mut db, &graph, &[&source], callee.signature(), "str")
        else {
            panic!("shadow Type source demand failed")
        };
        match prepared {
            NamedTypePreparation::Ready(proof) => assert_ne!(proof.source(), &TyTerm::Str),
            NamedTypePreparation::Alias(_) => {}
            _ => panic!("real Type shadow became absent or unresolved"),
        }
        let pending = format!("{ORIGINAL_STR}\nlrr_pending_types!();\n");
        let (mut db, source, graph) = prepare_source(&pending, edition);
        let selected = source
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "compare")
            .unwrap()
            .def_key
            .fingerprint();
        let outcome = db
            .fetch(
                &crate::db::provider::SyntaxProvider,
                QueryKey::FunctionSignature(selected),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let crate::db::value::QueryValue::FunctionSignature(signature) = outcome.value.as_ref()
        else {
            panic!()
        };
        match prepare_named(&mut db, &graph, &[&source], signature, "str") {
            QueryPoll::NeedInput(_) => {}
            QueryPoll::Ready(Ok(NamedTypePreparation::Name(SignatureName::Unresolved(
                NameLookup::NeedInput(_),
            )))) => {}
            _ => panic!("pending actual producer must retain Type completion authority"),
        }
    }
}
