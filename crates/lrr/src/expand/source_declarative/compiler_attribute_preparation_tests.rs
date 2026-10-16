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
fn actual_core_import_candidates_do_not_complete_attribute_producers() {
    let (source, mut provider, plan) = original_root();
    let strict = provider.routes_in(&[]).unwrap();
    let before = strict[0]
        .deferred()
        .iter()
        .filter(|row| row.reason == DeferredImportReason::AttributeProducer)
        .count();
    assert!(before > 0, "actual core attributed-import witness changed");
    let pending: BTreeSet<_> = provider.macro_index().pending_producers().collect();
    let retained = provider.prepare_compiler_attributes(&plan).unwrap();
    assert!(retained > 0);
    let candidates = provider.routes_in(&[]).unwrap();
    let after = candidates[0]
        .deferred()
        .iter()
        .filter(|row| row.reason == DeferredImportReason::AttributeProducer)
        .count();
    assert!(
        after < before,
        "no actual compiler-annotated import was prepared"
    );
    assert_eq!(
        pending,
        provider.macro_index().pending_producers().collect()
    );
    assert!(
        provider
            .require_final_compiler_attributes(&BTreeMap::new())
            .is_err()
    );
    // The strict constructor has no access to preparation authority.
    let (dependencies, available) = provider.route_dependency_sets().unwrap();
    let strict_again = SourceMacroRoutes::from_lookup_with_available(
        source.outline(),
        &dependencies,
        &available,
        &[],
        |id| provider.macro_index().definition(id).cloned(),
        |module, at, name| provider.macro_index().lookup(module, at, name),
    )
    .unwrap();
    assert_eq!(
        before,
        strict_again
            .deferred()
            .iter()
            .filter(|row| row.reason == DeferredImportReason::AttributeProducer)
            .count()
    );
}
#[test]
fn candidates_require_every_final_proof_and_strict_competing_attribute_reselection() {
    let (source, mut provider, plan) = original_root();
    provider.prepare_compiler_attributes(&plan).unwrap();
    let prepared = provider.compiler_attribute_preparation.as_ref().unwrap();
    let mut completed = BTreeMap::new();
    let mut checked_competitor = false;
    for item in &source.outline().items {
        if !prepared.permits(item) {
            continue;
        }
        let call = crate::syntax::outline::potential_attribute_producer(item).unwrap();
        let proof = source_derives::prepare_empty_with_annotations(
            &source,
            &provider,
            call.item_input,
            &MacroNamespaces::default(),
            Some(&plan),
        )
        .unwrap()
        .expect("candidate still requires actual final annotation selection");
        if !checked_competitor {
            let row = plan
                .rows(&source, call.item_input, proof.item)
                .unwrap()
                .unwrap()
                .next()
                .unwrap();
            let [crate::syntax::imports::ImportSegment::Name(name)] = row.segments.as_slice()
            else {
                panic!("qualified candidate");
            };
            let mut pending = MacroNamespaces::default();
            let path = item
                .scope
                .module_path
                .iter()
                .map(|part| crate::expand::scopes::MacroName::parse(part, 2024).unwrap())
                .chain(std::iter::once(
                    crate::expand::scopes::MacroName::parse(name, 2024).unwrap(),
                ))
                .collect();
            pending
                .pending(
                    MacroPath::new(item.scope.crate_key, path).unwrap(),
                    Digest([213; 32]),
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
            let mut competing = MacroNamespaces::default();
            let definition = provider
                .compiler
                .as_ref()
                .unwrap()
                .attribute_catalog()
                .unwrap()
                .definitions()
                .next()
                .unwrap()
                .clone();
            let path = item
                .scope
                .module_path
                .iter()
                .map(|part| crate::expand::scopes::MacroName::parse(part, 2024).unwrap())
                .chain(std::iter::once(
                    crate::expand::scopes::MacroName::parse(name, 2024).unwrap(),
                ))
                .collect();
            competing
                .define(
                    MacroPath::new(item.scope.crate_key, path).unwrap(),
                    definition,
                )
                .unwrap();
            assert!(
                source_derives::prepare_empty_with_annotations(
                    &source,
                    &provider,
                    call.item_input,
                    &competing,
                    Some(&plan),
                )
                .is_err()
            );
            checked_competitor = true;
        }
        completed.insert(call.item_input, proof);
    }
    assert!(checked_competitor);
    // Proof storage alone does not retire the actual index producer.
    assert!(
        provider
            .require_final_compiler_attributes(&completed)
            .is_err()
    );
    for (producer, proof) in &completed {
        provider
            .complete_original_inert_item(*producer, proof)
            .unwrap();
    }
    provider
        .require_final_compiler_attributes(&completed)
        .unwrap();
    completed.pop_first();
    assert!(
        provider
            .require_final_compiler_attributes(&completed)
            .is_err()
    );
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
