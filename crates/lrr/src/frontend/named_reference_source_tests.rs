//! Unchanged named-pointee headers and LRR source-authority controls.
use super::tests::{callable, source as prepare_source};
use super::*;
use crate::{
    frontend::source_types::{NamedTypePreparation, prepare_named},
    source::manifest::Namespace,
};
const POINT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-borrow-scope.rs"
);
#[test]
fn unchanged_point_reference_and_ordinary_parameter_share_the_selected_source_type() {
    for edition in [2015, 2018, 2021, 2024] {
        let text = format!("{POINT}\nfn lrr_by_value(p:Point){{}}");
        let (mut db, source, graph) = prepare_source(&text, edition);
        let callee = callable(&mut db, &source, &graph, "x_coord");
        db.trace.detailed = true;
        let proof = ClosedReferenceParameter::capture(&mut db, &callee, &graph, &[&source], 0)
            .unwrap()
            .unwrap();
        let point = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "Point")
            .unwrap()
            .def_key
            .fingerprint();
        assert_eq!(proof.pointee_source(), &TyTerm::Nominal(point, vec![]));
        let ordinary = callable(&mut db, &source, &graph, "lrr_by_value");
        let QueryPoll::Ready(Ok(NamedTypePreparation::Ready(common))) =
            prepare_named(&mut db, &graph, &[&source], ordinary.signature(), "Point")
        else {
            panic!("ordinary common source Type demand failed")
        };
        assert!(common.same_type(&proof.pointee));
        let QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(hints))) =
            prepare_argument_hints(&mut db, &ordinary, &graph, &[&source])
        else {
            panic!()
        };
        assert_eq!(hints[0].source, *proof.pointee_source());
        assert!(hints[0].descriptor.is_none());
        // A public nominal spelling + arbitrary descriptor grants no by-value
        // runtime admission. Its opaque packet transport is still required.
        let QueryPoll::Ready(Ok(ContractPreparation::NeedType { .. })) = prepare_contracts(
            &mut db,
            &ordinary,
            &graph,
            &[&source],
            &[],
            None,
            &[ArgumentType {
                source: common.source().clone(),
                descriptor: TypeId(2),
            }],
        ) else {
            panic!("unproved by-value nominal descriptor was admitted")
        };
        assert!(
            !db.trace
                .events
                .iter()
                .any(|event| matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
        );
        assert_eq!(db.trace.counts.get("TypeSchema:ProviderRun"), Some(&1));
        let mut store = GuestStore::new(1, 10_000, 4_000_000).unwrap();
        let reference = proof.materialize_hint(&mut store).unwrap();
        let TypeKey::Ref { target, .. } = store.type_descriptor(reference).unwrap().key.as_ref()
        else {
            panic!()
        };
        common.require_descriptor(&store, *target).unwrap();
        let TypeKey::Nominal {
            definition, schema, ..
        } = store.type_descriptor(*target).unwrap().key.as_ref()
        else {
            panic!()
        };
        assert_eq!(definition.fingerprint(), point);
        let mut wrong_definition = definition.clone();
        wrong_definition.declared_name = "Other".into();
        let expected_schema = *schema;
        let wrong_schema = store
            .intern_type(TypeKey::Nominal {
                definition: definition.clone(),
                schema: Digest([207; 32]),
                args: Box::new([]),
                const_args: Box::new([]),
            })
            .unwrap();
        let wrong_owner = store
            .intern_type(TypeKey::Nominal {
                definition: wrong_definition,
                schema: expected_schema,
                args: Box::new([]),
                const_args: Box::new([]),
            })
            .unwrap();
        assert!(common.require_descriptor(&store, wrong_schema).is_err());
        assert!(common.require_descriptor(&store, wrong_owner).is_err());
        db.begin_expansion_epoch(1).unwrap();
        assert!(proof.require_current(&mut db, &callee).is_err());
    }
}
#[test]
fn named_reference_identity_does_not_resolve_unreached_field_types_or_generic_arguments() {
    let (mut db, source, graph) = prepare_source(
        "struct Point{cold:fn()}fn take(x:&Point){}fn main(){}",
        2024,
    );
    let callee = callable(&mut db, &source, &graph, "take");
    db.trace.detailed = true;
    let proof = ClosedReferenceParameter::capture(&mut db, &callee, &graph, &[&source], 0)
        .unwrap()
        .unwrap();
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    proof.materialize_hint(&mut store).unwrap();
    assert!(
        !db.trace
            .events
            .iter()
            .any(|event| matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
    );
    let (mut db, source, graph) = prepare_source(
        "struct Point<T>{cold:T}fn take<T>(x:&Point<T>){}fn main(){}",
        2024,
    );
    let callee = callable(&mut db, &source, &graph, "take");
    assert!(
        ClosedReferenceParameter::capture(&mut db, &callee, &graph, &[&source], 0)
            .unwrap()
            .is_none()
    );
}
#[test]
fn exact_positive_type_bucket_never_relaxes_primitive_absence_or_ambiguity() {
    use crate::frontend::{
        names::{NameDefinition, NameLookup},
        type_name_namespace::bucket_selects,
    };
    let definition = NameDefinition {
        namespace: Namespace::Type,
        identity: Digest([208; 32]),
        defining_unit: Digest([209; 32]),
        definition_edition: 2024,
    };
    let expected = NameLookup::Defined(definition.clone());
    let mut names = BTreeMap::new();
    assert!(bucket_selects(&names, "Point", &NameLookup::Absent));
    names.insert("Point".into(), NameLookup::Absent);
    assert!(!bucket_selects(&names, "Point", &NameLookup::Absent));
    names.insert("Point".into(), expected.clone());
    assert!(bucket_selects(&names, "Point", &expected));
    names.insert("Point".into(), NameLookup::Ambiguous(vec![definition]));
    assert!(!bucket_selects(&names, "Point", &expected));
    assert!(!bucket_selects(&names, "Point", &NameLookup::Absent));
}

#[test]
fn nominal_reference_parameter_cannot_install_without_the_actual_context_certificate() {
    let (mut db, source, graph) = prepare_source(POINT, 2024);
    let callee = callable(&mut db, &source, &graph, "x_coord");
    let proof = ClosedReferenceParameter::capture(&mut db, &callee, &graph, &[&source], 0)
        .unwrap()
        .unwrap();
    let outcome = db
        .fetch(
            &SyntaxProvider,
            QueryKey::BodyStructure(callee.body().unwrap().key()),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let crate::db::value::QueryValue::BodyStructure(index) = outcome.value.as_ref() else {
        panic!()
    };
    let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
    let mut frame = Frame::new(
        index.clone(),
        index
            .pc(crate::ir::pc::NodeId(0), crate::ir::pc::Phase::Entry)
            .unwrap(),
        &mut store,
    )
    .unwrap();
    assert!(
        frame
            .install_parameter_sources(vec![proof.source_template().clone()])
            .is_err()
    );
    assert_eq!(frame.counters.operations_started, 0);
    assert!(frame.active_scopes().is_empty());
}
#[test]
fn named_aliases_keep_the_selected_source_type_and_distinct_same_spelled_owners() {
    let text = "struct Point{x:isize}type Alias=Point;mod other{pub struct Point{pub x:isize}}fn take(x:&Alias){}fn direct(x:&Point){}fn main(){}";
    let (mut db, source, graph) = prepare_source(text, 2024);
    let alias = callable(&mut db, &source, &graph, "take");
    let direct = callable(&mut db, &source, &graph, "direct");
    let one = ClosedReferenceParameter::capture(&mut db, &alias, &graph, &[&source], 0)
        .unwrap()
        .unwrap();
    let two = ClosedReferenceParameter::capture(&mut db, &direct, &graph, &[&source], 0)
        .unwrap()
        .unwrap();
    assert!(one.pointee.same_type(&two.pointee));
    let other = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "Point" && !item.def_key.module_path.is_empty())
        .unwrap()
        .def_key
        .fingerprint();
    assert_ne!(one.pointee_source(), &TyTerm::Nominal(other, vec![]));
}
