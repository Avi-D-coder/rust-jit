//! Original source/expansion components only: no current constant query or main execution.
use super::*;
use crate::{
    core::profile::ProfileIdentity,
    db::key::OutlineKey,
    expand::source_declarative::SourceDeclarativeDemand,
    frontend::generated_macros::GeneratedMacroCatalog,
    source::{
        changes::{PreparedSourceInputs, SourceSelectionOutcome, SourceUnitInput},
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
const VIS: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-pub-matcher.rs"
);
const RENDER: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/rustdoc-html/macro/const-rendering-macros-33302.rs"
);
fn source(text: &str, edition: u16) -> PreparedSourceView {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let unit = Digest::of_bytes(b"generated constant source component");
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("generated-const-original.rs").unwrap();
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
    view
}
fn expand(text: &str, edition: u16, chosen: &str) -> (SourceDeclarative, GeneratedMacroCatalog) {
    let view = source(text, edition);
    let call = view
        .outline()
        .item_invocations
        .iter()
        .find(|row| row.range.start as usize == text.find(chosen).unwrap())
        .unwrap()
        .clone();
    let providers = vec![SourceDeclarative::new(view.share()).unwrap()];
    let routes = providers
        .iter()
        .flat_map(|p| p.routes_in(&providers).unwrap())
        .collect::<Vec<_>>();
    let graph =
        crate::frontend::source_macro_routes::SourceMacroRoutes::discovery_graph(&routes).unwrap();
    let SourceDeclarativeDemand::Ready(request) = providers[0]
        .demand_for_discovery(
            call.item_input,
            Digest::of_bytes(b"generated constant component compile environment"),
            crate::expand::source_declarative::engine_version(),
            &graph,
            &providers,
        )
        .unwrap()
    else {
        panic!("actual original source selection prerequisite")
    };
    let items =
        GeneratedItems::admit(request.key(), Arc::new(request.expand().unwrap()), 1).unwrap();
    let catalog = GeneratedMacroCatalog::new(
        &items,
        call.scope,
        Key(items.record().key().parent_context.0),
        view.captured_macro_cfg().unwrap(),
        1,
    )
    .unwrap();
    (providers.into_iter().next().unwrap(), catalog)
}
fn member(catalog: &GeneratedMacroCatalog, kind: GeneratedHeaderKind) -> Digest {
    catalog
        .captured_owner()
        .0
        .items()
        .iter()
        .find(|h| h.kind == kind)
        .unwrap()
        .key
        .fingerprint()
}
#[test]
fn unchanged_vis_const_retains_written_type_and_owner_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (owner, catalog) = expand(VIS, edition, "vis_passthru! { pub const A: i32 = 0; }");
        let definition = member(&catalog, GeneratedHeaderKind::Const);
        let proof =
            RetainedGeneratedConstant::capture(&owner, &catalog, definition, 1, MAX_BYTES).unwrap();
        proof.revalidate(&owner, &catalog, 1).unwrap();
        assert_eq!(
            proof.owner.selection().outline.parse.profile.edition,
            edition
        );
        assert_eq!(proof.scope.module_path, vec!["with_pub"]);
        assert!(matches!(proof.form(), GeneratedConstantForm::Scalar { .. }));
        assert_eq!(proof.record.key().call_edition, edition);
        assert_eq!(proof.record.key().definition_edition, edition);
        assert!(proof.written_type.spans().iter().any(
            |s| s.source.content.0 == owner.module_source().selection().outline.parse.content.0
        ));
        assert_eq!(
            proof.prerequisite(),
            GeneratedConstantPrerequisite::CurrentNamespace
        );
        // Retained syntax/provenance is deliberately not a Type or query proof.
    }
}
#[test]
fn unchanged_render_scalar_and_associated_array_keep_separate_origins() {
    let (owner, catalog) = expand(RENDER, 2024, "make!(4);");
    let (items, _, _) = catalog.captured_owner();
    let scalar = items
        .items()
        .iter()
        .find(|h| {
            h.kind == GeneratedHeaderKind::Const && h.name.as_ref().is_some_and(|n| n.text == "CST")
        })
        .unwrap();
    let proof = RetainedGeneratedConstant::capture(
        &owner,
        &catalog,
        scalar.key.fingerprint(),
        1,
        MAX_BYTES,
    )
    .unwrap();
    assert!(matches!(proof.form(), GeneratedConstantForm::Scalar { .. }));
    let parsed = ParsedFile::from_generated_items(items).unwrap();
    let written_array = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Const::cast)
        .find(|c| matches!(c.ty(), Some(ast::Type::ArrayType(_))))
        .unwrap();
    let written_range = range(written_array.syntax().text_range());
    let array = items
        .items()
        .iter()
        .find(|h| {
            h.kind == GeneratedHeaderKind::Const
                && h.start == written_range.start
                && h.end == written_range.end
        })
        .unwrap();
    let proof =
        RetainedGeneratedConstant::capture(&owner, &catalog, array.key.fingerprint(), 1, MAX_BYTES)
            .unwrap();
    let GeneratedConstantForm::Array { element, length } = proof.form() else {
        panic!()
    };
    assert!(!element.spans().is_empty());
    assert!(!length.spans().is_empty());
    assert!(
        length.spans().iter().any(
            |s| s.source.content.0 == owner.module_source().selection().outline.parse.content.0
        )
    );
    assert_eq!(
        proof.prerequisite(),
        GeneratedConstantPrerequisite::AssociatedMember
    );
    assert_ne!(length.range, proof.initializer.range);
}
#[test]
fn retained_member_source_epoch_and_quota_refuse_without_authority_upgrade() {
    let (owner, catalog) = expand(VIS, 2024, "vis_passthru! { pub const A: i32 = 0; }");
    let definition = member(&catalog, GeneratedHeaderKind::Const);
    let proof =
        RetainedGeneratedConstant::capture(&owner, &catalog, definition, 1, MAX_BYTES).unwrap();
    assert!(proof.revalidate(&owner, &catalog, 2).is_err());
    assert!(
        RetainedGeneratedConstant::capture(&owner, &catalog, Digest([97; 32]), 1, MAX_BYTES)
            .is_err()
    );
    assert!(RetainedGeneratedConstant::capture(&owner, &catalog, definition, 1, 0).is_err());
    let copied = SourceDeclarative::new(source(VIS, 2024)).unwrap();
    assert!(proof.revalidate(&copied, &catalog, 1).is_err());
}
#[test]
fn primitive_spelling_remains_a_type_prerequisite_when_source_shadows_it() {
    let text = "type i32=bool;macro_rules! make{()=>{const X:i32=true;}}make!();fn main(){}";
    let (owner, catalog) = expand(text, 2024, "make!();");
    let proof = RetainedGeneratedConstant::capture(
        &owner,
        &catalog,
        member(&catalog, GeneratedHeaderKind::Const),
        1,
        MAX_BYTES,
    )
    .unwrap();
    assert_eq!(
        proof.prerequisite(),
        GeneratedConstantPrerequisite::CurrentNamespace
    );
    // No primitive descriptor, evaluator result or source term is minted.
}
#[test]
fn unsealed_generated_definition_cannot_become_a_current_constant_request() {
    let (owner, catalog) = expand(VIS, 2024, "vis_passthru! { pub const A: i32 = 0; }");
    let db = Database::default();
    let result = CurrentGeneratedConstant::capture(
        &db,
        &owner,
        member(&catalog, GeneratedHeaderKind::Const),
        MAX_BYTES,
    );
    assert!(result.is_err());
}
fn current_world(
    text: &str,
    module: &[&str],
    edition: u16,
    seal_type: bool,
) -> (Database, SourceDeclarative, Digest) {
    let view = source(text, edition);
    let owner = SourceDeclarative::new(view.share()).unwrap();
    let mut scope = view.outline().root_scope.clone();
    scope
        .module_path
        .extend(module.iter().map(|s| (*s).to_string()));
    let mut db = Database::default();
    db.publish_inputs(view.prepared_inputs().clone()).unwrap();
    db.begin_expansion_epoch(1).unwrap();
    let mut definition = None;
    for namespace in [Namespace::Value, Namespace::Type] {
        if namespace == Namespace::Type && !seal_type {
            continue;
        }
        let mut discovery = crate::expand::source_discovery::SourceDiscovery::new(
            vec![view.share()],
            scope.clone(),
            namespace,
            Digest::of_bytes(b"generated constant current world"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"generated constant current compile environment"),
            crate::expand::source_declarative::engine_version(),
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
                crate::expand::source_discovery::SourceDiscoveryStep::Sealed(_) => break,
                crate::expand::source_discovery::SourceDiscoveryStep::NeedExpansion { request } => {
                    discovery
                        .admit_expansion(Arc::new(request.expand().unwrap()))
                        .unwrap()
                }
                crate::expand::source_discovery::SourceDiscoveryStep::NeedGeneratedExpansion {
                    request,
                } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                crate::expand::source_discovery::SourceDiscoveryStep::Incomplete {
                    diagnostic,
                    ..
                } => panic!(
                    "actual generated constant preparation frontier: {} {}",
                    diagnostic.code, diagnostic.message
                ),
                _ => panic!("actual generated constant source prerequisite is unestablished"),
            }
        }
        if namespace == Namespace::Value {
            let NameLookup::Defined(found) = discovery.lookup("A").unwrap() else {
                panic!("exact generated Value member absent")
            };
            definition = Some(found.identity);
        }
        discovery.publish(&mut db).unwrap();
    }
    (db, owner, definition.unwrap())
}
#[test]
fn unchanged_vis_current_source_pair_keeps_type_and_query_prerequisites_all_editions() {
    for edition in [2015, 2018, 2021, 2024] {
        let (db, owner, definition) = current_world(VIS, &["with_pub"], edition, true);
        let proof = CurrentGeneratedConstant::capture(&db, &owner, definition, MAX_BYTES).unwrap();
        proof.revalidate(&db, &owner).unwrap();
        assert_eq!(proof.retained().scope().module_path, vec!["with_pub"]);
        assert_eq!(proof.retained().member().fingerprint(), definition);
        assert!(
            proof
                .prerequisites()
                .contains(&GeneratedConstantPrerequisite::HygienicSourceType)
        );
        assert!(
            proof
                .prerequisites()
                .contains(&GeneratedConstantPrerequisite::ConstantQueryWorld)
        );
        assert!(
            proof
                .prerequisites()
                .contains(&GeneratedConstantPrerequisite::RuntimeLayout)
        );
        assert!(proof.dependencies().contains_key(&InputKey::ExpansionEpoch));
        assert!(proof.metadata_bytes() <= MAX_BYTES);
        assert_ne!(proof.seal_keys().0.domain, proof.seal_keys().1.domain);
    }
}
#[test]
fn current_missing_type_epoch_publication_copy_and_quota_refuse() {
    let (db, owner, definition) = current_world(VIS, &["with_pub"], 2024, false);
    assert!(CurrentGeneratedConstant::capture(&db, &owner, definition, MAX_BYTES).is_err());
    let (mut db, owner, definition) = current_world(VIS, &["with_pub"], 2024, true);
    let proof = CurrentGeneratedConstant::capture(&db, &owner, definition, MAX_BYTES).unwrap();
    assert!(CurrentGeneratedConstant::capture(&db, &owner, definition, 0).is_err());
    assert!(CurrentGeneratedConstant::capture(&db, &owner, Digest([98; 32]), MAX_BYTES).is_err());
    let copied = SourceDeclarative::new(source(VIS, 2024)).unwrap();
    assert!(proof.revalidate(&db, &copied).is_err());
    db.begin_expansion_epoch(2).unwrap(); // Actual between-run publication barrier.
    assert!(proof.revalidate(&db, &owner).is_err());
    assert!(CurrentGeneratedConstant::capture(&db, &owner, definition, MAX_BYTES).is_err());
}
#[test]
fn unchanged_external_and_local_generated_consts_require_reached_body_preparation() {
    for (text, needle, edition) in [
        (
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/const-external-macro-const-err.rs"
            ),
            "static_assert!(2 + 2 == 5)",
            2018,
        ),
        (
            include_str!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/block-to-expr-metavar.rs"
            ),
            "m_block!({ 1 })",
            2024,
        ),
    ] {
        let view = source(text, edition);
        let parsed = ParsedFile::parse(
            view.captured_macro_blob().unwrap(),
            view.selection().outline.parse.profile.clone(),
        )
        .unwrap();
        let selected = parsed
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter_map(ast::MacroCall::cast)
            .find(|call| {
                u32::from(call.syntax().text_range().start()) as usize == text.find(needle).unwrap()
            })
            .unwrap();
        let call = view
            .macro_call_at(range(selected.syntax().text_range()))
            .unwrap();
        assert!(call.requires_body_entry());
        assert_eq!(call.key(), &view.selection().outline.parse);
        assert!(
            !view
                .outline()
                .item_invocations
                .iter()
                .any(|i| i.range == call.range())
        );
        // The exact original body is cold: no local producer/constant query is fabricated.
    }
}
#[test]
fn current_exact_type_shadow_is_retained_without_primitive_authority() {
    let text = "type i32=bool;macro_rules!make{()=>{const A:i32=true;}}make!();fn main(){}";
    let (db, owner, definition) = current_world(text, &[], 2024, true);
    let proof = CurrentGeneratedConstant::capture(&db, &owner, definition, MAX_BYTES).unwrap();
    let Some((InputValue::DiscoverySeal(input), _)) =
        db.published_input(&InputKey::DiscoverySeal(proof.seal_keys().1.clone()))
    else {
        panic!()
    };
    let bucket = BucketKey {
        scope: proof.retained().scope().clone(),
        namespace: Namespace::Type,
        name: "i32".into(),
    };
    assert!(matches!(
        input.scope.as_ref().unwrap().lookup(&bucket).unwrap(),
        NameLookup::Defined(_)
    ));
    assert!(
        proof
            .prerequisites()
            .contains(&GeneratedConstantPrerequisite::HygienicSourceType)
    );
}
