//! Re-selection controls nested under actual current result/source fixture helper.
use super::*;
use crate::frontend::constant_macro_admission::CurrentConstantMacroProof;
use crate::syntax::pure_constants::Node;
const PROVIDER: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/cross-crate/auxiliary/const-fn-evaluated-cross-crate.rs"
);
#[test]
fn unchanged_array_type_children_own_exact_current_source_header_all_editions() {
    for (original, name, spelling) in [
        (ARRAY_MISMATCH, "NUMBERS", "3"),
        (ARRAY_OOB, "BLUB", "FOO[4]"),
    ] {
        for edition in [2015, 2018, 2021, 2024] {
            let (db, view, definition) = world(original, name, edition);
            let parent = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
            let (_, demand) = parent.array().unwrap();
            let before = db.trace.counts.clone();
            let child = parent
                .admission()
                .reselect_array_length_current(&db, &view, demand, MAX_BYTES)
                .unwrap();
            assert_eq!(child.plan().owner_definition(), definition);
            assert_eq!(child.plan().owner_context(), demand.context());
            assert_eq!(child.plan().owner_body(), Some(demand.body()));
            assert_eq!(child.plan().range(), demand.range());
            assert_eq!(
                &original[demand.range().start as usize..demand.range().end as usize],
                spelling
            );
            child.revalidate(&db, &view).unwrap();
            let child_type =
                CurrentConstantResultType::capture_current(&db, &view, child, MAX_BYTES).unwrap();
            assert_eq!(
                child_type.scalar().unwrap().primitive(),
                PrimitiveType::Usize
            );
            assert_eq!(db.trace.counts, before);
        }
    }
}
#[test]
fn actual_admitted_initializer_macro_proof_is_retained_in_header_length_child() {
    for edition in [2015, 2018, 2021, 2024] {
        let source = "const X:[u8;3]=panic!(\"retained\");fn main(){}";
        let (db, view, definition) = world(source, "X", edition);
        let base = source_admit(&db, &view, definition, ConstantRequest::Constant).unwrap();
        let handle = base.plan().owner_body().unwrap();
        let facts = view.entered_macro_facts(&handle.key()).unwrap();
        let invocation = facts
            .invocations
            .iter()
            .find(|i| handle.range.contains(i.range))
            .unwrap();
        let proof = CurrentConstantMacroProof::capture_current(
            &db,
            &view,
            &base,
            &handle.key(),
            invocation.range,
            MAX_BYTES,
        )
        .unwrap();
        let admitted = ConstantSourceAdmission::capture_current_with_macro_proofs(
            &db,
            &view,
            definition,
            ConstantRequest::Constant,
            MAX_BYTES,
            std::slice::from_ref(&proof),
        )
        .unwrap();
        let parent =
            CurrentConstantResultType::capture_current(&db, &view, admitted, MAX_BYTES).unwrap();
        let (_, demand) = parent.array().unwrap();
        let before = db.trace.counts.clone();
        let child = parent
            .admission()
            .reselect_array_length_current(&db, &view, demand, MAX_BYTES)
            .unwrap();
        let initializer = child.plan().source().named_constant("X").unwrap();
        let Node::AdmittedPanic(retained) = child.plan().source().node(&initializer.body).unwrap()
        else {
            panic!("parent actual macro certificate discarded")
        };
        assert_eq!(retained.identity(), proof.identity());
        assert_eq!(retained.message(), "retained");
        let empty = source_admit(&db, &view, definition, demand.request()).unwrap();
        assert_ne!(child.plan().identity(), empty.plan().identity());
        assert_eq!(db.trace.counts, before);
    }
}
#[test]
fn actual_multisource_world_cannot_be_reselected_as_an_empty_singleton() {
    let mut inputs_vec = Vec::new();
    let mut selections = Vec::new();
    for (index, text) in [ARRAY_MISMATCH, PROVIDER].into_iter().enumerate() {
        let profile = ProfileIdentity::current();
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        let path = SourcePath::new(format!("actual-original-{index}.rs")).unwrap();
        let unit = Digest::of_bytes(format!("child original unit {index}").as_bytes());
        selections.push(SourceSelection {
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
        });
        inputs_vec.push(SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        });
    }
    let inputs = PreparedSourceInputs::refresh(None, inputs_vec, Default::default()).unwrap();
    let views: Vec<_> = selections
        .iter()
        .map(|s| {
            let SourceSelectionOutcome::Selected(v) = inputs.select(s).unwrap() else {
                panic!()
            };
            v
        })
        .collect();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    db.trace.detailed = true;
    let item = views[0]
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "NUMBERS")
        .unwrap();
    let definition = item.def_key.fingerprint();
    let scope = item.scope.clone();
    for namespace in [Namespace::Value, Namespace::Type] {
        let mut discovery = SourceDiscovery::new(
            views.iter().map(PreparedSourceView::share).collect(),
            scope.clone(),
            namespace,
            Digest::of_bytes(b"actual child multisource world"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"child source declarative"),
            Digest::of_bytes(b"child source compiler"),
            Arc::new(
                crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                    &ProfileIdentity::current(),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::Sealed(_) => break,
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Incomplete { diagnostic, .. } => panic!(
                    "actual discovery incomplete: {} {}",
                    diagnostic.code, diagnostic.message
                ),
                _ => panic!("actual original source set prerequisite not established"),
            }
        }
        discovery.publish(&mut db).unwrap();
    }
    let parent = ConstantSourceAdmission::capture_current_in_sources(
        &db,
        &views[0],
        definition,
        ConstantRequest::Constant,
        MAX_BYTES,
        &views.iter().collect::<Vec<_>>(),
    )
    .unwrap();
    let parent =
        CurrentConstantResultType::capture_current(&db, &views[0], parent, MAX_BYTES).unwrap();
    let (_, demand) = parent.array().unwrap();
    assert!(source_admit(&db, &views[0], definition, demand.request()).is_err());
    let child = parent
        .admission()
        .reselect_array_length_current(&db, &views[0], demand, MAX_BYTES)
        .unwrap();
    assert_eq!(child.plan().range(), demand.range());
    child.revalidate(&db, &views[0]).unwrap();
    assert!(db.trace.counts.is_empty());
}
#[test]
fn demand_from_another_declaration_or_scalar_parent_is_not_reselection_authority() {
    let source = "const X:[u8;3]=[1,2,3];const Y:[u8;4]=[1,2,3,4];const Z:u8=1;fn main(){}";
    let (db, view, definition) = world(source, "X", 2024);
    let x = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let (_, demand) = x.array().unwrap();
    for name in ["Y", "Z"] {
        let other = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == name)
            .unwrap()
            .def_key
            .fingerprint();
        let parent = source_admit(&db, &view, other, ConstantRequest::Constant).unwrap();
        assert!(
            parent
                .reselect_array_length_current(&db, &view, demand, MAX_BYTES)
                .is_err()
        );
    }
}
#[test]
fn forged_range_and_changed_argument_parent_plan_refuse_before_any_child() {
    let (db, view, definition) = world(ARRAY_MISMATCH, "NUMBERS", 2024);
    let mut parent = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let ResultReadiness::Array { length, .. } = &mut parent.readiness else {
        panic!()
    };
    length.range.start += 1;
    assert!(
        parent
            .admission
            .reselect_array_length_current(&db, &view, length, MAX_BYTES)
            .is_err()
    );
    let source = "const fn f(x:u32)->[u8;2]{[1,2]}fn main(){}";
    let (db, view, definition) = world(source, "f", 2024);
    let a = ready(
        &db,
        &view,
        definition,
        ConstantRequest::ConstCall(vec![arg(1)]),
    )
    .unwrap();
    let b = ready(
        &db,
        &view,
        definition,
        ConstantRequest::ConstCall(vec![arg(2)]),
    )
    .unwrap();
    let (_, demand) = a.array().unwrap();
    assert!(demand.validate_parent(b.admission()).is_err());
    assert!(
        b.admission()
            .reselect_array_length_current(&db, &view, demand, MAX_BYTES)
            .is_err()
    );
}
#[test]
fn changed_epoch_refuses_child_but_identical_discovery_republication_reuses_parent() {
    let (mut db, view, definition) = world(ARRAY_MISMATCH, "NUMBERS", 2024);
    let parent = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let (_, demand) = parent.array().unwrap();
    let revision = db.source_revision();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(revision, db.source_revision());
    assert!(
        parent
            .admission()
            .reselect_array_length_current(&db, &view, demand, MAX_BYTES)
            .is_err()
    );
    let (mut db, view, definition) = world(ARRAY_MISMATCH, "NUMBERS", 2024);
    let parent = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let (_, demand) = parent.array().unwrap();
    let stamps = db.source_view_namespace_stamps().unwrap();
    let old_revision = db.revision();
    let source_revision = db.source_revision();
    let scope = parent.admission().plan().owner_context().scope.clone();
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        scope,
        Namespace::Type,
        Digest::of_bytes(b"result type current world"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"result type declarative"),
        Digest::of_bytes(b"result type compiler"),
        Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &view.selection().outline.parse.profile,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    loop {
        match discovery.poll().unwrap() {
            SourceDiscoveryStep::Sealed(_) => break,
            SourceDiscoveryStep::NeedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                .admit_expansion(Arc::new(request.expand().unwrap()))
                .unwrap(),
            _ => panic!("exact already-published original replay prerequisite"),
        }
    }
    discovery.publish(&mut db).unwrap();
    assert_eq!(old_revision, db.revision());
    assert_eq!(source_revision, db.source_revision());
    assert_eq!(stamps, db.source_view_namespace_stamps().unwrap());
    parent.admission().revalidate(&db, &view).unwrap();
    let reused = parent
        .admission()
        .reselect_array_length_current(&db, &view, demand, MAX_BYTES)
        .unwrap();
    let fresh = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let (_, fresh_demand) = fresh.array().unwrap();
    let fresh_child = fresh
        .admission()
        .reselect_array_length_current(&db, &view, fresh_demand, MAX_BYTES)
        .unwrap();
    assert_eq!(reused.plan().identity(), fresh_child.plan().identity());
}
#[test]
fn source_copy_and_metadata_residual_cannot_invent_child_evidence() {
    let (db, view, definition) = world(ARRAY_MISMATCH, "NUMBERS", 2024);
    let parent = ready(&db, &view, definition, ConstantRequest::Constant).unwrap();
    let (_, demand) = parent.array().unwrap();
    let (_, copy, _) = world("const NUMBERS:[u8;3]=[1,2,3];fn main(){}", "NUMBERS", 2024);
    assert!(
        parent
            .admission()
            .reselect_array_length_current(&db, &copy, demand, MAX_BYTES)
            .is_err()
    );
    let before = db.trace.counts.clone();
    let err = parent
        .admission()
        .reselect_array_length_current(&db, &view, demand, parent.admission().metadata_bytes() - 1)
        .unwrap_err();
    assert_eq!(err.code, "LRR-E-METADATA-LIMIT");
    assert_eq!(db.trace.counts, before);
}
#[test]
fn source_independent_fn_header_length_reselects_from_actual_parent_evidence() {
    let (db, view, definition) = world("const fn f(x:u32)->[u8;2]{[1,2]}fn main(){}", "f", 2024);
    let parent = ready(
        &db,
        &view,
        definition,
        ConstantRequest::ConstCall(vec![arg(1)]),
    )
    .unwrap();
    let (_, demand) = parent.array().unwrap();
    let child = parent
        .admission()
        .reselect_array_length_current(&db, &view, demand, MAX_BYTES)
        .unwrap();
    child.revalidate(&db, &view).unwrap();
    assert_eq!(child.plan().owner_definition(), definition);
    assert_eq!(child.plan().range(), demand.range());
    let result = CurrentConstantResultType::capture_current(&db, &view, child, MAX_BYTES).unwrap();
    assert_eq!(result.scalar().unwrap().primitive(), PrimitiveType::Usize);
    assert!(db.trace.counts.is_empty());
}
