//! Enum variants as Value/Type namespace members of their enum, constructor
//! selection and binding through `CallResolver`, discriminant tables and the
//! committed aggregate representation of constructed nominal values.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    frontend::{
        callables::{CallResolver, CallSelection},
        names::{NameLookup, NameNamespaces},
    },
    runtime::{
        cell::{CellOwner, GuestStore, StorageContract},
        shape::{TypeId, TypeKey},
        value::ValueRepr,
    },
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        outline::{Outline, ScopeKey, VariantOutlineShape, variant_identity},
        parse::{ParseKey, ParsedFile},
    },
};
use std::collections::BTreeMap;

const EXPORT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs-enums/enum-export-inheritance.rs"
);
const DISCRIMINANTS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs-enums/enum-discr.rs"
);
const NUMERIC_FIELDS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/structs-enums/numeric-fields.rs"
);
const COLD_VARIANT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/enum-discriminant/issue-61696.rs"
);

fn publish(source: &str) -> (Database, Outline, PreparedSourceView) {
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let crate_key = Digest::of_bytes(b"lrr-nominal-constructor-fixture");
    let parsed = ParsedFile::parse(&blob, profile.clone()).unwrap();
    let outline = parsed.outline(crate_key, vec![], &cfg).unwrap();
    let path = SourcePath::new("main.rs").unwrap();
    let prepared = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path: path.clone(),
            crate_key,
            module_path: vec![],
            profile: profile.clone(),
            cfg,
            blob,
        }],
        BTreeMap::new(),
    )
    .unwrap();
    let mut db = Database::default();
    prepared.publish(&mut db).unwrap();
    let selection = SourceSelection {
        path,
        outline: crate::db::key::OutlineKey {
            parse: ParseKey {
                content: outline.content,
                profile,
            },
            crate_key,
            module_path: vec![],
            cfg: outline.cfg,
        },
    };
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("fixture source view absent");
    };
    (db, outline, view)
}
fn item<'a>(outline: &'a Outline, name: &str) -> &'a crate::syntax::outline::ItemOutline {
    outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == name)
        .unwrap()
}
fn root(outline: &Outline) -> ScopeKey {
    outline.root_scope.clone()
}
fn lookup(
    graph: &NameNamespaces,
    outline: &Outline,
    namespace: Namespace,
    path: &[&str],
) -> NameLookup {
    let path =
        NameNamespaces::path(outline.root_scope.crate_key, path, outline.profile.edition).unwrap();
    graph.lookup(namespace, &path, &root(outline)).unwrap()
}

#[test]
fn rustc_enum_variants_are_members_of_their_enum_namespace() {
    let (_, outline, _) = publish(EXPORT);
    let foo = item(&outline, "Foo");
    assert_eq!(
        foo.variants
            .iter()
            .map(|variant| (variant.name.as_str(), variant.shape))
            .collect::<Vec<_>>(),
        [
            ("Bar", VariantOutlineShape::Unit),
            ("Baz", VariantOutlineShape::Unit),
            ("Boo", VariantOutlineShape::Unit)
        ]
    );
    let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
    let bar = variant_identity(foo.def_key.fingerprint(), "Bar");
    for namespace in [Namespace::Value, Namespace::Type] {
        assert!(matches!(
            lookup(&graph, &outline, namespace, &["a", "Foo", "Bar"]),
            NameLookup::Defined(found) if found.identity == bar
        ));
    }
    let selected = graph.enum_variant(bar).unwrap();
    assert_eq!(selected.enumeration, foo.def_key.fingerprint());
    assert_eq!(selected.name, "Bar");
    // The enum itself is still the Type member at its own path.
    assert!(matches!(
        lookup(&graph, &outline, Namespace::Type, &["a", "Foo"]),
        NameLookup::Defined(found) if found.identity == foo.def_key.fingerprint()
    ));
    assert_eq!(
        lookup(&graph, &outline, Namespace::Value, &["a", "Foo", "Quux"]),
        NameLookup::Absent
    );
}

#[test]
fn rustc_record_variants_bind_no_value_constructor() {
    let (_, outline, _) = publish(COLD_VARIANT);
    let e1 = item(&outline, "E1");
    assert_eq!(e1.variants.len(), 4);
    assert_eq!(e1.variants[0].shape, VariantOutlineShape::Record);
    let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
    let v1 = variant_identity(e1.def_key.fingerprint(), "V1");
    assert!(matches!(
        lookup(&graph, &outline, Namespace::Type, &["E1", "V1"]),
        NameLookup::Defined(found) if found.identity == v1
    ));
    assert_eq!(
        lookup(&graph, &outline, Namespace::Value, &["E1", "V1"]),
        NameLookup::Absent
    );
    assert!(matches!(
        lookup(&graph, &outline, Namespace::Value, &["E1", "V3"]),
        NameLookup::Defined(found) if found.identity == variant_identity(e1.def_key.fingerprint(), "V3")
    ));
}

#[test]
fn glob_import_of_an_enum_binds_its_variants() {
    // LRR control: `use E::*` reaches variants through the ordinary glob edge.
    let source = "use E::*; enum E { X, Y(i32), Z { q: bool } } fn main() {}";
    let (_, outline, _) = publish(source);
    let e = item(&outline, "E");
    let mut graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
    graph.resolve_imports().unwrap();
    for (name, namespace) in [
        ("X", Namespace::Value),
        ("Y", Namespace::Value),
        ("Z", Namespace::Type),
    ] {
        assert!(matches!(
            graph.lookup_unqualified(namespace, name, &root(&outline), outline.profile.edition, None).unwrap(),
            NameLookup::Defined(found) if found.identity == variant_identity(e.def_key.fingerprint(), name)
        ));
    }
    assert_eq!(
        graph
            .lookup_unqualified(
                Namespace::Value,
                "Z",
                &root(&outline),
                outline.profile.edition,
                None
            )
            .unwrap(),
        NameLookup::Absent
    );
}

#[test]
fn rustc_discriminant_tables_follow_declaration_order() {
    let (mut db, outline, _) = publish(DISCRIMINANTS);
    let schema = |db: &mut Database, name: &str| {
        let outcome = db
            .fetch(
                &SyntaxProvider,
                QueryKey::TypeSchema(item(&outline, name).def_key.fingerprint()),
            )
            .into_ready()
            .unwrap()
            .unwrap();
        let QueryValue::TypeSchema(schema) = outcome.value.as_ref() else {
            panic!("schema absent")
        };
        schema.as_ref().clone()
    };
    assert_eq!(
        schema(&mut db, "Animal")
            .literal_discriminants()
            .unwrap()
            .as_ref(),
        [Some(0), Some(1), Some(2), Some(3)]
    );
    assert_eq!(
        schema(&mut db, "Hero")
            .literal_discriminants()
            .unwrap()
            .as_ref(),
        [Some(-1), Some(-2), Some(-3), Some(-4)]
    );
    // LRR controls: implicit successors, radix/suffix spellings, and a
    // non-literal expression leaving it and its implicit successor unknown.
    let (mut db, outline, _) =
        publish("enum A { X, Y = 0x10, Z, W = 7u8, V = 1 << 2, U } fn main() {}");
    let outcome = db
        .fetch(
            &SyntaxProvider,
            QueryKey::TypeSchema(item(&outline, "A").def_key.fingerprint()),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::TypeSchema(schema) = outcome.value.as_ref() else {
        panic!("schema absent")
    };
    assert_eq!(
        schema.literal_discriminants().unwrap().as_ref(),
        [Some(0), Some(16), Some(17), Some(7), None, None]
    );
}

#[test]
fn rustc_numeric_field_initializers_map_tuple_ordinals() {
    let (mut db, outline, _) = publish(NUMERIC_FIELDS);
    let outcome = db
        .fetch(
            &SyntaxProvider,
            QueryKey::TypeSchema(item(&outline, "S").def_key.fingerprint()),
        )
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::TypeSchema(schema) = outcome.value.as_ref() else {
        panic!("schema absent")
    };
    let variant = schema.variant(None).unwrap();
    // Original `S{1: 10, 0: 11}` written order.
    assert_eq!(
        variant.named_initializer_slots(&["1", "0"]).unwrap(),
        [1, 0]
    );
    assert!(variant.named_initializer_slots(&["0", "0"]).is_err());
    assert!(variant.named_initializer_slots(&["0"]).is_err());
    assert!(variant.named_initializer_slots(&["0", "2"]).is_err());
}

#[test]
fn constructors_select_and_bind_through_the_value_namespace() {
    let (mut db, outline, view) = publish(EXPORT);
    let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let main = item(&outline, "main").def_key.fingerprint();
    let QueryPoll::Ready(Ok(CallSelection::Constructor(target))) =
        resolver.source_function(&mut db, main, "a::Foo::Baz")
    else {
        panic!("variant constructor was not selected")
    };
    assert_eq!(target.owner(), &item(&outline, "Foo").def_key);
    assert_eq!(target.variant_name(), Some("Baz"));
    let bound = bind_constructor(&mut db, &graph, &sources, &target)
        .into_ready()
        .unwrap()
        .unwrap();
    assert_eq!(bound.shape(), FieldShape::Unit);
    assert_eq!(bound.variant().variant(), 1);
    assert!(bound.variant().fields().is_empty());
    assert_eq!(
        bound.variant().identity(),
        item(&outline, "Foo").def_key.fingerprint()
    );

    // The unit variant commits as the enum's nominal aggregate.
    let mut store = GuestStore::for_epoch(1).unwrap();
    let ty = store.intern_nominal(bound.variant()).unwrap();
    assert!(matches!(
        store.type_descriptor(ty).unwrap().key.as_ref(),
        TypeKey::Nominal { identity, .. } if *identity == item(&outline, "Foo").def_key.fingerprint()
    ));
    let ticket = construct_from(&mut store, bound.variant(), &[]);
    assert_eq!(store.aggregate_variant(ticket).unwrap(), Some(1));
    let frame = store.new_frame_anchor().unwrap();
    let cell = store
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    store.initialize(cell, ticket).unwrap();
    assert_eq!(
        store.enum_cast_operand(cell).unwrap(),
        Some(ValueRepr::signed(TypeId(8), 1).unwrap())
    );
}

#[test]
fn rustc_tuple_struct_constructor_commits_declaration_order_fields() {
    let (mut db, outline, view) = publish(NUMERIC_FIELDS);
    let graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
    let sources = [&view];
    let resolver = CallResolver::new(&graph, &sources).unwrap();
    let main = item(&outline, "main").def_key.fingerprint();
    let QueryPoll::Ready(Ok(CallSelection::Constructor(target))) =
        resolver.source_function(&mut db, main, "S")
    else {
        panic!("tuple struct constructor was not selected")
    };
    assert_eq!(target.variant_name(), None);
    let bound = bind_constructor(&mut db, &graph, &sources, &target)
        .into_ready()
        .unwrap()
        .unwrap();
    assert_eq!(bound.shape(), FieldShape::Tuple);
    assert_eq!(bound.variant().fields().len(), 2);
    let mut store = GuestStore::for_epoch(1).unwrap();
    let first = store
        .ticket(ValueRepr::unsigned(TypeId(9), 11).unwrap())
        .unwrap();
    let second = store
        .ticket(ValueRepr::unsigned(TypeId(10), 10).unwrap())
        .unwrap();
    let ticket = construct_from(&mut store, bound.variant(), &[first, second]);
    assert_eq!(store.aggregate_variant(ticket).unwrap(), Some(0));
    let field = store.aggregate_field(ticket, 1).unwrap();
    assert_eq!(
        store.read_scalar(field).unwrap(),
        ValueRepr::unsigned(TypeId(10), 10).unwrap()
    );
    // A struct value is not an enum cast operand.
    let frame = store.new_frame_anchor().unwrap();
    let ty = store.intern_nominal(bound.variant()).unwrap();
    let cell = store
        .new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::FrameSlot { frame, slot: 0 },
        )
        .unwrap();
    store.initialize(cell, ticket).unwrap();
    assert_eq!(store.enum_cast_operand(cell).unwrap(), None);
    // A mismatched field Type is refused before commit.
    let wrong = store
        .ticket(ValueRepr::unsigned(TypeId(10), 1).unwrap())
        .unwrap();
    let other = store
        .ticket(ValueRepr::unsigned(TypeId(10), 1).unwrap())
        .unwrap();
    assert!(
        crate::vm::frame::nominal_values_frame::construct_from_arguments(
            bound.variant(),
            &[],
            &[wrong, other],
            &mut store
        )
        .is_err()
    );
}

fn construct_from(
    store: &mut GuestStore,
    bound: &crate::frontend::types::BoundNominalVariant,
    tickets: &[crate::runtime::cell::TicketId],
) -> crate::runtime::cell::TicketId {
    crate::vm::frame::nominal_values_frame::construct_from_arguments(bound, &[], tickets, store)
        .unwrap()
}
