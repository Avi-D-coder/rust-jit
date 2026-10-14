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
fn generated_call_resolves_captured_original_macro_body() {
    let v = source(
        "use made as selected;macro_rules! leaf{()=>{pub fn made(){7_i64;}}}macro_rules! seed{()=>{leaf!();}}seed!();",
    );
    let original = v
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "leaf")
        .unwrap()
        .def_key
        .fingerprint();
    let mut w = discover(&v);
    expand_original(&mut w);
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = w.poll().unwrap() else {
        panic!("enclosing definition demand absent")
    };
    assert_eq!(
        request.key().macro_identity,
        lrr::expand::identity::MacroIdentity::Declaration {
            definition: original
        }
    );
    let items = expand_generated(&mut w);
    assert_eq!(items.items()[0].name.as_ref().unwrap().text, "made");
    assert!(items.text().contains("7_i64"));
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    let lrr::frontend::names::NameLookup::Defined(d) = w.lookup("selected").unwrap() else {
        panic!("leaf import absent")
    };
    assert_eq!(d.identity, items.items()[0].key.fingerprint());
}
#[test]
fn generated_local_definition_shadows_enclosing_original_with_real_hygiene() {
    let v = source(
        "macro_rules! leaf{()=>{pub fn original(){}}}macro_rules! seed{()=>{macro_rules! leaf{()=>{pub fn generated(){}}}leaf!();}}seed!();",
    );
    let mut w = discover(&v);
    expand_original(&mut w);
    let items = expand_generated(&mut w);
    assert_eq!(items.items()[0].name.as_ref().unwrap().text, "generated");
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
}
#[test]
fn generated_call_cannot_see_original_definition_after_parent_invocation() {
    let v = source(
        "macro_rules! seed{()=>{late!();}}seed!();macro_rules! late{()=>{pub fn forbidden(){}}}",
    );
    let mut w = discover(&v);
    expand_original(&mut w);
    assert!(matches!(
        w.poll().unwrap(),
        SourceDiscoveryStep::Incomplete { .. }
    ));
    assert!(w.lookup("forbidden").is_err());
    assert!(w.sealed_expansions().is_err());
}
#[test]
fn generated_call_resolves_prior_outward_definition_and_retains_argument_tokens() {
    let v = source(
        "macro_rules! first{()=>{macro_rules! leaf{(7_i64)=>{pub fn made(){7_i64;}}}}}first!();macro_rules! seed{()=>{leaf!(7_i64);}}seed!();",
    );
    let mut w = discover(&v);
    expand_original(&mut w);
    expand_original(&mut w);
    let items = expand_generated(&mut w);
    assert_eq!(items.items()[0].name.as_ref().unwrap().text, "made");
    assert!(items.text().contains("7_i64"));
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    assert_eq!(w.sealed_expansions().unwrap().count(), 3);
}

#[test]
fn generated_call_uses_invocation_scope_after_macro_definition() {
    let v =
        source("macro_rules! seed{()=>{leaf!();}}macro_rules! leaf{()=>{pub fn made(){}}}seed!();");
    let mut w = discover(&v);
    expand_original(&mut w);
    let items = expand_generated(&mut w);
    assert_eq!(items.items()[0].name.as_ref().unwrap().text, "made");
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
}
#[test]
fn generated_call_keeps_inline_module_enclosing_source_chain() {
    let v = source(
        "macro_rules! leaf{()=>{pub fn made(){}}}mod inner{use made as selected;macro_rules! seed{()=>{leaf!();}}seed!();}",
    );
    let scope = lrr::syntax::outline::ScopeKey {
        crate_key: v.outline().root_scope.crate_key,
        module_path: vec!["inner".into()],
        owner: None,
    };
    let mut w = SourceDiscovery::new(
        vec![v.share()],
        scope,
        lrr::source::manifest::Namespace::Value,
        Digest([245; 32]),
        1,
    )
    .unwrap()
    .with_declarative(Digest([243; 32]), Digest([244; 32]))
    .unwrap();
    expand_original(&mut w);
    let items = expand_generated(&mut w);
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    let lrr::frontend::names::NameLookup::Defined(d) = w.lookup("selected").unwrap() else {
        panic!("inline module leaf header not published in captured owner scope")
    };
    assert_eq!(d.identity, items.items()[0].key.fingerprint());
}

#[test]
fn malformed_enclosing_rules_remain_lazy_until_explicit_generated_execution() {
    let v = source("macro_rules! leaf{bad}macro_rules! seed{()=>{leaf!();}}seed!();");
    let mut w = discover(&v);
    expand_original(&mut w);
    let SourceDiscoveryStep::NeedGeneratedExpansion { request } = w.poll().unwrap() else {
        panic!("malformed enclosing definition was expanded during resolution")
    };
    assert_eq!(request.expand().unwrap_err().code, "LRR-E-MACRO-EXPANSION");
    let SourceDiscoveryStep::NeedGeneratedExpansion { request: again } = w.poll().unwrap() else {
        panic!("failed explicit execution lost pending demand")
    };
    assert!(std::sync::Arc::ptr_eq(&request, &again));
    assert!(w.sealed_expansions().is_err());
}
