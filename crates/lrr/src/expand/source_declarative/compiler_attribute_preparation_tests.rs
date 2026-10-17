//! LRR scheduling/authority controls over unchanged actual core source. These
//! candidate graphs are not complete SDK namespaces or runtime fixture passes.
use super::*;
use crate::{
    db::memo::Database,
    expand::{source_declarative::original_annotations::OriginalAnnotationPlan, source_derives},
    frontend::{
        macro_names::{MacroNamespaces, MacroPath},
        source_macro_routes::DeferredImportReason,
    },
};
fn original_root() -> (
    PreparedSourceView,
    SourceDeclarative,
    OriginalAnnotationPlan,
) {
    let (manifest, unit, inputs, selection, _) = role_tests::core_sources();
    let source = select(&inputs, &selection);
    let facet = Arc::new(
        SourceMacroExportFacet::capture(
            &manifest,
            unit,
            manifest.graph.units[&unit].source_path.as_str(),
            source.share(),
        )
        .unwrap(),
    );
    let mut db = Database::default();
    role_tests::publish(&mut db, &manifest, unit, &[facet]);
    let observation = db
        .macro_export_source_observation(&selection, manifest.graph.fingerprint())
        .unwrap()
        .unwrap();
    let mut provider = SourceDeclarative::new(source.share()).unwrap();
    provider
        .bind_macro_export_source(Arc::new(observation), manifest.graph.fingerprint())
        .unwrap();
    provider.compiler = Some(Arc::new(
        crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
            &selection.outline.parse.profile,
        )
        .unwrap(),
    ));
    let scan = source.declaration_inventory(Default::default()).unwrap();
    let plan = OriginalAnnotationPlan::capture(&source, scan).unwrap();
    (source, provider, plan)
}
#[test]
fn wrong_source_and_changed_cfg_cannot_reuse_candidate_preparation() {
    let (source, mut provider, plan) = original_root();
    provider.prepare_compiler_attributes(&plan).unwrap();
    let prepared = provider.compiler_attribute_preparation.as_ref().unwrap();
    let (other_input, other_selection) =
        input(Digest([214; 32]), "src/lib.rs", role_tests::ROOT, 2024);
    let inputs = PreparedSourceInputs::refresh(None, vec![other_input], BTreeMap::new()).unwrap();
    let other = select(&inputs, &other_selection);
    assert!(plan.require_source(&other).is_err());
    assert!(prepared.require_outline(other.outline()).is_err());
    let mut changed = source.outline().clone();
    changed.cfg = Digest([215; 32]);
    assert!(prepared.require_outline(&changed).is_err());
    changed = source.outline().clone();
    changed.profile.edition = 2021;
    assert!(prepared.require_outline(&changed).is_err());
}
