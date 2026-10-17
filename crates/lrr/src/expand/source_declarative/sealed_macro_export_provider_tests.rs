//! Positive owning-transport controls appended to the existing genuine AUX and
//! required full-core source tests. No source original or positive is weakened.
use super::*;
use crate::db::{key::QueryKey, provider::SyntaxProvider};
fn ready<T>(poll: QueryPoll<T>) -> T {
    match poll {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => {
            panic!("owning sealed source provider authority: {error:?}")
        }
        QueryPoll::NeedInput(input) => panic!("owning sealed source provider input: {input:?}"),
    }
}
fn unchanged(
    discovery: &mut SourceDiscovery,
    db: &mut Database,
    facet: &Arc<SourceMacroExportFacet>,
    count: usize,
) {
    assert_eq!(discovery.sealed_expansions().unwrap().len(), count);
    assert!(matches!(
        discovery.poll().unwrap(),
        SourceDiscoveryStep::Sealed(_)
    ));
    let view = ready(discovery.completed_macro_export_facet(db, facet));
    ready(view.revalidate(db));
}
pub(super) fn exercise(
    db: &mut Database,
    mut discovery: SourceDiscovery,
    facets: &[Arc<SourceMacroExportFacet>],
) {
    // Observe this transport scope; fixture preparation above remains separate.
    db.trace.detailed = true;
    let selected = facets
        .iter()
        .find(|facet| {
            discovery.key().domain
                == crate::db::seals::SealDomain::Scope {
                    id: facet.source().outline().root_scope.fingerprint(),
                    namespace: Namespace::Macro,
                }
        })
        .expect("actual selected facet scope");
    let count = discovery.sealed_expansions().unwrap().len();
    let view = ready(discovery.completed_macro_export_facet(db, selected));
    let provider =
        view.provider() as *const crate::expand::source_declarative::SourceDeclarative as usize;
    let seal = view.seal().key();
    drop(view);
    // Actual role preparation receipts/environment in a second database, with
    // no discovery publication. NeedInput cannot drain the complete source.
    let mut base = BTreeMap::new();
    let key = InputKey::SourceDependencies;
    base.insert(key.clone(), db.input(&key).unwrap().as_ref().clone());
    for facet in facets {
        let key = InputKey::UnitCompileEnv(facet.unit());
        base.insert(key.clone(), db.input(&key).unwrap().as_ref().clone());
    }
    let mut unpublished = Database::default();
    unpublished.publish_inputs(base).unwrap();
    unpublished.begin_expansion_epoch(seal.epoch).unwrap();
    unpublished.publish_macro_export_sources(facets).unwrap();
    match discovery.take_completed_macro_export_providers(&mut unpublished, facets) {
        QueryPoll::NeedInput(InputRequest::Discover { key }) => assert_eq!(key, seal),
        other => panic!("owning transport must demand genuine complete publication: {other:?}"),
    }
    unchanged(&mut discovery, db, selected, count);
    let mut duplicate = facets.to_vec();
    if duplicate.len() == 1 {
        duplicate.push(Arc::clone(&duplicate[0]));
    } else {
        let last = duplicate.len() - 1;
        duplicate[last] = Arc::clone(&duplicate[0]);
    }
    assert!(matches!(
        discovery.take_completed_macro_export_providers(db, &duplicate),
        QueryPoll::Ready(Err(_))
    ));
    unchanged(&mut discovery, db, selected, count);
    let error = match discovery.take_completed_macro_export_providers_with_limit(db, facets, 0) {
        QueryPoll::Ready(Err(error)) => error,
        other => panic!("zero ownership quota must refuse without consuming: {other:?}"),
    };
    assert_eq!(error.code, "LRR-E-RUNTIME-RESOURCE");
    unchanged(&mut discovery, db, selected, count);
    let owned = ready(discovery.take_completed_macro_export_providers(db, facets));
    assert_eq!(owned.seal(), &seal);
    assert_eq!(owned.facets().len(), facets.len());
    assert_eq!(owned.providers().len(), facets.len());
    assert_eq!(owned.records().len(), count);
    assert!(owned.metadata_bytes() > std::mem::size_of_val(owned.as_ref()));
    assert_eq!(
        owned.engine(),
        crate::expand::source_declarative::engine_version()
    );
    assert!(
        owned
            .providers()
            .iter()
            .any(|actual| actual as *const _ as usize == provider),
        "actual provider Vec storage must move, not deepclone"
    );
    for refusal in [
        discovery.poll().map(|_| ()),
        discovery.publish(db),
        discovery.sealed_declaration_expansions().map(|_| ()),
        discovery.impl_header_inventory().map(|_| ()),
        discovery.lookup("assert_eq").map(|_| ()),
    ] {
        assert_eq!(refusal.unwrap_err().code, "LRR-E-MACRO-EXPORT-CONSUMED");
    }
    assert!(
        matches!(discovery.take_completed_macro_export_providers(db, facets), QueryPoll::Ready(Err(error)) if error.code == "LRR-E-MACRO-EXPORT-CONSUMED")
    );
    drop(discovery);
    owned.require_current(db).unwrap();
    let trace = db.trace.events.len();
    owned.require_current(db).unwrap();
    assert_eq!(
        db.trace.events.len(),
        trace,
        "owning currentness uses cached input stamps only"
    );
    for facet in facets {
        let source = facet.source().selection();
        assert!(
            db.input(&InputKey::CapturedSource(source.location_identity()))
                .is_none()
        );
        assert!(
            db.input(&InputKey::SourceBlob(source.outline.parse.content))
                .is_none()
        );
        assert!(db.prepared_source(source).unwrap().is_none());
    }
    assert!(
        db.trace
            .events
            .iter()
            .all(|event| !matches!(event.key, QueryKey::BodyStructure(_) | QueryKey::Thunk(_)))
    );
    assert_eq!(
        db.trace.truncated, 0,
        "transport trace must remain complete"
    );
    let current = ready(db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(seal.clone())));
    assert_eq!(current.fingerprint, owned.seal_value());
    db.begin_expansion_epoch(seal.epoch.checked_add(1).unwrap())
        .unwrap();
    assert!(owned.require_current(db).is_err());
}
