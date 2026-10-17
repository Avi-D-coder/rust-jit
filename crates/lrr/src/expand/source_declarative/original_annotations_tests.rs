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


