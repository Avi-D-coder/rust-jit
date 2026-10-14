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

fn expand_one(w: &mut SourceDiscovery) -> lrr::expand::generated_items::GeneratedItems {
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("expansion demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    let items =
        lrr::expand::generated_items::GeneratedItems::admit(request.key(), record.clone(), 1)
            .unwrap();
    w.admit_expansion(record).unwrap();
    items
}
#[test]
fn earlier_macro_generates_import_and_macro_for_later_original_invocation() {
    let v = source(
        "macro_rules! seed{()=>{use emitted as selected;macro_rules! next{()=>{pub fn emitted(){7_i64;}}}}}seed!();next!();",
    );
    let mut w = discover(&v);
    let first = expand_one(&mut w);
    let macro_id = first
        .items()
        .iter()
        .find(|h| h.name.as_ref().is_some_and(|n| n.text == "next"))
        .unwrap()
        .key
        .fingerprint();
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("generated binding did not wake later invocation")
    };
    assert_eq!(
        request.key().macro_identity,
        lrr::expand::identity::MacroIdentity::Declaration {
            definition: macro_id
        }
    );
    let record = std::sync::Arc::new(request.expand().unwrap());
    let second =
        lrr::expand::generated_items::GeneratedItems::admit(request.key(), record.clone(), 1)
            .unwrap();
    w.admit_expansion(record).unwrap();
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    let lrr::frontend::names::NameLookup::Defined(d) = w.lookup("selected").unwrap() else {
        panic!("generated import target absent")
    };
    assert_eq!(d.identity, second.items()[0].key.fingerprint());
    assert_eq!(w.sealed_expansions().unwrap().count(), 2);
}
#[test]
fn outward_generated_macro_definitions_preserve_emitted_textual_order() {
    let v = source(
        "macro_rules! seed{()=>{macro_rules! next{()=>{pub fn first(){}}}macro_rules! next{()=>{pub fn second(){}}}}}seed!();next!();",
    );
    let mut w = discover(&v);
    expand_one(&mut w);
    let result = expand_one(&mut w);
    assert_eq!(result.items()[0].name.as_ref().unwrap().text, "second");
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
}
#[test]
fn later_original_definition_shadows_outward_generated_binding() {
    let v = source(
        "macro_rules! seed{()=>{macro_rules! next{()=>{pub fn generated(){}}}}}seed!();macro_rules! next{()=>{pub fn original(){}}}next!();",
    );
    let mut w = discover(&v);
    expand_one(&mut w);
    let result = expand_one(&mut w);
    assert_eq!(result.items()[0].name.as_ref().unwrap().text, "original");
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
}
#[test]
fn malformed_outward_generated_rules_remain_lazy_until_later_demand() {
    let v = source("macro_rules! seed{()=>{macro_rules! next{bad}}}seed!();next!();");
    let mut w = discover(&v);
    expand_one(&mut w);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("generated demand absent")
    };
    assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-EXPANSION");
    assert!(w.lookup("absent").is_err());
}
#[test]
fn inactive_outward_macro_does_not_become_a_ready_definition() {
    let v =
        source("macro_rules! seed{()=>{#[cfg(any())]macro_rules! next{()=>{}}}}seed!();next!();");
    let mut w = discover(&v);
    expand_one(&mut w);
    assert!(matches!(
        w.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(w.lookup("absent").is_err());
}
