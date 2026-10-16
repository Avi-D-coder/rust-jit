//! Source authority controls on complete unchanged official inputs; no guest run.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::{
        key::{OutlineKey, QueryKey},
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::pc::{NodeId, Phase},
    source::{
        changes::{PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => panic!("local Type authority: {error:?}"),
        QueryPoll::NeedInput(input) => panic!("local Type authority suspended: {input:?}"),
    }
}
fn prepare(text: &str, owner: &str, edition: u16) -> (Database, Arc<PreparedSourceLocalPlan>) {
    let unit = Digest([240; 32]);
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let path = SourcePath::new("local-type-original.rs").unwrap();
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!()
    };
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let item = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == owner)
        .unwrap();
    let body = item.body.as_ref().unwrap();
    let value = ready(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.key())));
    let QueryValue::BodyStructure(index) = value.value.as_ref() else {
        panic!()
    };
    let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
    let source = Arc::new(ready(crate::frontend::source_body_context::capture(
        &mut db,
        &[&view],
        index,
        &pc,
    )));
    let context = ready(source.prepare(&mut db));
    let (root, hygiene) = crate::syntax::floating_fixture_hygiene::root(unit, edition);
    let parent = PublishedMacroIndex::from_view(view.share(), root, hygiene).unwrap();
    let plan = ready(PreparedSourceLocalPlan::prepare(&mut db, &parent, context));
    (db, plan)
}
#[test]
fn unchanged_local_record_name_selects_its_actual_declared_type() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-to-subfield.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, plan) = prepare(ORIGINAL, "main", edition);
        let declaration = plan
            .context()
            .facts()
            .items
            .iter()
            .find(|item| item.item.def_key.declared_name == "A")
            .unwrap();
        let identity = declaration.item.def_key.fingerprint();
        let mut count = 0;
        for node in plan.context().source().index().nodes() {
            let Some(occurrence) = ready(plan.context().body_type_occurrence(&mut db, node.id))
            else {
                continue;
            };
            use crate::frontend::source_body_context::type_occurrences::{
                BodyTypeSegment, BodyTypeUse,
            };
            let [BodyTypeSegment::Name(name)] = occurrence.segments() else {
                continue;
            };
            if !matches!(name.name(), "A" | "B") {
                continue;
            }
            let expected = plan
                .context()
                .facts()
                .items
                .iter()
                .find(|item| item.item.def_key.declared_name == name.name())
                .unwrap()
                .item
                .def_key
                .fingerprint();
            assert_eq!(occurrence.usage(), BodyTypeUse::RecordConstructor);
            let answer = ready(plan.type_prefix(&mut db, node.id));
            let LocalTypePrefixSelection::Lookup(NameLookup::Defined(selected)) =
                answer.selection()
            else {
                panic!("actual local Type name did not select its declared source");
            };
            assert_eq!(selected.identity, expected);
            assert_eq!(selected.definition_edition, edition);
            assert!(Arc::ptr_eq(answer.retained_plan(), &plan));
            count += 1;
        }
        assert_eq!(count, 2);
        // A genuine opaque pinned mark cannot compare equal merely because the
        // declaration has the same written name. No source admission is minted.
        use lrr_expand_adapter::{
            hygiene::{HygieneStore, MarkRequest},
            token_wire::{Edition, Key as WireKey, Transparency},
        };
        let wire_edition = match edition {
            2015 => Edition::E2015,
            2018 => Edition::E2018,
            2021 => Edition::E2021,
            _ => Edition::E2024,
        };
        let mut hygiene = HygieneStore::default();
        let unit = WireKey([240; 32]);
        let root = hygiene.root(unit, wire_edition).unwrap();
        let marked = hygiene
            .apply(MarkRequest {
                parent: root,
                call_site: root,
                expansion: WireKey([241; 32]),
                defining_unit: unit,
                definition_edition: wire_edition,
                transparency: Transparency::Opaque,
            })
            .unwrap();
        let comparison = hygiene.name_contexts(&[marked]).unwrap();
        let class =
            serde_json::from_slice(&serde_json::to_vec(&comparison[0].item).unwrap()).unwrap();
        let candidate = crate::frontend::names::NameDefinition {
            namespace: crate::source::manifest::Namespace::Type,
            identity,
            defining_unit: Digest([240; 32]),
            definition_edition: edition,
        };
        assert!(matches!(
            plan.type_names.items(&[candidate], class).unwrap(),
            ItemNames::Selected(None)
        ));
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::Thunk(_)
                | QueryKey::Constant(_)
                | QueryKey::TypeSchema(_)
                | QueryKey::FunctionSignature(_)
        )));
    }
    // Complete original main/runtime is unexecuted; this is source authority only.
}
#[test]
fn unchanged_local_import_requires_its_actual_dependency_before_module_tail() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/resolve/blind-item-mixed-use-item.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (_db, plan) = prepare(ORIGINAL, "main", edition);
        let import = plan.context().facts().imports.first().unwrap();
        let at = ORIGINAL.find("g(Data2").unwrap() + 2;
        // This private request control uses the actual entered NameRef. It may
        // only test the typed import hold; it cannot mint LocalTypePrefix.
        let site = OrdinaryNameSite {
            source: plan.entered.parent.source(),
            body: &plan.entered.body,
            at: at as u64,
            name: "Data2",
            context: plan.adapter().context(),
            namespace: crate::source::manifest::Namespace::Type,
        };
        assert!(
            matches!(plan.adapter().lookup_prepared_type_prefix(&site,&plan.type_names,edition).unwrap(),super::super::PrefixResolution::NeedImport(identity) if identity==import.source.identity)
        );
    }
}

#[test]
fn unchanged_box_local_type_keeps_its_actual_unfinished_macro_frontier() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/box/unit/basic-operations.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, plan) = prepare(ORIGINAL, "autoderef_field", edition);
        let pending = plan
            .context()
            .facts()
            .events
            .iter()
            .find_map(|event| match event.kind {
                crate::syntax::entered_macros::BodyMacroEventKind::Producer {
                    identity, ..
                } => Some(identity),
                _ => None,
            })
            .unwrap();
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
            if name.name() != "J" {
                continue;
            }
            let answer = ready(plan.type_prefix(&mut db, node.id));
            assert!(
                matches!(answer.selection(),LocalTypePrefixSelection::Lookup(NameLookup::NeedInput(identity)) if *identity==pending)
            );
            count += 1;
        }
        assert_eq!(count, 1);
    }
    // Genuine standard-macro output/currentness remains required. No Box/main pass.
}

#[test]
fn unchanged_const_module_tail_demands_its_actual_current_type_bucket() {
    const ORIGINAL: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/issue-19244.rs");
    use crate::{
        db::dependency::{DependencyStamp, InputKey},
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        frontend::{
            compile_input_catalog::CompileInputCatalog, current_module_names::ModuleNameCompletion,
        },
        source::manifest::Namespace,
    };
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, plan) = prepare(ORIGINAL, "STRUCT", edition);
        db.begin_expansion_epoch(1).unwrap();
        let prefix = ready(plan.type_prefix(&mut db, NodeId(0)));
        assert!(matches!(
            prefix.selection(),
            LocalTypePrefixSelection::ModuleTail
        ));
        let selector = prefix.module_selector().unwrap();
        let (_, hygiene) = plan.entered.local_hygiene();
        let used = prefix.use_name().unwrap();
        assert_eq!(
            selector.normalized_context(),
            hygiene.item(used.context()).unwrap()
        );
        assert_eq!(
            selector.comparison_context(),
            hygiene.name_item(used.context()).unwrap()
        );
        assert!(
            hygiene
                .context(selector.normalized_context())
                .unwrap()
                .parent
                .is_none()
        );
        assert_eq!(
            plan.lexical_outline_key().parse,
            plan.context().source().parsed().key()
        );
        assert_eq!(
            plan.lexical_outline_root().content,
            plan.lexical_outline_key().parse.content
        );
        let unsealed = ready(prefix.module_name(&mut db)).unwrap();
        assert_eq!(unsealed.completion(), ModuleNameCompletion::Unsealed);
        assert!(unsealed.lookup().is_none());

        let source = plan.context().module_source().unwrap();
        let mut discovery = SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Type,
            Digest::of_bytes(b"actual local Type tail upstream discovery"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"actual local Type tail compile context"),
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
                _ => panic!("unchanged const source needs another actual producer"),
            }
        }
        assert!(published);
        let completed = ready(prefix.module_name(&mut db)).unwrap();
        assert_eq!(completed.completion(), ModuleNameCompletion::Complete);
        let Some(NameLookup::Defined(definition)) = completed.lookup() else {
            panic!("actual MyStruct module source was not selected")
        };
        let declared = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "MyStruct")
            .unwrap();
        assert_eq!(definition.identity, declared.def_key.fingerprint());
        assert!(matches!(
            completed.answer_dependency(),
            Some(DependencyStamp::Query {
                key: QueryKey::SealedName(_),
                ..
            })
        ));
        assert!(matches!(
            completed.certificate_dependency(),
            Some(DependencyStamp::Input {
                key: InputKey::DiscoverySeal(_),
                ..
            })
        ));
        let same = ready(prefix.module_name(&mut db)).unwrap();
        assert!(Arc::ptr_eq(&same, &completed));

        db.begin_expansion_epoch(2).unwrap();
        let refreshed = ready(prefix.module_name(&mut db)).unwrap();
        assert_eq!(refreshed.completion(), ModuleNameCompletion::Unsealed);
        assert!(refreshed.lookup().is_none());
        assert_ne!(refreshed.semantic_identity(), completed.semantic_identity());

        // Retaining the old result is not current authority after actual source
        // removal. This mutates LRR publication only, never the upstream input.
        db.publish_inputs(Default::default()).unwrap();
        assert!(matches!(
            prefix.module_name(&mut db),
            QueryPoll::Ready(Err(_))
        ));
        assert!(!db.trace.events.iter().any(|event| matches!(
            event.key,
            QueryKey::Thunk(_)
                | QueryKey::Constant(_)
                | QueryKey::TypeSchema(_)
                | QueryKey::FunctionSignature(_)
        )));
    }
}

#[test]
fn pinned_opaque_outer_mark_never_falls_back_to_lexical_module_membership() {
    const ORIGINAL: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/issue-19244.rs");
    use lrr_expand_adapter::{
        hygiene::{HygieneStore, MarkRequest},
        token_wire::{Edition, Key as WireKey, TokenForest, Transparency},
    };
    for edition in [2015, 2018, 2021, 2024] {
        let (_, plan) = prepare(ORIGINAL, "STRUCT", edition);
        let wire_edition = match edition {
            2015 => Edition::E2015,
            2018 => Edition::E2018,
            2021 => Edition::E2021,
            _ => Edition::E2024,
        };
        let mut pinned = HygieneStore::default();
        let owner = WireKey([240; 32]);
        let root = pinned.root(owner, wire_edition).unwrap();
        let foreign_root = pinned.root(WireKey([245; 32]), wire_edition).unwrap();
        let outer = WireKey([242; 32]);
        let opaque = pinned
            .apply(MarkRequest {
                parent: root,
                call_site: root,
                expansion: outer,
                defining_unit: owner,
                definition_edition: wire_edition,
                transparency: Transparency::Opaque,
            })
            .unwrap();
        let emitted = pinned
            .apply(MarkRequest {
                parent: foreign_root,
                call_site: opaque,
                expansion: WireKey([243; 32]),
                defining_unit: WireKey([245; 32]),
                definition_edition: wire_edition,
                transparency: Transparency::SemiTransparent,
            })
            .unwrap();
        let rules_only = pinned
            .apply(MarkRequest {
                parent: foreign_root,
                call_site: root,
                expansion: WireKey([244; 32]),
                defining_unit: WireKey([245; 32]),
                definition_edition: wire_edition,
                transparency: Transparency::SemiTransparent,
            })
            .unwrap();
        let keys = [root, opaque, emitted, rules_only];
        let normalized = pinned.resolution_contexts(&keys).unwrap();
        let names = pinned.name_contexts(&keys).unwrap();
        let encoded = TokenForest::new(vec![], pinned.records(), vec![])
            .unwrap()
            .encode()
            .unwrap();
        let decoded = crate::syntax::token_wire::TokenForest::decode(&encoded).unwrap();
        let facts = HygieneFacts::new(
            decoded.contexts().to_vec(),
            serde_json::from_slice(&serde_json::to_vec(&normalized).unwrap()).unwrap(),
        )
        .unwrap()
        .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
        .unwrap();
        assert!(
            matches!(plan.lexical_module.route(&facts, Key(opaque.0)).unwrap(), ModuleRoute::Definition(key) if key.0 == outer.0)
        );
        // The inner macro_rules call class inherited the opaque outer mark;
        // latest call-context equality must not choose the lexical module.
        assert!(
            matches!(plan.lexical_module.route(&facts, Key(emitted.0)).unwrap(), ModuleRoute::Definition(key) if key.0 == outer.0)
        );
        assert!(matches!(
            plan.lexical_module
                .route(&facts, Key(rules_only.0))
                .unwrap(),
            ModuleRoute::Lexical(_)
        ));
        assert!(plan.lexical_module.route(&facts, Key([246; 32])).is_err());
    }
    // Pure adapter-context authority control, not an admitted generated program.
}
