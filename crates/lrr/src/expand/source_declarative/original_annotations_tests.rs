//! Source-index component controls around the unchanged core macro source.
//! Explicit namespace inputs here exercise proof boundaries; they are not a
//! published SDK namespace, complete facet or runtime/full-fixture claim.
use super::*;
use crate::{
    expand::{source_declarative::original_annotations::OriginalAnnotationPlan, source_derives},
    frontend::macro_names::{MacroNamespaces, MacroPath},
};

fn original_core() -> (
    PreparedSourceView,
    SourceDeclarative,
    OriginalAnnotationPlan,
) {
    let package = fixture_root().join("library/core");
    let path = package.join("src/macros/mod.rs");
    let (mut manifest, unit) = manifest(&path, CORE, 2024, &package);
    let root = package.join("src/lib.rs").to_str().unwrap().to_owned();
    manifest.inputs.source_files.insert(
        root.clone(),
        Some(Digest::of_bytes(include_bytes!(
            "../../../../../fixtures/upstream/rustc-checkout/library/core/src/lib.rs"
        ))),
    );
    manifest.graph.units.get_mut(&unit).unwrap().source_path = root;
    let (mut input, mut selection) = input(unit, "src/macros/mod.rs", CORE, 2024);
    input.module_path = vec!["macros".into()];
    selection.outline.module_path = input.module_path.clone();
    let inputs = PreparedSourceInputs::refresh(None, vec![input], Default::default()).unwrap();
    let source = select(&inputs, &selection);
    let facet =
        SourceMacroExportFacet::capture(&manifest, unit, path.to_str().unwrap(), source.share())
            .unwrap();
    assert_eq!(facet.source().selection(), source.selection());
    let mut provider = SourceDeclarative::new(source.share()).unwrap();
    provider.compiler = Some(Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &source.selection().outline.parse.profile,
        )
        .unwrap(),
    ));
    let scan = source.declaration_inventory(Default::default()).unwrap();
    let plan = OriginalAnnotationPlan::capture(&source, Arc::clone(&scan)).unwrap();
    assert!(Arc::ptr_eq(plan.scan(), &scan));
    assert_eq!(plan.scan_identity(), scan.fingerprint());
    (source, provider, plan)
}

#[test]
fn unchanged_original_inert_macros_restore_exact_declarations_and_routes() {
    let (source, mut provider, plan) = original_core();
    let mut restored = BTreeSet::new();
    for item in &source.outline().items {
        if item.def_key.item_kind != crate::source::manifest::ItemKind::Macro {
            continue;
        }
        let Some(call) = crate::syntax::outline::potential_attribute_producer(item) else {
            continue;
        };
        let Some(proof) = source_derives::prepare_empty_with_annotations(
            &source,
            &provider,
            call.item_input,
            &MacroNamespaces::default(),
            Some(&plan),
        )
        .unwrap() else {
            continue;
        };
        assert_eq!(proof.item, item.def_key.fingerprint());
        provider
            .complete_original_inert_item(call.item_input, &proof)
            .unwrap();
        let definition = provider.macro_index().definition(proof.item).unwrap();
        assert_eq!(
            definition.defining_unit,
            source.outline().root_scope.crate_key
        );
        assert_eq!(definition.definition_edition, 2024);
        assert!(restored.insert(proof.item));
        // A consumed exact producer cannot publish a second declaration.
        assert!(
            provider
                .complete_original_inert_item(call.item_input, &proof)
                .is_err()
        );
    }
    assert!(
        !restored.is_empty(),
        "actual core inert declarations were not restored"
    );
    let dependencies = source
        .source_dependencies()
        .unwrap()
        .aliases_for(source.outline().root_scope.crate_key, 2024)
        .unwrap();
    let routes = SourceMacroRoutes::from_completed_index(
        source.outline(),
        provider.macro_index(),
        &dependencies,
        &[],
    )
    .unwrap();
    for (identity, ordinal, _) in provider.macro_index().restored_original_macros() {
        let item = &source.outline().items[ordinal];
        if item
            .header
            .attributes
            .iter()
            .any(|attribute| attribute.class == crate::syntax::outline::AttributeClass::MacroExport)
        {
            assert!(
                routes
                    .definitions()
                    .iter()
                    .any(|route| route.definition.identity == identity)
            );
        }
    }
}

#[test]
fn exact_original_meta_proof_waits_for_competing_namespace_and_rejects_other_source() {
    let (source, provider, plan) = original_core();
    let mut checked = 0;
    for item in &source.outline().items {
        if item.def_key.item_kind != crate::source::manifest::ItemKind::Macro {
            continue;
        }
        let Some(call) = crate::syntax::outline::potential_attribute_producer(item) else {
            continue;
        };
        let Some(proof) = source_derives::prepare_empty_with_annotations(
            &source,
            &provider,
            call.item_input,
            &MacroNamespaces::default(),
            Some(&plan),
        )
        .unwrap() else {
            continue;
        };
        let row = plan
            .rows(&source, call.item_input, proof.item)
            .unwrap()
            .unwrap()
            .next()
            .unwrap();
        let [crate::syntax::imports::ImportSegment::Name(name)] = row.segments.as_slice() else {
            continue;
        };
        let mut pending = MacroNamespaces::default();
        pending
            .pending(
                MacroPath::new(
                    source.outline().root_scope.crate_key,
                    item.scope
                        .module_path
                        .iter()
                        .map(|part| crate::expand::scopes::MacroName::parse(part, 2024).unwrap())
                        .chain(std::iter::once(
                            crate::expand::scopes::MacroName::parse(name, 2024).unwrap(),
                        ))
                        .collect(),
                )
                .unwrap(),
                Digest::of_bytes(b"unresolved actual attribute competitor control"),
            )
            .unwrap();
        assert!(
            source_derives::prepare_empty_with_annotations(
                &source,
                &provider,
                call.item_input,
                &pending,
                Some(&plan),
            )
            .unwrap()
            .is_none()
        );
        let (other, selection) = input(Digest([197; 32]), "src/macros/mod.rs", CORE, 2024);
        let inputs = PreparedSourceInputs::refresh(None, vec![other], Default::default()).unwrap();
        let other = select(&inputs, &selection);
        assert!(proof.original_macro(&other, call.item_input).is_err());
        assert!(plan.rows(&other, call.item_input, proof.item).is_err());
        checked += 1;
        break;
    }
    assert_eq!(checked, 1, "no unchanged core Meta proof exercised");
}
