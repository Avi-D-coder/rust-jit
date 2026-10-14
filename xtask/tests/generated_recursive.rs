use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::parse::ParseKey,
};
fn source(text: &str) -> PreparedSourceView {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([241; 32]);
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
        panic!("source absent")
    };
    view
}

fn discover(v: &PreparedSourceView) -> SourceDiscovery {
    SourceDiscovery::new(
        vec![v.share()],
        v.outline().root_scope.clone(),
        lrr::source::manifest::Namespace::Value,
        Digest([245; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap()
}
fn expand_original(w: &mut SourceDiscovery) {
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("original demand absent")
    };
    w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
}
fn expand_generated(w: &mut SourceDiscovery) -> lrr::expand::generated_items::GeneratedItems {
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = w.poll().unwrap() else {
        panic!("enclosing generated demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    let items =
        lrr::expand::generated_items::GeneratedItems::admit(request.key(), record.clone(), 1)
            .unwrap();
    w.admit_expansion(record).unwrap();
    items
}
#[test]
fn recursive_generated_call_resolves_ancestor_body_and_records_all_outputs() {
    let v = source(
        "use made as selected;macro_rules! seed{()=>{macro_rules! leaf{()=>{pub fn made(){7_i64;}}}macro_rules! middle{()=>{leaf!();}}middle!();}}seed!();",
    );
    let mut w = discover(&v);
    expand_original(&mut w);
    let first = expand_generated(&mut w);
    assert!(!first.calls().is_empty());
    assert!(w.lookup("selected").is_err());
    let second = expand_generated(&mut w);
    assert_eq!(second.items()[0].name.as_ref().unwrap().text, "made");
    let SourceDiscoveryStep::Sealed(seal) = w.poll().unwrap() else {
        panic!("recursive producers did not seal")
    };
    let records = w.sealed_expansions().unwrap().collect::<Vec<_>>();
    assert_eq!(records.len(), 3);
    for record in records {
        assert!(
            seal.input_dependencies()
                .binary_search(&(
                    record.key().fingerprint(),
                    record.descriptor().fingerprint()
                ))
                .is_ok()
        );
    }
    let lrr::frontend::names::NameLookup::Defined(d) = w.lookup("selected").unwrap() else {
        panic!("recursive leaf import absent")
    };
    assert_eq!(d.identity, second.items()[0].key.fingerprint());
}
#[test]
fn recursive_generated_macro_binding_wakes_later_parent_call_and_original_use() {
    let v = source(
        "macro_rules! maker{()=>{macro_rules! next{()=>{}}}}macro_rules! seed{()=>{maker!();next!();}}seed!();next!();",
    );
    let mut w = discover(&v);
    expand_original(&mut w);
    let definitions = expand_generated(&mut w);
    assert_eq!(definitions.items()[0].name.as_ref().unwrap().text, "next");
    expand_generated(&mut w);
    expand_original(&mut w);
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    assert_eq!(w.sealed_expansions().unwrap().count(), 4);
}
#[test]
fn recursive_generated_import_waits_for_actual_child_header() {
    let v = source(
        "macro_rules! leaf{()=>{pub fn made(){}}}macro_rules! middle{()=>{use made as selected;leaf!();}}macro_rules! seed{()=>{middle!();}}seed!();",
    );
    let mut w = discover(&v);
    expand_original(&mut w);
    expand_generated(&mut w);
    assert!(w.lookup("selected").is_err());
    let leaf = expand_generated(&mut w);
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    let lrr::frontend::names::NameLookup::Defined(d) = w.lookup("selected").unwrap() else {
        panic!("recursive import absent")
    };
    assert_eq!(d.identity, leaf.items()[0].key.fingerprint());
}

#[test]
fn recursive_outward_bindings_preserve_parent_textual_order() {
    let v = source(
        "macro_rules! maker{()=>{macro_rules! next{()=>{pub fn first(){}}}}}macro_rules! seed{()=>{maker!();macro_rules! next{()=>{pub fn second(){}}}}}seed!();next!();",
    );
    let mut w = discover(&v);
    expand_original(&mut w);
    expand_generated(&mut w);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("later original call not ready")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    let items =
        lrr::expand::generated_items::GeneratedItems::admit(request.key(), record.clone(), 1)
            .unwrap();
    assert_eq!(items.items()[0].name.as_ref().unwrap().text, "second");
    w.admit_expansion(record).unwrap();
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
}
#[test]
fn recursive_generated_depth_limit_retains_exact_pending_demand() {
    let v = source("macro_rules! recur{()=>{recur!();}}recur!();");
    let mut w = discover(&v);
    expand_original(&mut w);
    for _ in 0..255 {
        expand_generated(&mut w);
    }
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = w.poll().unwrap() else {
        panic!("depth-boundary demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    assert_eq!(
        w.admit_expansion(record.clone()).unwrap_err().code,
        "LRR-E-DISCOVERY-LIMIT"
    );
    assert!(w.expansion_record(record.key()).is_none());
    let SourceDiscoveryStep::NeedGeneratedExpansion { request: again } = w.poll().unwrap() else {
        panic!("depth refusal lost pending demand")
    };
    assert!(std::sync::Arc::ptr_eq(&request, &again));
    assert!(w.sealed_expansions().is_err());
}
