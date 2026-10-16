//! LRR storage/lease controls using unchanged upstream nested declarations.
//! This does not execute the fixture's const evaluation or complete main.
use super::*;
use crate::{
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    frontend::names::NameNamespaces,
    runtime::{
        cell::{CellOwner, GuestStore, StorageContract},
        place::{Place, PlaceFailure},
        reference::Access,
        shape::TypeId,
        value::ValueRepr,
    },
    source::changes::PreparedSourceView,
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};

const ORIGINAL: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/issue-19244.rs");
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("nested source type failed: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("nested source type suspended: {input:?}"),
    }
}
fn world(edition: u16) -> (Database, PreparedSourceView, NameNamespaces) {
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(ORIGINAL, edition);
    let source = provider.module_source().share();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.trace.detailed = true;
    let graph = NameNamespaces::from_outlines(&[source.outline()]).unwrap();
    (db, source, graph)
}
fn selected<T>(
    db: &mut Database,
    source: &PreparedSourceView,
    graph: &NameNamespaces,
    name: &str,
    select: impl FnOnce(&mut Database, &SchemaNames<'_>) -> T,
) -> T {
    let definition = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
        .def_key
        .fingerprint();
    let captured = ready(db.fetch(&SyntaxProvider, QueryKey::TypeSchema(definition)));
    let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
        panic!()
    };
    let names = ready(SchemaNames::from_database(db, graph, source, schema));
    select(db, &names)
}
fn variant(
    db: &mut Database,
    source: &PreparedSourceView,
    graph: &NameNamespaces,
    name: &str,
) -> BoundNominalVariant {
    selected(db, source, graph, name, |db, names| {
        let bound = ready(
            names
                .bind_variant_with(None, |name, field| {
                    names.current_name(db, &[source], name, field)
                })
                .unwrap(),
        );
        let NominalBinding::Ready(bound) = bound else {
            panic!("current variant is pending: {bound:?}")
        };
        bound
    })
}
fn member(
    db: &mut Database,
    source: &PreparedSourceView,
    graph: &NameNamespaces,
    owner: &str,
    field: &str,
) -> BoundNominalField {
    selected(db, source, graph, owner, |db, names| {
        let bound =
            ready(names.bind_named_field_current(db, &[source], None, field, &names.schema.scope));
        let NominalFieldBinding::Ready(bound) = bound else {
            panic!("current field is pending: {bound:?}")
        };
        bound
    })
}
fn source_value(db: &mut Database, source: &PreparedSourceView) -> u64 {
    // Read only the actual original selected initializer's literal. No altered
    // declaration, generated adapter source, or expected output replaces it.
    let captured = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::ParseFile(source.selection().outline.parse.clone()),
    ));
    let QueryValue::ParseFile(parsed) = captured.value.as_ref() else {
        panic!()
    };
    let selected = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Const::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == "NESTED_S"))
        .unwrap();
    selected
        .syntax()
        .descendants()
        .filter_map(ast::Literal::cast)
        .next()
        .unwrap()
        .syntax()
        .text()
        .to_string()
        .parse()
        .unwrap()
}

#[test]
fn upstream_nested_nominal_field_uses_exact_source_type_through_two_address_projections() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source, graph) = world(edition);
        let parent = variant(&mut db, &source, &graph, "Nested");
        let child = variant(&mut db, &source, &graph, "MyStruct");
        let outer = member(&mut db, &source, &graph, "Nested", "nested");
        let inner = member(&mut db, &source, &graph, "MyStruct", "field");
        assert_eq!(
            outer.source_type().source(),
            &TyTerm::Nominal(child.identity(), vec![])
        );
        assert_eq!(&parent.fields()[0], outer.source_type());
        assert_eq!(outer.primitive(), None);
        assert_eq!(inner.primitive(), Some(crate::ir::op::PrimitiveType::Usize));
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::BodyStructure(_)
                | QueryKey::Thunk(_)
                | QueryKey::FunctionSignature(_)
                | QueryKey::Constant(_)
                | QueryKey::ExpansionInput(_)
        )));

        let mut store = GuestStore::new(1, 1000, 1_000_000).unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let value = source_value(&mut db, &source);
        let scalar = ValueRepr::unsigned(TypeId(13), value).unwrap();
        let (child_ty, child_guard) = store.begin_nominal_construction(&child).unwrap();
        let ticket = store.ticket(scalar).unwrap();
        store.construction_push(child_guard, 0, ticket).unwrap();
        let child_ticket = store.commit_construction(child_guard, child_ty).unwrap();
        let (parent_ty, parent_guard) = store.begin_nominal_construction(&parent).unwrap();
        // Wrong concrete type cannot be transferred even though both values
        // are admitted inputs. The original ticket remains owned on refusal.
        let wrong = store.ticket(scalar).unwrap();
        assert!(store.construction_push(parent_guard, 0, wrong).is_err());
        assert_eq!(store.peek_ticket(wrong).unwrap(), scalar);
        store.discard(wrong).unwrap();
        store
            .construction_push(parent_guard, 0, child_ticket)
            .unwrap();
        assert!(store.peek_ticket(child_ticket).is_err());
        let parent_ticket = store.commit_construction(parent_guard, parent_ty).unwrap();
        let root = store
            .new_cell(
                StorageContract::Exact(parent_ty),
                true,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(root, parent_ticket).unwrap();
        let nested = store
            .project_source_field(&Place::cell(root), &outer)
            .unwrap();
        assert_eq!(store.place_type_id(&nested).unwrap(), child_ty);
        let leaf = store.project_source_field(&nested, &inner).unwrap();
        let reference = store.borrow(&leaf, Access::Mutable).unwrap();
        assert_eq!(store.read_reference(reference).unwrap(), scalar);
        let error = store
            .project_source_field(&nested, &outer)
            .err()
            .expect("foreign parent admitted");
        assert!(
            matches!(error, PlaceFailure::RuntimeFault(error) if error.code == "LRR-E-OWNERSHIP")
        );
        assert_eq!(store.read_reference(reference).unwrap(), scalar);
        // A consumed owner ticket cannot be reused to inspect an inner cell.
        let child_cell = store.aggregate_field(parent_ticket, 0);
        assert!(
            child_cell.is_err(),
            "parent ticket must already be consumed"
        );
        // Moving the outer owner invalidates the full existing nested lease.
        let root_ticket = store.move_from(root).unwrap();
        assert!(store.read_reference(reference).is_err());
        // Explicit harness teardown is not a selected user Drop/no-Drop proof.
        let nested_ticket = store.aggregate_cleanup_next(root_ticket).unwrap().unwrap();
        let scalar_ticket = store
            .aggregate_cleanup_next(nested_ticket)
            .unwrap()
            .unwrap();
        store.discard(scalar_ticket).unwrap();
        assert!(
            store
                .aggregate_cleanup_next(nested_ticket)
                .unwrap()
                .is_none()
        );
        store.discard(nested_ticket).unwrap();
        assert!(store.aggregate_cleanup_next(root_ticket).unwrap().is_none());
        store.discard(root_ticket).unwrap();
    }
}
