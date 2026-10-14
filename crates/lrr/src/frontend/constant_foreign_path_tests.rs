//! Source-selected cross-unit original request; no evaluator/runtime acceptance.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::OutlineKey,
        memo::Database,
    },
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    frontend::{
        constant_path_selection::{ConstantPathOutcome, ConstantPathSelection},
        constant_source_admission::ConstantRequest,
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        dependencies::SourceDependency,
        manifest::Namespace,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{outline::BodyHandle, parse::ParseKey, spans::SourceRange},
};
use std::sync::Arc;
const LIMIT: usize = 32 * 1024 * 1024;
const ROOT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/cross-crate/const-fn-evaluated-cross-crate.rs"
);
const PROVIDER: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/cross-crate/auxiliary/const-fn-evaluated-cross-crate.rs"
);
const LENGTH_ROOT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/cross-crate/cross-crate-array-len.rs"
);
const LENGTH_PROVIDER: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/cross-crate/auxiliary/cross-crate-array-len.rs"
);
fn unit(text: &str, path: &str, key: Digest, edition: u16) -> SourceUnitInput {
    SourceUnitInput {
        path: SourcePath::new(path).unwrap(),
        crate_key: key,
        module_path: vec![],
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
        .map(|u| SourceSelection {
            path: u.path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: u.blob.content(),
                    profile: u.profile.clone(),
                },
                crate_key: u.crate_key,
                module_path: u.module_path.clone(),
                cfg: u.cfg.fingerprint(),
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
                panic!("genuine prepared original absent")
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
    name: &str,
    world: Digest,
) {
    let item = target
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == name)
        .unwrap();
    let mut discovery = SourceDiscovery::new(
        views.iter().map(|v| v.share()).collect(),
        item.scope.clone(),
        Namespace::Value,
        world,
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"foreign constant compile environment"),
        crate::expand::source_declarative::engine_version(),
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
                "actual source discovery incomplete: {} {}",
                diagnostic.code, diagnostic.message
            ),
            _ => panic!("actual source discovery requires unestablished authority"),
        }
    }
    discovery.publish(db).unwrap();
}
fn world(
    root: &str,
    provider: &str,
    alias: &str,
    root_edition: u16,
    provider_edition: u16,
) -> (Database, Vec<PreparedSourceView>) {
    let consumer = Digest::of_bytes(b"current constant root");
    let provider_key = Digest::of_bytes(b"current constant provider");
    let (mut db, views) = universe(
        vec![
            unit(root, "root.rs", consumer, root_edition),
            unit(provider, "provider.rs", provider_key, provider_edition),
        ],
        vec![SourceDependency {
            consumer,
            alias: alias.into(),
            provider: provider_key,
        }],
    );
    let refs: Vec<_> = views.iter().collect();
    let world = Digest::of_bytes(b"exact original constant world");
    seal(&mut db, &refs, &views[0], "main", world);
    seal(&mut db, &refs, &views[1], "FOO", world);
    (db, views)
}
fn site(view: &PreparedSourceView, text: &str, needle: &str) -> (BodyHandle, SourceRange) {
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .clone();
    let start = text.find(needle).unwrap();
    (
        body,
        SourceRange {
            start: start as u32,
            end: (start + needle.len()) as u32,
        },
    )
}
fn capture(
    db: &Database,
    views: &[PreparedSourceView],
    text: &str,
    needle: &str,
) -> Result<ConstantPathOutcome, Diagnostic> {
    let (body, range) = site(&views[0], text, needle);
    let refs: Vec<_> = views.iter().collect();
    ConstantPathSelection::capture_current(db, &views[0], &body, range, &refs, LIMIT)
}
fn selected(outcome: ConstantPathOutcome) -> ConstantPathSelection {
    let ConstantPathOutcome::Selected(selected) = outcome else {
        panic!("actual original constant not selected: {outcome:?}")
    };
    *selected
}
#[test]
fn unchanged_cross_crate_main_selects_actual_auxiliary_request_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, views) = world(
            ROOT,
            PROVIDER,
            "const_fn_evaluated_cross_crate",
            edition,
            edition,
        );
        let before = db.trace.events.len();
        let revision = db.revision();
        let selected = selected(capture(&db, &views, ROOT, "lib::FOO").unwrap());
        assert_eq!(selected.definition(), definition(&views[1], "FOO"));
        assert_ne!(
            selected.target().outline.crate_key,
            views[0].selection().outline.crate_key
        );
        assert_eq!(selected.target(), views[1].selection());
        assert_eq!(
            selected.context().scope.crate_key,
            views[1].selection().outline.crate_key
        );
        assert_eq!(
            selected.body(),
            views[1]
                .outline()
                .items
                .iter()
                .find(|i| i.def_key.declared_name == "FOO")
                .unwrap()
                .body
                .as_ref()
                .unwrap()
        );
        assert_eq!(selected.site().source(), views[0].selection());
        assert!(matches!(selected.request(), ConstantRequest::Constant));
        assert!(
            selected
                .dependencies()
                .contains_key(&InputKey::DefinitionBody(selected.definition()))
        );
        selected
            .revalidate(&db, &views[0], &[&views[0], &views[1]])
            .unwrap();
        assert_eq!(db.trace.events.len(), before);
        assert_eq!(db.revision(), revision);
    }
}
#[test]
fn unchanged_cross_crate_length_path_selects_source_without_evaluating_length() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, views) = world(
            LENGTH_ROOT,
            LENGTH_PROVIDER,
            "cross_crate_array_len",
            edition,
            edition,
        );
        let before = db.trace.events.len();
        let a = selected(capture(&db, &views, LENGTH_ROOT, "cross_crate_array_len::FOO").unwrap());
        assert_eq!(a.definition(), definition(&views[1], "FOO"));
        assert_eq!(a.target(), views[1].selection());
        a.revalidate(&db, &views[0], &[&views[1], &views[0]])
            .unwrap();
        assert_eq!(db.trace.events.len(), before);
    }
}
#[test]
fn selected_foreign_owner_retains_distinct_owning_editions() {
    for (root_edition, provider_edition) in [(2015, 2024), (2024, 2015), (2018, 2021), (2021, 2018)]
    {
        let (db, views) = world(
            ROOT,
            PROVIDER,
            "const_fn_evaluated_cross_crate",
            root_edition,
            provider_edition,
        );
        let a = selected(capture(&db, &views, ROOT, "lib::FOO").unwrap());
        assert_eq!(
            a.site().source().outline.parse.profile.edition,
            root_edition
        );
        assert_eq!(a.target().outline.parse.profile.edition, provider_edition);
        a.revalidate(&db, &views[0], &[&views[0], &views[1]])
            .unwrap();
    }
}
#[test]
fn actual_renamed_foreign_reexport_preserves_defining_member_not_import_owner() {
    // Supplemental route control; official unchanged extern alias is the positive witness above.
    let text = "extern crate const_fn_evaluated_cross_crate as lib;mod exported{pub use crate::lib::FOO as RENAMED;}fn main(){let _=exported::RENAMED;}";
    for edition in [2015, 2018, 2021, 2024] {
        let (db, views) = world(
            text,
            PROVIDER,
            "const_fn_evaluated_cross_crate",
            edition,
            edition,
        );
        let a = selected(capture(&db, &views, text, "exported::RENAMED").unwrap());
        assert_eq!(a.definition(), definition(&views[1], "FOO"));
        assert_eq!(a.target(), views[1].selection());
    }
}
#[test]
fn identical_spelling_in_another_retained_provider_never_selects_wrong_owner() {
    let root = Digest::of_bytes(b"current constant root");
    let right = Digest::of_bytes(b"right provider");
    let wrong = Digest::of_bytes(b"wrong provider");
    let (mut db, views) = universe(
        vec![
            unit(ROOT, "root.rs", root, 2024),
            unit(PROVIDER, "right.rs", right, 2024),
            unit(PROVIDER, "wrong.rs", wrong, 2024),
        ],
        vec![SourceDependency {
            consumer: root,
            alias: "const_fn_evaluated_cross_crate".into(),
            provider: right,
        }],
    );
    let refs: Vec<_> = views.iter().collect();
    let world = Digest::of_bytes(b"exact original constant world");
    seal(&mut db, &refs, &views[0], "main", world);
    seal(&mut db, &refs, &views[1], "FOO", world);
    seal(&mut db, &refs, &views[2], "FOO", world);
    let a = selected(capture(&db, &views, ROOT, "lib::FOO").unwrap());
    assert_eq!(a.definition(), definition(&views[1], "FOO"));
    assert_ne!(a.definition(), definition(&views[2], "FOO"));
    assert_eq!(a.target(), views[1].selection());
    assert!(
        a.revalidate(&db, &views[0], &[&views[0], &views[2]])
            .is_err()
    );
}
#[test]
fn published_original_source_missing_from_scope_subset_cannot_mint_selection() {
    let root = Digest::of_bytes(b"current constant root");
    let provider = Digest::of_bytes(b"current constant provider");
    let (mut db, views) = universe(
        vec![
            unit(ROOT, "root.rs", root, 2024),
            unit(PROVIDER, "provider.rs", provider, 2024),
            unit(
                LENGTH_PROVIDER,
                "unretained.rs",
                Digest::of_bytes(b"unretained original"),
                2024,
            ),
        ],
        vec![SourceDependency {
            consumer: root,
            alias: "const_fn_evaluated_cross_crate".into(),
            provider,
        }],
    );
    let refs = [&views[0], &views[1]];
    let world = Digest::of_bytes(b"exact original constant world");
    seal(&mut db, &refs, &views[0], "main", world);
    seal(&mut db, &refs, &views[1], "FOO", world);
    let (body, range) = site(&views[0], ROOT, "lib::FOO");
    let before = db.trace.events.len();
    let e = ConstantPathSelection::capture_current(&db, &views[0], &body, range, &refs, LIMIT)
        .unwrap_err();
    assert_eq!(e.code, "LRR-E-CONST-PATH-SOURCE");
    assert!(e.message.contains("complete current original source set"));
    assert_eq!(before, db.trace.events.len());
}
#[test]
fn missing_actual_foreign_scope_seal_refuses_before_request() {
    let root = Digest::of_bytes(b"current constant root");
    let provider = Digest::of_bytes(b"current constant provider");
    let (mut db, views) = universe(
        vec![
            unit(ROOT, "root.rs", root, 2024),
            unit(PROVIDER, "provider.rs", provider, 2024),
        ],
        vec![SourceDependency {
            consumer: root,
            alias: "const_fn_evaluated_cross_crate".into(),
            provider,
        }],
    );
    let refs: Vec<_> = views.iter().collect();
    seal(
        &mut db,
        &refs,
        &views[0],
        "main",
        Digest::of_bytes(b"exact original constant world"),
    );
    assert!(capture(&db, &views, ROOT, "lib::FOO").is_err());
}
#[test]
fn foreign_target_world_cannot_be_joined_to_current_caller() {
    let (mut db, views) = world(ROOT, PROVIDER, "const_fn_evaluated_cross_crate", 2024, 2024);
    let refs: Vec<_> = views.iter().collect();
    seal(
        &mut db,
        &refs,
        &views[1],
        "FOO",
        Digest::of_bytes(b"another selected world"),
    );
    let e = capture(&db, &views, ROOT, "lib::FOO").unwrap_err();
    // Both genuine same-epoch worlds remain published. The shared current
    // certificate reader refuses this ambiguity before the constant join.
    assert_eq!(e.code, "LRR-E-CALL-CONTEXT");
    assert_eq!(e.message, "lexical namespace seal selection is ambiguous");
}
#[test]
fn stale_derived_epoch_and_compile_membership_refuse_without_query_work() {
    let (mut db, views) = world(ROOT, PROVIDER, "const_fn_evaluated_cross_crate", 2024, 2024);
    let a = selected(capture(&db, &views, ROOT, "lib::FOO").unwrap());
    let revision = db.source_revision();
    let before = db.trace.events.len();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(revision, db.source_revision());
    assert!(
        a.revalidate(&db, &views[0], &[&views[0], &views[1]])
            .is_err()
    );
    assert_eq!(before, db.trace.events.len());
    let (mut db, views) = world(ROOT, PROVIDER, "const_fn_evaluated_cross_crate", 2024, 2024);
    let a = selected(capture(&db, &views, ROOT, "lib::FOO").unwrap());
    let mut facts = views[0].prepared_inputs().clone();
    facts.insert(
        InputKey::CompileEnv("changed".into()),
        InputValue::CompileEnv("yes".into()),
    );
    db.publish_inputs(facts).unwrap();
    assert!(
        a.revalidate(&db, &views[0], &[&views[0], &views[1]])
            .is_err()
    );
}
#[test]
fn copied_foreign_body_publication_and_wrong_owner_never_reuse_proof() {
    let (mut db, views) = world(ROOT, PROVIDER, "const_fn_evaluated_cross_crate", 2024, 2024);
    let a = selected(capture(&db, &views, ROOT, "lib::FOO").unwrap());
    let mut facts = views[0].prepared_inputs().clone();
    let def = definition(&views[1], "FOO");
    let InputValue::DefinitionBody(body) = facts.get_mut(&InputKey::DefinitionBody(def)).unwrap()
    else {
        panic!()
    };
    body.def_key = Digest::of_bytes(b"forged foreign body owner");
    db.publish_inputs(facts).unwrap();
    assert!(
        a.revalidate(&db, &views[0], &[&views[0], &views[1]])
            .is_err()
    );
}
#[test]
fn path_range_and_metadata_limits_refuse_without_provider_or_source_mutation() {
    let (db, views) = world(ROOT, PROVIDER, "const_fn_evaluated_cross_crate", 2024, 2024);
    let (body, range) = site(&views[0], ROOT, "lib::FOO");
    let refs = [&views[0], &views[1]];
    let before = db.trace.events.len();
    let revision = db.revision();
    let e =
        ConstantPathSelection::capture_current(&db, &views[0], &body, range, &refs, 1).unwrap_err();
    assert_eq!(e.code, "LRR-E-METADATA-LIMIT");
    let wrong = SourceRange {
        start: range.start + 1,
        ..range
    };
    assert!(
        ConstantPathSelection::capture_current(&db, &views[0], &body, wrong, &refs, LIMIT).is_err()
    );
    let a = selected(capture(&db, &views, ROOT, "lib::FOO").unwrap());
    let many = vec![&views[0]; 257];
    assert!(a.revalidate(&db, &views[0], &many).is_err());
    assert_eq!(db.trace.events.len(), before);
    assert_eq!(db.revision(), revision);
}

#[test]
fn reached_foreign_value_function_is_not_a_constant_request() {
    let provider = "pub fn FOO()->i32{unentered_missing_name}";
    let (db, views) = world(ROOT, provider, "const_fn_evaluated_cross_crate", 2024, 2024);
    let before = db.trace.events.len();
    let ConstantPathOutcome::OtherValueMember(member) =
        capture(&db, &views, ROOT, "lib::FOO").unwrap()
    else {
        panic!("a function source member must remain distinct from a constant")
    };
    assert_eq!(member.identity, definition(&views[1], "FOO"));
    assert_eq!(member.defining_unit, views[1].selection().outline.crate_key);
    assert_eq!(db.trace.events.len(), before);
}
