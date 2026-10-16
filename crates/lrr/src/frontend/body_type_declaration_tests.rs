//! Source proof controls over unchanged official inputs; no guest execution.
use super::*;
use crate::{
    expand::{
        generated_items::GeneratedItems,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::{
        compile_input_catalog::CompileInputCatalog, source_body_context,
        source_macros::PublishedMacroIndex,
    },
    ir::pc::{NodeId, Phase},
    source::changes::PreparedSourceView,
    syntax::parse::ParsedFile,
    traits::term::TyTerm,
};
const ORIGINAL: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/issue-19244.rs");
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("actual body Type: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("actual body Type suspended: {input:?}"),
    }
}
fn ready_type(poll: QueryPoll<BodyTypeOutcome>) -> PreparedBodyType {
    match ready(poll) {
        BodyTypeOutcome::Ready(proof) => proof,
        BodyTypeOutcome::Pending(demand) => panic!("actual body Type pending: {demand:?}"),
    }
}
fn world(text: &str, edition: u16) -> (Database, PreparedSourceView, SourceDiscovery) {
    let (provider, _, inputs) =
        crate::expand::source_declarative::assembly::tests::world(text, edition);
    let source = provider.module_source().share();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.trace.detailed = true;
    let mut discovery = SourceDiscovery::new(
        vec![source.share()],
        source.outline().root_scope.clone(),
        Namespace::Type,
        Digest::of_bytes(b"actual body Type declaration source control"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"actual body Type declaration compile context"),
        crate::expand::source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
    )
    .unwrap();
    let mut published = false;
    for _ in 0..100_000 {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::Sealed(_) => {
                discovery.publish(&mut db).unwrap();
                published = true;
                break;
            }
            SourceDiscoveryStep::Incomplete {
                diagnostic,
                blocked,
            } => panic!("actual Type discovery Incomplete {diagnostic:?} {blocked:?}"),
            SourceDiscoveryStep::NeedSourceDeriveSelection { .. } => {
                panic!("actual Type discovery NeedSourceDeriveSelection")
            }
            SourceDiscoveryStep::NeedSourceDeriveExpansion { .. } => {
                panic!("actual Type discovery NeedSourceDeriveExpansion")
            }
            SourceDiscoveryStep::NeedSourceDeriveAppend { .. } => {
                panic!("actual Type discovery NeedSourceDeriveAppend")
            }
            SourceDiscoveryStep::NeedDeriveAppend { .. } => {
                panic!("actual Type discovery NeedDeriveAppend")
            }
            SourceDiscoveryStep::NeedDeriveDeclaration { .. } => {
                panic!("actual Type discovery NeedDeriveDeclaration")
            }
            SourceDiscoveryStep::NeedAttributeSelection { .. } => {
                panic!("actual Type discovery NeedAttributeSelection")
            }
            SourceDiscoveryStep::NeedProceduralSelection { .. } => {
                panic!("actual Type discovery NeedProceduralSelection")
            }
        }
    }
    assert!(published);
    (db, source, discovery)
}
fn plan(
    db: &mut Database,
    source: &PreparedSourceView,
    body: &crate::syntax::outline::BodyHandle,
    boundary: Option<Arc<crate::expand::body_macro_boundary::GeneratedBodyBoundary>>,
) -> Arc<PreparedSourceLocalPlan> {
    let indexed = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key())));
    let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
        panic!()
    };
    let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
    let admitted = Arc::new(ready(source_body_context::capture(
        db,
        &[source],
        index,
        &pc,
    )));
    let context = ready(match boundary {
        Some(boundary) => admitted.prepare_with_generated_boundary(db, boundary),
        None => admitted.prepare(db),
    });
    let (root, facts) = crate::syntax::floating_fixture_hygiene::root(
        source.selection().outline.crate_key,
        source.outline().profile.edition,
    );
    let parent = PublishedMacroIndex::from_view(source.share(), root, facts).unwrap();
    ready(PreparedSourceLocalPlan::prepare(db, &parent, context))
}
#[test]
fn unchanged_const_direct_type_joins_current_declaration_and_shared_nominal_kernel() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source, _) = world(ORIGINAL, edition);
        let item = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "STRUCT")
            .unwrap();
        let plan = plan(&mut db, &source, item.body.as_ref().unwrap(), None);
        let from = db.trace.events.len();
        let proof = ready_type(plan.prepare_body_type(&mut db, NodeId(0)));
        let nominal = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "MyStruct")
            .unwrap();
        assert_eq!(
            proof.source_type().source(),
            &TyTerm::Nominal(nominal.def_key.fingerprint(), vec![])
        );
        assert_eq!(proof.declaration().definition(), &nominal.def_key);
        assert_eq!(proof.declaration().token_edition(), edition);
        let actual_prefix = ready(plan.type_prefix(&mut db, NodeId(0)));
        assert_eq!(
            proof.declaration().comparison_context(),
            plan.allocation_roots()
                .hygiene()
                .name_item(actual_prefix.use_name().unwrap().context())
                .unwrap()
        );
        assert!(
            nominal
                .declaration_span
                .contains(proof.declaration().declaration_range())
        );
        assert!(Arc::ptr_eq(proof.declaration().retained_plan(), &plan));
        assert_eq!(
            proof.declaration().fingerprint(),
            StableEncode::fingerprint(proof.declaration())
        );
        assert!(proof.exclusive_metadata_bytes().unwrap() > 0);
        assert!(!db.trace.events[from..].iter().any(|event| matches!(
            event.key,
            QueryKey::Thunk(_) | QueryKey::Constant(_) | QueryKey::FunctionSignature(_)
        )));
        // Existing ordinary header-selected named Type uses the same schema
        // kernel; obtain its real main signature, never a synthetic owner.
        let main = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap();
        let signature = ready(db.fetch(
            &SyntaxProvider,
            QueryKey::FunctionSignature(main.def_key.fingerprint()),
        ));
        let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
            panic!()
        };
        let graph =
            crate::frontend::names::NameNamespaces::from_outlines(&[source.outline()]).unwrap();
        let ordinary = ready(crate::frontend::source_types::prepare_named(
            &mut db,
            &graph,
            &[&source],
            signature,
            "MyStruct",
        ));
        let NamedTypePreparation::Ready(ordinary) = ordinary else {
            panic!("ordinary nominal source proof missing")
        };
        assert!(proof.source_type().same_type(&ordinary));
        ready(proof.declaration().require_current(&mut db));
        db.begin_expansion_epoch(2).unwrap();
        assert!(matches!(
            proof.declaration().require_current(&mut db),
            QueryPoll::Ready(Err(_))
        ));
    }
}
#[test]
fn generated_and_wrapper_body_use_the_same_unchanged_nominal_declaration() {
    for edition in [2015, 2018, 2021, 2024] {
        for wrapper in [false, true] {
            // Entire original prefix is unchanged. LRR source-route controls are
            // appended outside it; no full fixture or execution acceptance claimed.
            let source_text = format!(
                "{ORIGINAL}\nmacro_rules! emit{{()=>{{fn emitted(){{let _=MyStruct{{field:42}};}}}}}}{}",
                if wrapper {
                    "macro_rules! wrap{()=>{emit!();}}wrap!();"
                } else {
                    "emit!();"
                }
            );
            let (mut db, source, discovery) = world(&source_text, edition);
            let mut selected = None;
            for record in discovery.sealed_expansions().unwrap() {
                let items = GeneratedItems::admit(record.key(), Arc::clone(record), 1).unwrap();
                if !items.items().iter().any(|item| {
                    item.name
                        .as_ref()
                        .is_some_and(|name| name.text == "emitted")
                }) {
                    continue;
                }
                let key = ParsedFile::from_generated_items(&items).unwrap().key();
                let input = db.input(&InputKey::GeneratedSyntax(key.content)).unwrap();
                let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
                    panic!()
                };
                let id = unit
                    .definitions
                    .iter()
                    .find(|(_, key)| {
                        key.item_kind == ItemKind::Function && key.declared_name == "emitted"
                    })
                    .unwrap()
                    .0;
                let body = db.input(&InputKey::DefinitionBody(*id)).unwrap();
                let InputValue::DefinitionBody(body) = body.as_ref() else {
                    panic!()
                };
                let boundary = discovery.generated_body_macro_boundary(unit).unwrap();
                assert!(
                    selected
                        .replace(plan(&mut db, &source, body, Some(boundary)))
                        .is_none()
                );
            }
            let plan = selected.unwrap();
            let mut nodes = Vec::new();
            for node in plan.context().source().index().nodes() {
                let Some(occurrence) = ready(plan.context().body_type_occurrence(&mut db, node.id))
                else {
                    continue;
                };
                use crate::frontend::source_body_context::type_occurrences::BodyTypeSegment;
                if matches!(occurrence.segments(),[BodyTypeSegment::Name(name)] if name.name()=="MyStruct")
                {
                    nodes.push(node.id);
                }
            }
            assert_eq!(nodes.len(), 1);
            let from = db.trace.events.len();
            let proof = ready_type(plan.prepare_body_type(&mut db, nodes[0]));
            let original = source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "MyStruct")
                .unwrap();
            assert_eq!(proof.declaration().definition(), &original.def_key);
            assert_eq!(
                proof.source_type().source(),
                &TyTerm::Nominal(original.def_key.fingerprint(), vec![])
            );
            assert!(!db.trace.events[from..].iter().any(|event| matches!(
                event.key,
                QueryKey::Thunk(_) | QueryKey::Constant(_) | QueryKey::FunctionSignature(_)
            )));
            db.begin_expansion_epoch(2).unwrap();
            assert!(matches!(
                proof.declaration().require_current(&mut db),
                QueryPoll::Ready(Err(_))
            ));
        }
    }
}
#[test]
fn unchanged_local_type_needs_genuine_declaration_publication_before_schema() {
    const LOCAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-to-subfield.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, source, _) = world(LOCAL, edition);
        let main = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "main")
            .unwrap();
        let plan = plan(&mut db, &source, main.body.as_ref().unwrap(), None);
        let mut count = 0;
        for node in plan.context().source().index().nodes() {
            let Some(occurrence) = ready(plan.context().body_type_occurrence(&mut db, node.id))
            else {
                continue;
            };
            use crate::frontend::source_body_context::type_occurrences::BodyTypeSegment;
            let [BodyTypeSegment::Name(name)] = occurrence.segments() else {
                continue;
            };
            if !matches!(name.name(), "A" | "B") {
                continue;
            }
            let identity = plan
                .context()
                .facts()
                .items
                .iter()
                .find(|item| item.item.def_key.declared_name == name.name())
                .unwrap()
                .item
                .def_key
                .fingerprint();
            let from = db.trace.events.len();
            assert!(
                matches!(ready(plan.prepare_body_type(&mut db,node.id)),BodyTypeOutcome::Pending(BodyTypeDemand::DeclarationPublication(actual)) if actual==identity)
            );
            assert!(
                !db.trace.events[from..]
                    .iter()
                    .any(|event| matches!(event.key, QueryKey::TypeSchema(_)))
            );
            count += 1;
        }
        assert_eq!(count, 2);
    }
}
