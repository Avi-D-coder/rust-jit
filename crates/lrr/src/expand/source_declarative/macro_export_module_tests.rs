//! Shared capture/adoption controls using complete unchanged core and an
//! unchanged official auxiliary. No module/runtime namespace is activated.
use super::*;
use crate::{
    db::{
        dependency::InputKey,
        key::QueryKey,
        memo::Database,
        poll::{InputRequest, QueryPoll},
    },
    frontend::source_module_declarations::SourceModuleDeclaration,
    service::{
        expansion::{DemandContext, ExpansionCoordinator, PhaseIdentity},
        source_capture::SourceCapture,
    },
    source::{
        include_paths::{IncludeAuthorities, IncludeRoot, IncludeRootKind},
        macro_export_module::{MacroExportModuleAdoption, MacroExportModuleRequest},
        manifest::BodyKey,
        module_capture::ModuleObservation,
    },
};
pub(super) fn authorities(
    facet: &SourceMacroExportFacet,
    generation: Digest,
) -> IncludeAuthorities {
    IncludeAuthorities::new(
        facet.unit(),
        generation,
        [IncludeRoot {
            identity: Digest([253; 32]),
            path: facet.package_root().to_owned(),
            kind: IncludeRootKind::Source,
        }],
    )
    .unwrap()
}
fn owners(
    child_bytes: &[u8],
    child_path: &str,
) -> (
    Database,
    Arc<SourceMacroExportFacet>,
    Arc<SourceMacroExportFacet>,
) {
    let (mut manifest, unit, _, _, _) = super::role_tests::core_sources();
    let package = fixture_root().join("library/core");
    manifest.inputs.source_files.insert(
        package.join(child_path).to_str().unwrap().into(),
        Some(Digest::of_bytes(child_bytes)),
    );
    let (root_input, root_selection) = input(unit, "src/lib.rs", super::role_tests::ROOT, 2024);
    let (mut child_input, mut child_selection) = input(unit, child_path, child_bytes, 2024);
    child_input.module_path = vec!["macros".into()];
    child_selection.outline.module_path = child_input.module_path.clone();
    let inputs =
        PreparedSourceInputs::refresh(None, vec![root_input, child_input], BTreeMap::new())
            .unwrap();
    let root = Arc::new(
        SourceMacroExportFacet::capture(
            &manifest,
            unit,
            package.join("src/lib.rs").to_str().unwrap(),
            select(&inputs, &root_selection),
        )
        .unwrap(),
    );
    let child = Arc::new(
        SourceMacroExportFacet::capture(
            &manifest,
            unit,
            package.join(child_path).to_str().unwrap(),
            select(&inputs, &child_selection),
        )
        .unwrap(),
    );
    let mut db = Database::default();
    super::role_tests::publish(
        &mut db,
        &manifest,
        unit,
        &[Arc::clone(&root), Arc::clone(&child)],
    );
    db.begin_expansion_epoch(1).unwrap();
    (db, root, child)
}
fn declaration(parent: &SourceMacroExportFacet) -> SourceModuleDeclaration {
    SourceModuleDeclaration::from_captured(parent.source())
        .unwrap()
        .into_iter()
        .find(|declaration| declaration.request().name == "macros")
        .unwrap()
}
pub(super) fn adopt(
    db: &mut Database,
    request: &Arc<MacroExportModuleRequest>,
) -> Result<Arc<MacroExportModuleAdoption>, crate::core::diagnostic::Diagnostic> {
    let InputRequest::CaptureModule {
        request: capture_request,
        compile_environment,
    } = request.input()
    else {
        panic!("not shared module demand")
    };
    let capture = SourceCapture::open(
        authorities(request.parent(), request.parent().preparation()),
        BTreeMap::new(),
    )
    .unwrap();
    let captured = capture.capture_module(capture_request).unwrap();
    let context = DemandContext {
        request: request.identity(),
        session: Digest([254; 32]),
        unit: request.parent().unit(),
        revision: db.source_revision(),
        epoch: 1,
        source_generation: request.parent().preparation(),
        native_generation: None,
        compile_environment: *compile_environment,
    };
    let mut coordinator = ExpansionCoordinator::new(context.clone());
    let token = coordinator
        .issue(
            QueryKey::Outline(request.parent().source().selection().outline.clone()),
            PhaseIdentity {
                frame: request.identity(),
                body: BodyKey {
                    definition: capture_request.producer(),
                    exact_body_bytes: request.input().fingerprint(),
                },
                index: 0,
            },
            request.input().clone(),
        )
        .unwrap();
    coordinator.start(&context, &token).unwrap();
    coordinator
        .accept_module(&context, &token, &captured)
        .unwrap();
    coordinator
        .adopt_module(&context, &token, db, captured)
        .unwrap();
    match request.adopted(db) {
        QueryPoll::Ready(result) => result,
        other => panic!("adopted module not available: {other:?}"),
    }
}
#[test]
fn actual_core_child_uses_shared_unique_capture_and_retains_no_ordinary_source_grant() {
    let (mut db, parent, child) = owners(CORE, "src/macros/mod.rs");
    let request = MacroExportModuleRequest::prepare(
        &mut db,
        Arc::clone(&parent),
        None,
        Arc::clone(&child),
        declaration(&parent),
        &authorities(&parent, parent.preparation()),
    )
    .unwrap();
    let InputRequest::CaptureModule {
        request: capture_request,
        ..
    } = request.input()
    else {
        panic!("not shared module demand")
    };
    assert_eq!(
        capture_request
            .candidates()
            .iter()
            .map(|candidate| candidate.path().as_str())
            .collect::<Vec<_>>(),
        ["src/macros.rs", "src/macros/mod.rs"]
    );
    assert!(
        matches!(request.adopted(&mut db),QueryPoll::NeedInput(input) if input==*request.input())
    );
    assert_eq!(request.role_dependencies().count(), 4);
    assert!(request.exclusive_metadata_bytes() < 32 * 1024 * 1024);
    assert!(request.retained_admission_bytes() <= 320 * 1024 * 1024);
    let adopted = adopt(&mut db, &request).unwrap();
    let observations = adopted.capture().record().observations();
    assert!(matches!(observations[0], ModuleObservation::Absent));
    assert!(matches!(observations[1], ModuleObservation::Present(_)));
    assert_eq!(
        adopted
            .capture()
            .record()
            .selected()
            .unwrap()
            .blob()
            .bytes(),
        CORE
    );
    adopted.revalidate(&mut db).unwrap();
    assert!(adopted.retained_admission_bytes() <= 320 * 1024 * 1024);
    for source in [parent.source(), child.source()] {
        assert!(
            db.input(&InputKey::CapturedSource(
                source.selection().location_identity()
            ))
            .is_none()
        );
        assert!(
            db.input(&InputKey::SourceBlob(
                source.selection().outline.parse.content
            ))
            .is_none()
        );
        assert!(db.prepared_source(source.selection()).unwrap().is_none());
    }
    match db.fetch(
        &crate::db::provider::SyntaxProvider,
        QueryKey::Outline(child.source().selection().outline.clone()),
    ) {
        QueryPoll::Ready(Err(error)) => assert_eq!(error.message, "parse source absent"),
        other => panic!("module capture granted ordinary syntax authority: {other:?}"),
    }
    db.publish_macro_export_sources(&[]).unwrap();
    assert!(adopted.revalidate(&mut db).is_err());
}
#[test]
fn actual_same_scope_candidate_and_wrong_authority_do_not_mint_ancestry() {
    let (mut db, parent, child) = owners(CORE, "src/not-the-declared-module.rs");
    let error = MacroExportModuleRequest::prepare(
        &mut db,
        Arc::clone(&parent),
        None,
        child,
        declaration(&parent),
        &authorities(&parent, parent.preparation()),
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "configured macro-export child is not a declared source candidate"
    );
    let (mut db, parent, child) = owners(CORE, "src/macros/mod.rs");
    let error = MacroExportModuleRequest::prepare(
        &mut db,
        Arc::clone(&parent),
        None,
        child,
        declaration(&parent),
        &authorities(&parent, Digest([255; 32])),
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "macro-export module request preparation/source authority differs"
    );
}
#[test]
fn genuine_adopted_candidate_bytes_must_match_the_configured_child() {
    // The pure record may claim different unchanged official source at the
    // same path. Actual shared capture still observes real complete core bytes.
    let (mut db, parent, child) = owners(AUX, "src/macros/mod.rs");
    let request = MacroExportModuleRequest::prepare(
        &mut db,
        Arc::clone(&parent),
        None,
        child,
        declaration(&parent),
        &authorities(&parent, parent.preparation()),
    )
    .unwrap();
    let error = adopt(&mut db, &request).unwrap_err();
    assert_eq!(
        error.message,
        "macro-export module selected candidate differs from configured child"
    );
}
