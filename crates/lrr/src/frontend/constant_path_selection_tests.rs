//! Nested under genuine current source fixture helper; no evaluator/guest execution.
use super::*;
use crate::frontend::constant_path_selection::{ConstantPathOutcome, ConstantPathSelection};
use crate::frontend::names::{NameLookup, NameNamespaces};
const REEXPORT: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/reexport-from-multiple-namespaces.rs"
);
const AMBIGUOUS: &str =
    include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/imports/issue-25396.rs");
const SHADOW_PATTERN: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/shadowed/match-binding-shadows-const.rs"
);
fn publish_target(db: &mut Database, view: &PreparedSourceView, name: &str) {
    let item = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == name)
        .unwrap();
    let profile = &view.selection().outline.parse.profile;
    let mut discovery = SourceDiscovery::new(
        vec![view.share()],
        item.scope.clone(),
        Namespace::Value,
        Digest::of_bytes(b"LRR constant source current world"),
        1,
    )
    .unwrap()
    .with_declarative_compiler(
        Digest::of_bytes(b"LRR constant declarative"),
        Digest::of_bytes(b"LRR constant compiler"),
        Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(profile)
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
            _ => panic!("target requires unestablished discovery authority"),
        }
    }
    discovery.publish(db).unwrap();
}
fn site(
    view: &PreparedSourceView,
    definition: Digest,
    text: &str,
    needle: &str,
) -> (crate::syntax::outline::BodyHandle, SourceRange) {
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.fingerprint() == definition)
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
#[test]
fn unchanged_module_constant_qualified_path_selects_exact_bits_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, definition) = world(MODULE, "NUM", edition, true);
        publish_target(&mut db, &view, "BITS");
        let (body, range) = site(&view, definition, MODULE, "u8::BITS");
        let before = db.trace.events.len();
        let ConstantPathOutcome::Selected(selected) =
            ConstantPathSelection::capture_current(&db, &view, &body, range, &[&view], usize::MAX)
                .unwrap()
        else {
            panic!("module constant was not selected")
        };
        let target = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "BITS")
            .unwrap();
        assert_eq!(selected.definition(), target.def_key.fingerprint());
        assert_eq!(selected.context().scope.module_path, vec!["u8"]);
        assert_eq!(selected.target().outline.parse.profile.edition, edition);
        assert_eq!(selected.body(), target.body.as_ref().unwrap());
        assert!(matches!(selected.request(), ConstantRequest::Constant));
        selected.revalidate(&db, &view, &[&view]).unwrap();
        assert_eq!(db.trace.events.len(), before);
        assert!(
            selected
                .dependencies()
                .contains_key(&InputKey::DefinitionBody(selected.definition()))
        );
    }
}
#[test]
fn source_import_reexport_keeps_value_member_separate_from_equal_type_names() {
    // Whole official main still needs the user macro expansion/source lane.
    // Namespace adapter exercises unchanged reexports without interpreting it.
    let profile = ProfileIdentity::current();
    let blob = SourceBlob::new(REEXPORT.as_bytes().to_vec()).unwrap();
    let parsed = crate::syntax::parse::ParsedFile::parse(&blob, profile).unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"official reexport namespaces"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let mut graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
    let resolution = graph.resolve_imports().unwrap();
    assert!(resolution.missing.is_empty());
    let value = graph
        .lookup_source_path(
            Namespace::Value,
            &[
                crate::syntax::imports::ImportSegment::Name("use2".into()),
                crate::syntax::imports::ImportSegment::Name("A".into()),
            ],
            false,
            &outline.root_scope,
            2024,
        )
        .unwrap();
    let ty = graph
        .lookup_source_path(
            Namespace::Type,
            &[
                crate::syntax::imports::ImportSegment::Name("use2".into()),
                crate::syntax::imports::ImportSegment::Name("A".into()),
            ],
            false,
            &outline.root_scope,
            2024,
        )
        .unwrap();
    let NameLookup::Defined(value) = value else {
        panic!("original Value reexport missing")
    };
    let NameLookup::Defined(ty) = ty else {
        panic!("original Type reexport missing")
    };
    assert_ne!(value.identity, ty.identity);
    assert_eq!(value.namespace, Namespace::Value);
    assert_eq!(ty.namespace, Namespace::Type);
    assert_eq!(
        value.identity,
        outline
            .items
            .iter()
            .find(|i| i.scope.module_path == vec!["value_ns"] && i.def_key.declared_name == "A")
            .unwrap()
            .def_key
            .fingerprint()
    );
}
#[test]
fn supplemental_renamed_original_member_has_a_current_sealed_request() {
    for edition in [2015, 2018, 2021, 2024] {
        let text = "mod source{pub const A:u8=0;}mod exported{pub use super::source::A as B;}fn main(){let mut value=exported::B;}";
        let (mut db, view, definition) = world(text, "main", edition, true);
        publish_target(&mut db, &view, "A");
        let (body, range) = site(&view, definition, text, "exported::B");
        let ConstantPathOutcome::Selected(selected) =
            ConstantPathSelection::capture_current(&db, &view, &body, range, &[&view], usize::MAX)
                .unwrap()
        else {
            panic!("renamed declaration missing")
        };
        assert_eq!(selected.context().scope.module_path, vec!["source"]);
        assert_eq!(
            selected.definition(),
            view.outline()
                .items
                .iter()
                .find(|i| i.def_key.declared_name == "A")
                .unwrap()
                .def_key
                .fingerprint()
        );
    }
}
#[test]
fn explicit_local_and_parameter_bindings_never_fall_through_to_module_constant() {
    for (text, name, needle) in [
        ("const C:u32=9;fn main(){let mut C=1;C;}", "main", "C;"),
        (
            "const C:u32=9;fn chosen(mut C:u32)->u32{C}fn main(){}",
            "chosen",
            "{C}",
        ),
    ] {
        let (db, view, definition) = world(text, name, 2024, true);
        let (body, mut range) = site(&view, definition, text, needle);
        if needle == "{C}" {
            range.start += 1;
            range.end -= 1;
        } else {
            range.end -= 1;
        }
        assert!(matches!(
            ConstantPathSelection::capture_current(&db, &view, &body, range, &[&view], usize::MAX)
                .unwrap(),
            ConstantPathOutcome::LocalBinding(_)
        ));
    }
}
#[test]
fn local_constant_without_owned_publication_remains_precise_lexical_prerequisite() {
    let text = "const C:u32=9;fn main(){const C:u32=1;let mut value=C;}";
    let (db, view, definition) = world(text, "main", 2024, true);
    let (body, mut range) = site(&view, definition, text, "value=C");
    range.start += 6;
    assert!(matches!(
        ConstantPathSelection::capture_current(&db, &view, &body, range, &[&view], usize::MAX)
            .unwrap(),
        ConstantPathOutcome::RequiredLexicalConstant(_)
    ));
}
#[test]
fn missing_and_associated_paths_never_become_absent_or_guessed_members() {
    for (text, needle) in [
        ("fn main(){let mut value=missing;}", "missing"),
        ("struct S;fn main(){let mut value=S::VALUE;}", "S::VALUE"),
    ] {
        let (db, view, definition) = world(text, "main", 2024, true);
        let (body, range) = site(&view, definition, text, needle);
        assert!(matches!(
            ConstantPathSelection::capture_current(&db, &view, &body, range, &[&view], usize::MAX)
                .unwrap(),
            ConstantPathOutcome::RequiredNamespace(_)
        ));
    }
}
#[test]
fn copied_source_and_derived_epoch_invalidate_exact_selection() {
    let (mut db, view, definition) = world(MODULE, "NUM", 2024, true);
    publish_target(&mut db, &view, "BITS");
    let (body, range) = site(&view, definition, MODULE, "u8::BITS");
    let ConstantPathOutcome::Selected(selected) =
        ConstantPathSelection::capture_current(&db, &view, &body, range, &[&view], usize::MAX)
            .unwrap()
    else {
        panic!()
    };
    let (_, other, _) = world("const NUM:usize=8;fn main(){}", "NUM", 2024, true);
    assert!(selected.revalidate(&db, &other, &[&other]).is_err());
    let revision = db.source_revision();
    db.begin_expansion_epoch(2).unwrap();
    assert_eq!(db.source_revision(), revision);
    assert!(selected.revalidate(&db, &view, &[&view]).is_err());
}
#[test]
fn current_path_source_range_and_quota_refuse_before_request_attachment() {
    let (mut db, view, definition) = world(MODULE, "NUM", 2024, true);
    publish_target(&mut db, &view, "BITS");
    let (body, range) = site(&view, definition, MODULE, "u8::BITS");
    let before = db.trace.events.len();
    assert!(ConstantPathSelection::capture_current(&db, &view, &body, range, &[&view], 1).is_err());
    let wrong = SourceRange {
        start: range.start + 1,
        end: range.end,
    };
    assert!(
        ConstantPathSelection::capture_current(&db, &view, &body, wrong, &[&view], usize::MAX)
            .is_err()
    );
    assert_eq!(db.trace.events.len(), before);
}
#[test]
fn original_ambiguous_imports_never_collapse_equal_spellings() {
    let profile = ProfileIdentity::current();
    let parsed = crate::syntax::parse::ParsedFile::parse(
        &SourceBlob::new(AMBIGUOUS.as_bytes().to_vec()).unwrap(),
        profile,
    )
    .unwrap();
    let outline = parsed
        .outline(
            Digest::of_bytes(b"official ambiguous imports"),
            vec![],
            &CfgSet::guest_default(),
        )
        .unwrap();
    let mut graph = NameNamespaces::from_outlines(&[&outline]).unwrap();
    let resolved = graph.resolve_imports().unwrap();
    assert!(!resolved.ambiguous.is_empty());
    assert!(matches!(
        graph
            .lookup_unqualified(Namespace::Value, "WOMP", &outline.root_scope, 2024, None)
            .unwrap(),
        NameLookup::Ambiguous(_)
    ));
}
#[test]
fn original_pattern_binding_is_not_a_path_expression_authority() {
    let (db, view, definition) = world(SHADOW_PATTERN, "main", 2024, true);
    let (body, mut range) = site(&view, definition, SHADOW_PATTERN, "mut C");
    range.start += 4;
    assert!(
        ConstantPathSelection::capture_current(&db, &view, &body, range, &[&view], usize::MAX)
            .is_err()
    );
}
#[test]
fn generic_const_parameter_cannot_be_replaced_by_equal_named_constant() {
    let text = "const N:usize=9;fn chosen<const N:usize>()->usize{N}fn main(){}";
    let (db, view, definition) = world(text, "chosen", 2024, true);
    let (body, mut range) = site(&view, definition, text, "{N}");
    range.start += 1;
    range.end -= 1;
    assert!(
        ConstantPathSelection::capture_current(&db, &view, &body, range, &[&view], usize::MAX)
            .is_err()
    );
}

#[test]
fn cfg_disabled_site_and_replaced_body_fact_cannot_select_a_request() {
    let text = "const C:u32=9;fn main(){#[cfg(any())]{let mut value=C;}}";
    let (db, view, definition) = world(text, "main", 2024, true);
    let (body, mut range) = site(&view, definition, text, "value=C");
    range.start += 6;
    assert!(
        ConstantPathSelection::capture_current(&db, &view, &body, range, &[&view], usize::MAX)
            .is_err()
    );
    let (mut db, view, definition) = world(MODULE, "NUM", 2024, true);
    publish_target(&mut db, &view, "BITS");
    let (_body, range) = site(&view, definition, MODULE, "u8::BITS");
    let mut facts = view.prepared_inputs().clone();
    let InputValue::DefinitionBody(body) = facts
        .get_mut(&InputKey::DefinitionBody(definition))
        .unwrap()
    else {
        panic!()
    };
    body.def_key = Digest::of_bytes(b"forged caller owner");
    db.publish_inputs(facts).unwrap();
    assert!(
        ConstantPathSelection::capture_current(
            &db,
            &view,
            &view
                .outline()
                .items
                .iter()
                .find(|i| i.def_key.fingerprint() == definition)
                .unwrap()
                .body
                .as_ref()
                .unwrap()
                .clone(),
            range,
            &[&view],
            usize::MAX
        )
        .is_err()
    );
}

#[test]
fn original_generic_local_route_is_data_and_cannot_mint_strict_constant_context() {
    let text = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/generics/generic-fn-infer.rs"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let (db, view, definition) = world(text, "id", edition, true);
        let (body, mut range) = site(&view, definition, text, "return x");
        range.start += 7;
        let parsed = db.prepared_parse(&view.selection().outline.parse).unwrap();
        let before = db.trace.events.len();
        let route = parsed
            .constant_path_route(&view, &body, range)
            .unwrap()
            .unwrap();
        assert_eq!(
            route.segments(),
            &[crate::syntax::imports::ImportSegment::Name("x".into())]
        );
        assert!(!route.absolute());
        assert_eq!(
            route.context().scope,
            view.outline()
                .items
                .iter()
                .find(|item| item.def_key.fingerprint() == definition)
                .unwrap()
                .scope
        );
        let error = parsed.constant_path_site(&view, &body, range).unwrap_err();
        assert_eq!(error.code, "LRR-E-CONST-PATH-SOURCE");
        assert!(error.message.contains("ParamEnv"));
        assert_eq!(db.trace.events.len(), before);
    }
}
