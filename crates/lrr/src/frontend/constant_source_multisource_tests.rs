//! Exact original multisource admission; no full-main pass assertion.
use super::*;
use crate::source::dependencies::SourceDependency;
const CROSS_ROOT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/cross-crate/const-fn-evaluated-cross-crate.rs"
);
const CROSS_PROVIDER: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/cross-crate/auxiliary/const-fn-evaluated-cross-crate.rs"
);
const REEXPORT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/reexport-from-multiple-namespaces.rs"
);
fn unit(text: &str, path: &str, key: Digest, module: &[&str], edition: u16) -> SourceUnitInput {
    SourceUnitInput {
        path: SourcePath::new(path).unwrap(),
        crate_key: key,
        module_path: module.iter().map(|s| (*s).into()).collect(),
        profile: ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
    }
}
fn universe(
    units: Vec<SourceUnitInput>,
    edges: Vec<SourceDependency>,
) -> (Database, Vec<PreparedSourceView>) {
    let selections: Vec<_> = units
        .iter()
        .map(|unit| SourceSelection {
            path: unit.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: unit.blob.content(),
                    profile: unit.profile.clone(),
                },
                crate_key: unit.crate_key,
                module_path: unit.module_path.clone(),
                cfg: unit.cfg.fingerprint(),
            },
        })
        .collect();
    let inputs =
        PreparedSourceInputs::refresh_with_dependencies(None, units, Default::default(), edges)
            .unwrap();
    let mut db = Database::default();
    inputs.publish(&mut db).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let views = selections
        .iter()
        .map(|selection| {
            let SourceSelectionOutcome::Selected(view) = inputs.select(selection).unwrap() else {
                panic!("actual selected original source absent")
            };
            view
        })
        .collect();
    (db, views)
}
fn definition(view: &PreparedSourceView, name: &str) -> Digest {
    view.outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == name)
        .unwrap()
        .def_key
        .fingerprint()
}
fn seal(
    db: &mut Database,
    views: &[&PreparedSourceView],
    target: &PreparedSourceView,
    definition: Digest,
    namespace: Namespace,
    world: Digest,
) {
    let scope = target
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.fingerprint() == definition)
        .unwrap()
        .scope
        .clone();
    let mut discovery = SourceDiscovery::new(
        views.iter().map(|v| v.share()).collect(),
        scope,
        namespace,
        world,
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR multisource declarative"),
        Digest::of_bytes(b"LRR multisource compiler"),
        Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &target.selection().outline.parse.profile,
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
            _ => panic!("current source namespace prerequisite not established"),
        }
    }
    discovery.publish(db).unwrap();
}
fn seal_both(
    db: &mut Database,
    views: &[&PreparedSourceView],
    target: &PreparedSourceView,
    definition: Digest,
) {
    for ns in [Namespace::Value, Namespace::Type] {
        seal(
            db,
            views,
            target,
            definition,
            ns,
            Digest::of_bytes(b"LRR multisource current world"),
        );
    }
}
fn capture(
    db: &Database,
    target: &PreparedSourceView,
    definition: Digest,
    views: &[&PreparedSourceView],
) -> Result<ConstantSourceAdmission, Diagnostic> {
    ConstantSourceAdmission::capture_current_in_sources(
        db,
        target,
        definition,
        ConstantRequest::Constant,
        usize::MAX,
        views,
    )
}
#[test]
fn unchanged_cross_crate_const_fn_retains_selected_original_provider_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let root = Digest::of_bytes(b"official cross crate root");
        let provider = Digest::of_bytes(b"official cross crate provider");
        let (mut db, views) = universe(
            vec![
                unit(CROSS_ROOT, "cross-root.rs", root, &[], edition),
                unit(CROSS_PROVIDER, "cross-provider.rs", provider, &[], edition),
            ],
            vec![SourceDependency {
                consumer: root,
                alias: "const_fn_evaluated_cross_crate".into(),
                provider,
            }],
        );
        let refs: Vec<_> = views.iter().collect();
        let def = definition(&views[1], "FOO");
        seal_both(&mut db, &refs, &views[1], def);
        assert!(
            ConstantSourceAdmission::capture_current(
                &db,
                &views[1],
                def,
                ConstantRequest::Constant,
                usize::MAX
            )
            .is_err()
        );
        let before = db.trace.events.len();
        let admitted = capture(&db, &views[1], def, &refs).unwrap();
        assert_eq!(admitted.plan().owner_definition(), def);
        assert_eq!(admitted.plan().profile().edition, edition);
        assert_eq!(
            evaluate(admitted.plan(), ConstLimits::default())
                .unwrap()
                .value()
                .scalar()
                .unwrap()
                .payload,
            1
        );
        admitted.revalidate(&db, &views[1]).unwrap();
        assert_eq!(before, db.trace.events.len());
        assert!(
            admitted
                .dependencies()
                .contains_key(&InputKey::DefinitionBody(def))
        );
        assert!(admitted.metadata_bytes() <= usize::MAX);
        // Exact original provider plan only. Root lib::FOO dispatch/whole main remains separate.
    }
}
#[test]
fn unchanged_inline_module_member_can_join_a_relevant_original_source_set() {
    for edition in [2015, 2018, 2021, 2024] {
        let key = Digest::of_bytes(b"original module source set");
        let (mut db, views) = universe(
            vec![
                unit(MODULE, "module.rs", key, &[], edition),
                unit(
                    CROSS_PROVIDER,
                    "aux.rs",
                    Digest::of_bytes(b"original module dependency"),
                    &[],
                    edition,
                ),
            ],
            vec![],
        );
        let refs: Vec<_> = views.iter().collect();
        let def = definition(&views[0], "BITS");
        seal_both(&mut db, &refs, &views[0], def);
        let a = capture(&db, &views[0], def, &refs).unwrap();
        assert_eq!(a.plan().owner_context().scope.module_path, vec!["u8"]);
        assert_eq!(
            evaluate(a.plan(), ConstLimits::default())
                .unwrap()
                .value()
                .scalar()
                .unwrap()
                .payload,
            8
        );
        a.revalidate(&db, &views[0]).unwrap();
    }
}
#[test]
fn unchanged_reexport_value_member_keeps_type_namespace_distinct() {
    for edition in [2015, 2018, 2021, 2024] {
        let key = Digest::of_bytes(b"official reexport source set");
        let (mut db, views) = universe(
            vec![
                unit(REEXPORT, "reexport.rs", key, &[], edition),
                unit(
                    CROSS_PROVIDER,
                    "aux.rs",
                    Digest::of_bytes(b"original reexport dependency"),
                    &[],
                    edition,
                ),
            ],
            vec![],
        );
        let refs: Vec<_> = views.iter().collect();
        let value = views[0]
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "A" && i.scope.module_path == ["value_ns"])
            .unwrap();
        let def = value.def_key.fingerprint();
        seal_both(&mut db, &refs, &views[0], def);
        let a = capture(&db, &views[0], def, &refs).unwrap();
        assert_eq!(a.plan().owner_definition(), def);
        assert_eq!(
            evaluate(a.plan(), ConstLimits::default())
                .unwrap()
                .value()
                .scalar()
                .unwrap()
                .payload,
            0
        );
        // Original declaration macro/use3 expansion remains a separate source prerequisite.
    }
}
fn external_world() -> (Database, Vec<PreparedSourceView>, Digest) {
    let key = Digest::of_bytes(b"exact external module source set");
    let (mut db, views) = universe(
        vec![
            unit("fn main(){}", "root.rs", key, &[], 2024),
            unit(
                "pub const X:u32=22;",
                "provider.rs",
                Digest::of_bytes(b"exact selected constant provider"),
                &[],
                2024,
            ),
        ],
        vec![],
    );
    let def = definition(&views[1], "X");
    let refs: Vec<_> = views.iter().collect();
    seal_both(&mut db, &refs, &views[1], def);
    (db, views, def)
}
#[test]
fn actual_selected_provider_scope_and_canonical_set_order_are_owned() {
    let (db, views, def) = external_world();
    let a = capture(&db, &views[1], def, &[&views[0], &views[1]]).unwrap();
    let b = capture(&db, &views[1], def, &[&views[1], &views[0]]).unwrap();
    assert_eq!(a.identity(), b.identity());
    assert_eq!(
        a.plan().owner_context().scope.crate_key,
        views[1].selection().outline.crate_key
    );
    assert!(a.plan().owner_context().scope.module_path.is_empty());
    a.revalidate(&db, &views[1]).unwrap();
}
#[test]
fn incomplete_extra_duplicate_and_wrong_snapshot_sources_refuse() {
    let (db, views, def) = external_world();
    let before = db.trace.events.len();
    assert!(capture(&db, &views[1], def, &[&views[1]]).is_err());
    assert!(capture(&db, &views[1], def, &[&views[0], &views[1], &views[0]]).is_err());
    let key = views[0].selection().outline.crate_key;
    let (_, copied) = universe(
        vec![
            unit("fn main(){let changed=1;}", "root.rs", key, &[], 2024),
            unit(
                "pub const X:u32=22;",
                "provider.rs",
                Digest::of_bytes(b"exact selected constant provider"),
                &[],
                2024,
            ),
        ],
        vec![],
    );
    assert!(capture(&db, &views[1], def, &[&copied[0], &views[1]]).is_err());
    let admitted = capture(&db, &views[1], def, &[&views[0], &views[1]]).unwrap();
    assert_eq!(copied[1].selection(), views[1].selection());
    assert!(admitted.revalidate(&db, &copied[1]).is_err());
    let key = views[0].selection().outline.crate_key;
    let (extra_db, extra) = universe(
        vec![
            unit("fn main(){}", "root.rs", key, &[], 2024),
            unit(
                "pub const X:u32=22;",
                "provider.rs",
                Digest::of_bytes(b"exact selected constant provider"),
                &[],
                2024,
            ),
            unit(
                "pub const Y:u8=1;",
                "extra.rs",
                Digest::of_bytes(b"exact extra source provider"),
                &[],
                2024,
            ),
        ],
        vec![],
    );
    // Even without relying on a namespace seal, actual prepared membership
    // proves that the third original cannot be erased by a supplied subset.
    assert!(
        retained_sources::RetainedConstantSources::capture(
            &extra_db,
            &extra[1],
            &[&extra[0], &extra[1]],
            &extra_db.source_view_namespace_stamps().unwrap(),
            usize::MAX
        )
        .is_err()
    );
    assert_eq!(before, db.trace.events.len());
}
#[test]
fn type_value_source_set_or_world_mismatch_never_mints_primitive_authority() {
    let key = Digest::of_bytes(b"type value world mismatch");
    let (mut db, views) = universe(
        vec![
            unit("fn main(){}", "root.rs", key, &[], 2024),
            unit(
                "pub const X:u32=22;",
                "provider.rs",
                Digest::of_bytes(b"exact selected constant provider"),
                &[],
                2024,
            ),
        ],
        vec![],
    );
    let def = definition(&views[1], "X");
    seal(
        &mut db,
        &[&views[0], &views[1]],
        &views[1],
        def,
        Namespace::Value,
        Digest::of_bytes(b"LRR multisource current world"),
    );
    seal(
        &mut db,
        &[&views[0], &views[1]],
        &views[1],
        def,
        Namespace::Type,
        Digest::of_bytes(b"different world"),
    );
    assert!(capture(&db, &views[1], def, &[&views[0], &views[1]]).is_err());
    // A real singleton Type seal can be published only for its own target view.
    let key = Digest::of_bytes(b"type value sources mismatch");
    let (mut db, views) = universe(
        vec![
            unit("const X:u32=1;", "one.rs", key, &[], 2024),
            unit(
                "pub const Y:u8=2;",
                "two.rs",
                Digest::of_bytes(b"type value source companion"),
                &[],
                2024,
            ),
        ],
        vec![],
    );
    let def = definition(&views[0], "X");
    seal(
        &mut db,
        &[&views[0], &views[1]],
        &views[0],
        def,
        Namespace::Value,
        Digest::of_bytes(b"LRR multisource current world"),
    );
    seal(
        &mut db,
        &[&views[0]],
        &views[0],
        def,
        Namespace::Type,
        Digest::of_bytes(b"LRR multisource current world"),
    );
    assert!(capture(&db, &views[0], def, &[&views[0], &views[1]]).is_err());
}
#[test]
fn imported_primitive_spelling_is_not_recovered_from_its_descriptor() {
    let key = Digest::of_bytes(b"source selected primitive shadow");
    let (mut db, views) = universe(
        vec![
            unit(
                "mod names{pub type u32=bool;}mod values{use super::names::u32;pub const X:u32=true;}fn main(){}",
                "root.rs",
                key,
                &[],
                2024,
            ),
            unit(
                CROSS_PROVIDER,
                "provider.rs",
                Digest::of_bytes(b"primitive shadow source companion"),
                &[],
                2024,
            ),
        ],
        vec![],
    );
    let refs: Vec<_> = views.iter().collect();
    let def = definition(&views[0], "X");
    seal_both(&mut db, &refs, &views[0], def);
    // Alias evaluation remains a precise type integration prerequisite.
    assert!(capture(&db, &views[0], def, &refs).is_err());
}
#[test]
fn derived_epoch_and_changed_original_compile_facts_invalidate_without_query_work() {
    let (mut db, views, def) = external_world();
    let a = capture(&db, &views[1], def, &[&views[0], &views[1]]).unwrap();
    let revision = db.source_revision();
    let before = db.trace.events.len();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(revision, db.source_revision());
    assert!(a.revalidate(&db, &views[1]).is_err());
    assert_eq!(before, db.trace.events.len());
    let (mut db, views, def) = external_world();
    let a = capture(&db, &views[1], def, &[&views[0], &views[1]]).unwrap();
    let mut facts = views[1].prepared_inputs().clone();
    facts.insert(
        InputKey::CompileEnv("changed".into()),
        InputValue::CompileEnv("yes".into()),
    );
    db.publish_inputs(facts).unwrap();
    assert!(a.revalidate(&db, &views[1]).is_err());
}
#[test]
fn stale_body_context_and_owning_edition_refuse_exact_replay() {
    let (mut db, views, def) = external_world();
    let a = capture(&db, &views[1], def, &[&views[0], &views[1]]).unwrap();
    let mut facts = views[1].prepared_inputs().clone();
    let InputValue::DefinitionBody(body) = facts.get_mut(&InputKey::DefinitionBody(def)).unwrap()
    else {
        panic!()
    };
    body.range.end -= 1;
    db.publish_inputs(facts).unwrap();
    assert!(a.revalidate(&db, &views[1]).is_err());
    let key = views[0].selection().outline.crate_key;
    let (_, changed) = universe(
        vec![
            unit("fn main(){}", "root.rs", key, &[], 2024),
            unit(
                "pub const X:u32=22;",
                "provider.rs",
                Digest::of_bytes(b"exact selected constant provider"),
                &[],
                2018,
            ),
        ],
        vec![],
    );
    assert!(a.revalidate(&db, &changed[1]).is_err());
}
#[test]
fn caller_supplied_metadata_budget_refusal_is_pure() {
    let (db, views, def) = external_world();
    let before = db.trace.events.len();
    let error = ConstantSourceAdmission::capture_current_in_sources(
        &db,
        &views[1],
        def,
        ConstantRequest::Constant,
        1,
        &[&views[0], &views[1]],
    )
    .unwrap_err();
    assert_eq!(error.code, "LRR-E-METADATA-LIMIT");
    assert_eq!(before, db.trace.events.len());
}
#[test]
fn singleton_api_remains_exact_for_unchanged_nested_original() {
    let (db, view, def) = world(NESTED, "X", 2024, true);
    let a = ConstantSourceAdmission::capture_current(
        &db,
        &view,
        def,
        ConstantRequest::Constant,
        usize::MAX,
    )
    .unwrap();
    let b = capture(&db, &view, def, &[&view]).unwrap();
    assert_eq!(a.identity(), b.identity());
    a.revalidate(&db, &view).unwrap();
}

#[test]
fn changed_cfg_and_declaration_context_never_reuse_source_authority() {
    let (mut db, views, def) = external_world();
    let a = capture(&db, &views[1], def, &[&views[0], &views[1]]).unwrap();
    let mut facts = views[1].prepared_inputs().clone();
    let InputValue::DeclarationContext(context) =
        facts.get_mut(&InputKey::DeclarationContext(def)).unwrap()
    else {
        panic!()
    };
    context.scope.module_path.clear();
    db.publish_inputs(facts).unwrap();
    assert!(a.revalidate(&db, &views[1]).is_err());
    let key = views[0].selection().outline.crate_key;
    let mut root = unit("fn main(){}", "root.rs", key, &[], 2024);
    let mut child = unit(
        "pub const X:u32=22;",
        "provider.rs",
        Digest::of_bytes(b"exact selected constant provider"),
        &[],
        2024,
    );
    root.cfg = CfgSet::default();
    child.cfg = CfgSet::default();
    let (_, changed) = universe(vec![root, child], vec![]);
    assert_ne!(
        changed[1].selection().outline.cfg,
        views[1].selection().outline.cfg
    );
    assert!(a.revalidate(&db, &changed[1]).is_err());
}
#[test]
fn complete_disclosed_seal_set_does_not_invent_an_uncaptured_import_target() {
    let key = Digest::of_bytes(b"unestablished imported constant source");
    let (db, views) = universe(
        vec![unit(
            "mod missing;use missing::A;const X:u32=1;",
            "root.rs",
            key,
            &[],
            2024,
        )],
        vec![],
    );
    let def = definition(&views[0], "X");
    assert!(capture(&db, &views[0], def, &[&views[0]]).is_err());
    // No manually constructed scope seal or provider execution to erase missing source.
    assert!(db.trace.events.is_empty());
}

#[test]
fn empty_original_records_require_exact_descriptor_fragment_and_epoch_before_query() {
    let (db, views, def) = original_empty_world(REEXPORT);
    let refs: Vec<_> = views.iter().collect();
    let admitted = capture(&db, &views[0], def, &refs).unwrap();
    let (key, input) = admitted
        .dependencies()
        .keys()
        .find_map(|key| {
            let InputKey::DiscoverySeal(seal) = key else {
                return None;
            };
            if !matches!(
                seal.domain,
                SealDomain::Scope {
                    namespace: Namespace::Value,
                    ..
                }
            ) {
                return None;
            }
            let (InputValue::DiscoverySeal(input), _) = db.published_input(key)? else {
                return None;
            };
            Some((seal, input))
        })
        .unwrap();
    assert_eq!(input.records.len(), 1);
    assert_eq!(input.declaration_fragments.len(), 1);
    let stamps = db.source_view_namespace_stamps().unwrap();
    let retained =
        RetainedConstantSources::capture(&db, &views[0], &refs, &stamps, usize::MAX).unwrap();
    super::super::empty_expansions::validate(&db, input, key, &retained).unwrap();
    let trace = db.trace.events.len();
    let mut changed = input.clone();
    changed.records[0].output = Digest::of_bytes(b"foreign expansion output");
    assert!(super::super::empty_expansions::validate(&db, &changed, key, &retained).is_err());
    changed = input.clone();
    changed.records.clear();
    assert!(super::super::empty_expansions::validate(&db, &changed, key, &retained).is_err());
    changed = input.clone();
    changed.records.push(input.records[0].clone());
    changed
        .declaration_fragments
        .push(input.declaration_fragments[0].clone());
    assert!(super::super::empty_expansions::validate(&db, &changed, key, &retained).is_err());
    let mut epoch = key.clone();
    epoch.epoch += 1;
    assert!(super::super::empty_expansions::validate(&db, input, &epoch, &retained).is_err());
    assert_eq!(db.trace.events.len(), trace);
}
fn original_empty_world(text: &str) -> (Database, Vec<PreparedSourceView>, Digest) {
    let key = Digest::of_bytes(b"actual original empty macro record admission");
    let (mut db, views) = universe(vec![unit(text, "reexport.rs", key, &[], 2024)], vec![]);
    let def = views[0]
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "A" && item.scope.module_path == ["value_ns"])
        .unwrap()
        .def_key
        .fingerprint();
    let refs: Vec<_> = views.iter().collect();
    seal_both(&mut db, &refs, &views[0], def);
    (db, views, def)
}
#[test]
fn reached_nonempty_generated_declaration_is_not_an_empty_original_grant() {
    // Adaptation is outside the unchanged original. The actual source macro
    // emits a declaration, so this closed empty-output lane must refuse it.
    let before = "pub macro A() {}";
    assert_eq!(REEXPORT.matches(before).count(), 1);
    let text = REEXPORT.replace(before, "pub macro A() { const Injected: u8 = 1; }");
    let (db, views, def) = original_empty_world(&text);
    let refs: Vec<_> = views.iter().collect();
    let trace = db.trace.events.len();
    assert!(capture(&db, &views[0], def, &refs).is_err());
    assert_eq!(db.trace.events.len(), trace);
}
