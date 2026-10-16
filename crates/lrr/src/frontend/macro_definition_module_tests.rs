//! Source authority controls only; no guest execution or stock fixture pass.
use super::*;
use crate::{
    db::value::QueryValue,
    expand::{
        generated_items::GeneratedItems,
        source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    },
    frontend::{
        compile_input_catalog::CompileInputCatalog, source_body_context,
        source_macros::PublishedMacroIndex,
    },
    ir::pc::{NodeId, Phase},
    source::manifest::{ItemKind, Namespace},
    syntax::parse::ParsedFile,
};
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("selected macro module: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("selected macro module suspended: {input:?}"),
    }
}
#[test]
fn unchanged_official_macro_owner_keeps_actual_nested_declaration_module() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/auxiliary/intercrate.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (provider, _, _) =
            crate::expand::source_declarative::assembly::tests::world(ORIGINAL, edition);
        let source = provider.module_source();
        let actual = source
            .outline()
            .items
            .iter()
            .find(|item| {
                item.def_key.item_kind == ItemKind::Macro
                    && item.scope.module_path == ["foo", "bar"]
            })
            .unwrap();
        let definition = provider
            .macro_index()
            .definition(actual.def_key.fingerprint())
            .unwrap();
        let receipt = provider
            .macro_index()
            .source_definition_module(source, definition)
            .unwrap()
            .unwrap();
        assert_eq!(receipt.item().scope, actual.scope);
        assert_eq!(receipt.item().declaration_span, actual.declaration_span);
        assert!(
            provider
                .macro_index()
                .hygiene_facts()
                .context(receipt.context())
                .unwrap()
                .parent
                .is_none()
        );
        let mut wrong = definition.clone();
        wrong.identity = Digest([217; 32]);
        assert!(
            provider
                .macro_index()
                .source_definition_module(source, &wrong)
                .unwrap()
                .is_none()
        );
        let (other, _, _) = crate::expand::source_declarative::assembly::tests::world(
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/dollar-crate-modern.rs"
            ),
            edition,
        );
        assert!(
            provider
                .macro_index()
                .source_definition_module(other.module_source(), definition)
                .is_err()
        );
    }
    // The check-pass crate and its full expansion/diagnostics remain unexecuted.
}

#[test]
fn actual_opaque_body_record_joins_selected_definition_module_and_expires() {
    // LRR authority matrix control: source assembly/selection are real. This is
    // not substituted for the unchanged upstream language/privacy witnesses.
    const SOURCE: &str = "#![feature(decl_macro)]mod definitions{struct Target{x:i32}pub macro emit(){fn answer(){let _=Target{x:1};}}}definitions::emit!();fn main(){}";
    for edition in [2015, 2018, 2021, 2024] {
        let (provider, _, inputs) =
            crate::expand::source_declarative::assembly::tests::world(SOURCE, edition);
        let source = provider.module_source().share();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        db.trace.detailed = true;
        let mut discovery = SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Type,
            Digest::of_bytes(b"actual opaque definition module authority"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"actual opaque definition module compile context"),
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
                _ => panic!("opaque source authority needs another actual producer"),
            }
        }
        assert!(published);
        let mut records = discovery.sealed_expansions().unwrap();
        let record = Arc::clone(records.next().unwrap());
        assert!(records.next().is_none());
        // Test-only lookup of the actual published structural parse key. The
        // production receipt never replays/parses an expansion or owns a tree.
        let items = GeneratedItems::admit(record.key(), Arc::clone(&record), 1).unwrap();
        let key = ParsedFile::from_generated_items(&items).unwrap().key();
        let input = db.input(&InputKey::GeneratedSyntax(key.content)).unwrap();
        let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
            panic!()
        };
        let definition = unit
            .definitions
            .iter()
            .find(|(_, definition)| {
                definition.item_kind == ItemKind::Function && definition.declared_name == "answer"
            })
            .unwrap()
            .0;
        let body = db.input(&InputKey::DefinitionBody(*definition)).unwrap();
        let InputValue::DefinitionBody(body) = body.as_ref() else {
            panic!()
        };
        let indexed = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key())));
        let QueryValue::BodyStructure(index) = indexed.value.as_ref() else {
            panic!()
        };
        let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
        let admitted = Arc::new(ready(source_body_context::capture(
            &mut db,
            &[&source],
            index,
            &pc,
        )));
        let boundary = discovery.generated_body_macro_boundary(unit).unwrap();
        let context = ready(admitted.prepare_with_generated_boundary(&mut db, boundary));
        let (root, facts) = crate::syntax::floating_fixture_hygiene::root(
            source.selection().outline.crate_key,
            edition,
        );
        let parent = PublishedMacroIndex::from_view(source.share(), root, facts).unwrap();
        let plan = ready(PreparedSourceLocalPlan::prepare(&mut db, &parent, context));
        let mut selected = None;
        for node in index.nodes() {
            let Some(occurrence) = ready(plan.context().body_type_occurrence(&mut db, node.id))
            else {
                continue;
            };
            use crate::frontend::source_body_context::type_occurrences::BodyTypeSegment;
            let [BodyTypeSegment::Name(name)] = occurrence.segments() else {
                continue;
            };
            if name.name() == "Target" {
                assert!(
                    selected
                        .replace(ready(plan.type_prefix(&mut db, node.id)))
                        .is_none()
                );
            }
        }
        let prefix = selected.unwrap();
        assert!(matches!(
            prefix.selection(),
            LocalTypePrefixSelection::NeedDefinitionModule(_)
        ));
        let (unrelated, _, _) = crate::expand::source_declarative::assembly::tests::world(
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/auxiliary/intercrate.rs"
            ),
            edition,
        );
        let refused = ready(SelectedMacroDefinitionModule::capture_source(
            &mut db,
            &prefix,
            unrelated.module_source(),
            unrelated.macro_index(),
        ));
        assert!(matches!(
            refused,
            DefinitionModuleOutcome::Pending(DefinitionModuleDemand::OwningSource(_))
        ));
        let captured = ready(SelectedMacroDefinitionModule::capture_source(
            &mut db,
            &prefix,
            &source,
            provider.macro_index(),
        ));
        let receipt = match captured {
            DefinitionModuleOutcome::Ready(receipt) => receipt,
            DefinitionModuleOutcome::Pending(demand) => {
                panic!("actual selected macro declaration module pending: {demand:?}")
            }
        };
        assert_eq!(receipt.scope().module_path, ["definitions"]);
        assert_ne!(
            receipt.scope(),
            prefix.retained_plan().module_scope().unwrap()
        );
        assert_eq!(receipt.source(), source.selection());
        assert_eq!(
            receipt.definition().identity,
            source
                .outline()
                .items
                .iter()
                .find(|item| item.def_key.item_kind == ItemKind::Macro)
                .unwrap()
                .def_key
                .fingerprint()
        );
        assert_eq!(receipt.identity(), receipt.fingerprint());
        assert!(
            receipt.exclusive_metadata_bytes()
                >= std::mem::size_of::<SelectedMacroDefinitionModule>()
        );
        assert!(Arc::ptr_eq(receipt.retained_plan(), &plan));
        assert_eq!(
            provider
                .macro_index()
                .hygiene_facts()
                .name_item(receipt.declaration_context())
                .unwrap(),
            receipt.declaration_comparison()
        );
        ready(receipt.revalidate(&mut db));
        db.begin_expansion_epoch(2).unwrap();
        assert!(matches!(
            receipt.revalidate(&mut db),
            QueryPoll::Ready(Err(_))
        ));
        assert!(matches!(
            SelectedMacroDefinitionModule::capture_source(
                &mut db,
                &prefix,
                &source,
                provider.macro_index()
            ),
            QueryPoll::Ready(Err(_))
        ));
        assert!(
            !db.trace
                .events
                .iter()
                .any(|event| matches!(event.key, QueryKey::Thunk(_) | QueryKey::Constant(_)))
        );
    }
    // No final Type privacy/member admission, runtime return or guest execution.
}
