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
