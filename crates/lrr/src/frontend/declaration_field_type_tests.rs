//! Source/authority controls over complete unchanged local-record rustc source.
#[path = "prepared_constructor_tests.rs"]
mod prepared_constructor_tests;
use super::*;
use crate::{
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::{callables::CallSelection, compile_input_catalog::CompileInputCatalog},
    source::changes::PreparedSourceView,
    traits::term::TyTerm,
};
const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-to-subfield.rs"
);
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual declaration field Type: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual declaration field Type demand: {input:?}"),
    }
}
fn complete(
    db: &mut Database,
    source: &PreparedSourceView,
    namespace: Namespace,
) -> SourceDiscovery {
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        namespace,
        Digest::of_bytes(b"declaration-field-Type-source-control"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"declaration-field-Type-compiler-control"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => {
                discovery.publish(db).unwrap();
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!("actual field discovery incomplete: {diagnostic:?} {blocked:?}"),
            SourceDiscoveryStep::NeedSourceDeriveSelection { .. } => {
                panic!("actual NeedSourceDeriveSelection")
            }
            SourceDiscoveryStep::NeedSourceDeriveExpansion { .. } => {
                panic!("actual NeedSourceDeriveExpansion")
            }
            SourceDiscoveryStep::NeedSourceDeriveAppend { .. } => {
                panic!("actual NeedSourceDeriveAppend")
            }
            SourceDiscoveryStep::NeedDeriveAppend { .. } => panic!("actual NeedDeriveAppend"),
            SourceDiscoveryStep::NeedDeriveDeclaration { .. } => {
                panic!("actual NeedDeriveDeclaration")
            }
            SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("actual NeedAttributeSelection")
            }
            SourceDiscoveryStep::NeedProceduralSelection { .. } => {
                panic!("actual NeedProceduralSelection")
            }
        }
    }
    discovery
}
fn world(text: &str, edition: u16, retained: bool) -> (Database, Arc<PreparedSourceLocalPlan>) {
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(text, edition);
    let source = provider.module_source().share();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.trace.detailed = true;
    let discovery = complete(&mut db, &source, Namespace::Value);
    let selected = ready(discovery.entry_function(&mut db, "main"));
    let CallSelection::Source(selected) = selected else {
        panic!("official source main unavailable: {selected:?}")
    };
    let mut body = selected.body().unwrap().key();
    if retained {
        let prepared = discovery
            .prepare_body_syntax(source.selection(), selected.identity())
            .unwrap();
        let input = ready(crate::db::retained_syntax::RetainedSyntaxInput::prepare(
            &mut db,
            prepared,
            discovery.key().clone(),
        ));
        ready(db.publish_retained_syntax(Arc::clone(&input)));
        body = ready(db.publish_retained_body(input, selected.identity()));
    }
    let plan = ready(db.prepare_source_local_plan(&discovery, &[&source], &body));
    // This is actual Type completion, not a Value-absence primitive fallback.
    // Production must project genuine existing Type receipts through the same
    // module service, or prepare this namespace before admitting the field.
    let _type_discovery = complete(&mut db, &source, Namespace::Type);
    for row in &plan.context().facts().items {
        ready(db.publish_local_declaration(plan.context(), row.item.def_key.fingerprint()));
    }
    (db, plan)
}
fn concrete(poll: QueryPoll<NamedTypePreparation>) -> SourceType {
    match ready(poll) {
        NamedTypePreparation::Ready(source) => source,
        NamedTypePreparation::Parameter(parameter) => {
            panic!("unexpected field generic: {parameter:?}")
        }
        NamedTypePreparation::Alias(alias) => panic!("unexpected deferred field alias: {alias:?}"),
        NamedTypePreparation::Name(name) => panic!("actual field Type pending: {name:?}"),
    }
}
fn schemas(db: &mut Database, plan: &PreparedSourceLocalPlan) -> Vec<NominalSchema> {
    plan.context()
        .facts()
        .items
        .iter()
        .filter(|row| {
            matches!(
                row.item.def_key.item_kind,
                ItemKind::Struct | ItemKind::Enum
            )
        })
        .map(|row| {
            let result = ready(db.fetch(
                &SyntaxProvider,
                QueryKey::TypeSchema(row.item.def_key.fingerprint()),
            ));
            let QueryValue::TypeSchema(schema) = result.value.as_ref() else {
                panic!("actual schema query differs")
            };
            schema.as_ref().clone()
        })
        .collect()
}
fn check(db: &mut Database, plan: &Arc<PreparedSourceLocalPlan>) {
    let nominal = plan
        .context()
        .facts()
        .items
        .iter()
        .find(|row| row.item.def_key.declared_name == "B")
        .unwrap()
        .item
        .def_key
        .fingerprint();
    let mut fields = 0;
    for schema in schemas(db, plan) {
        for field in schema.variants.iter().flat_map(|v| &v.fields) {
            let from = db.trace.events.len();
            let ty = concrete(plan.prepare_field_type(db, &schema, field));
            if field.name.as_deref() == Some("w") {
                assert_eq!(ty.source(), &TyTerm::Nominal(nominal, Vec::new()));
            } else {
                assert_eq!(ty.primitive(), Some(crate::ir::op::PrimitiveType::Isize));
            }
            assert!(
                !db.trace.events[from..]
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::Thunk(_) | QueryKey::Constant(_)))
            );
            fields += 1;
        }
    }
    assert_eq!(fields, 3);
}
#[test]
fn unchanged_local_field_types_use_their_actual_header_scope_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        for retained in [false, true] {
            let (mut db, plan) = world(ORIGINAL, edition, retained);
            check(&mut db, &plan);
        }
    }
}
#[test]
fn generated_and_wrapper_local_fields_join_the_same_declaration_prefix() {
    for edition in [2015, 2018, 2021, 2024] {
        for wrapper in [false, true] {
            let text = format!(
                "macro_rules! emit{{()=>{{{ORIGINAL}}}}}{}",
                if wrapper {
                    "macro_rules! wrap{()=>{emit!();}}wrap!();"
                } else {
                    "emit!();"
                }
            );
            let (mut db, plan) = world(&text, edition, false);
            check(&mut db, &plan);
            let schema = schemas(&mut db, &plan).remove(0);
            let field = &schema.variants[0].fields[0];
            db.begin_expansion_epoch(2).unwrap();
            refused(
                plan.prepare_field_type(&mut db, &schema, field),
                "stale generated field environment",
            );
        }
    }
}
fn refused(poll: QueryPoll<NamedTypePreparation>, label: &str) {
    match poll {
        QueryPoll::Ready(Err(_)) => {}
        QueryPoll::NeedInput(input) => panic!("{label} suspended unexpectedly: {input:?}"),
        QueryPoll::Ready(Ok(NamedTypePreparation::Ready(proof))) => {
            panic!("{label} admitted {:?}", proof.source())
        }
        QueryPoll::Ready(Ok(NamedTypePreparation::Name(name))) => {
            panic!("{label} returned successful Name {name:?}")
        }
        QueryPoll::Ready(Ok(NamedTypePreparation::Parameter(parameter))) => {
            panic!("{label} returned successful Parameter {parameter:?}")
        }
        QueryPoll::Ready(Ok(NamedTypePreparation::Alias(alias))) => {
            panic!("{label} returned successful Alias {alias:?}")
        }
    }
}
#[test]
fn field_recipe_substitution_and_revoked_owning_environment_are_refused() {
    let (mut db, plan) = world(ORIGINAL, 2024, false);
    let schema = schemas(&mut db, &plan)
        .into_iter()
        .find(|schema| {
            schema
                .variants
                .iter()
                .flat_map(|v| &v.fields)
                .any(|f| f.name.as_deref() == Some("w"))
        })
        .unwrap();
    let field = schema
        .variants
        .iter()
        .flat_map(|v| &v.fields)
        .find(|field| field.name.as_deref() == Some("w"))
        .unwrap();
    concrete(plan.prepare_field_type(&mut db, &schema, field));
    let mut forged = field.clone();
    forged.type_range =
        SourceRange::checked(field.type_range.start + 1, field.type_range.end).unwrap();
    refused(
        plan.prepare_field_type(&mut db, &schema, &forged),
        "caller field recipe substitution",
    );
    db.publish_inputs(BTreeMap::new()).unwrap();
    refused(
        plan.prepare_field_type(&mut db, &schema, field),
        "revoked owning field environment",
    );
}
