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
#[test]
fn rustc_empty_derive_retires_without_native_catalog_and_keeps_original_item() {
    let original = include_str!(
        "../../fixtures/upstream/rustc-checkout/tests/ui/derives/deriving-meta-empty-trait-list.rs"
    );
    // Adaptation stays outside the licensed source; this exercises discovery only.
    let v = source(&format!(
        "macro_rules! make {{()=>{{{original}}};}} make!();"
    ));
    let mut w = discover(&v)
        .with_compiler(std::sync::Arc::new(
            lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &ProfileIdentity::current(),
            )
            .unwrap(),
        ))
        .unwrap();
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("declarative producer absent")
    };
    w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    assert_eq!(w.accounted_expansion_count().unwrap(), 1);
    assert_eq!(w.retained_derive_output_count().unwrap(), 0);
    assert!(matches!(
        w.lookup("_Bar").unwrap(),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
}

#[test]
fn compiler_empty_groups_and_known_or_disabled_member_attributes_settle() {
    for item in [
        "#[derive()] #[derive()] struct Keep;",
        "#[derive()] struct Keep {#[doc=\"retained\"] field:u8}",
        "#[derive()] struct Keep {#[cfg(any())] #[missing] field:u8}",
    ] {
        let v = source(&format!("macro_rules! make {{()=>{{{item}}};}} make!();"));
        let mut w = discover(&v)
            .with_compiler(std::sync::Arc::new(
                lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                    &ProfileIdentity::current(),
                )
                .unwrap(),
            ))
            .unwrap();
        let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
            panic!("declarative producer absent")
        };
        w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            .unwrap();
        assert!(
            matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)),
            "did not settle: {item}"
        );
        assert_eq!(w.accounted_expansion_count().unwrap(), 1);
        assert_eq!(w.retained_derive_output_count().unwrap(), 0);
    }
}

#[test]
fn empty_derive_keeps_unknown_outer_member_and_qualified_annotations_pending() {
    for item in [
        "#[derive()] #[missing] struct Keep;",
        "#[derive()] struct Keep {#[missing] field:u8}",
        "#[other::derive()] struct Keep;",
    ] {
        let v = source(&format!("macro_rules! make {{()=>{{{item}}};}} make!();"));
        let mut w = discover(&v)
            .with_compiler(std::sync::Arc::new(
                lrr::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                    &ProfileIdentity::current(),
                )
                .unwrap(),
            ))
            .unwrap();
        let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
            panic!("declarative producer absent")
        };
        w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            .unwrap();
        assert!(
            matches!(w.poll().unwrap(), SourceDiscoveryStep::Incomplete { .. }),
            "unexpected completion: {item}"
        );
        assert!(w.sealed_expansions().is_err());
        assert!(w.lookup("Keep").is_err());
    }
}

#[test]
fn source_discovery_admits_exact_expansion_and_wakes_original_import() {
    let v = source("use made as selected;macro_rules! make{()=>{pub fn made(){7_i64;}}}make!();");
    let mut w = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("expansion demand absent")
    };
    assert!(w.lookup("selected").is_err());
    let record = std::sync::Arc::new(request.expand().unwrap());
    let g = lrr::expand::generated_items::GeneratedItems::admit(request.key(), record.clone(), 1)
        .unwrap();
    let id = g.items()[0].key.fingerprint();
    w.admit_expansion(record).unwrap();
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    let lrr::frontend::names::NameLookup::Defined(d) = w.lookup("selected").unwrap() else {
        panic!("generated import target absent")
    };
    assert_eq!(d.identity, id);
}
#[test]
fn source_discovery_completes_repeated_empty_producers_without_clearing_shared_frontier_early() {
    let v = source("macro_rules! noop{()=>{}}noop!();noop!();pub fn visible(){}");
    let mut w = discover(&v);
    let mut ids = std::collections::BTreeSet::new();
    for _ in 0..2 {
        let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
            panic!("next producer absent")
        };
        assert!(ids.insert(request.key().invocation));
        w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
            .unwrap();
        assert!(w.lookup("visible").is_err());
    }
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    assert!(matches!(
        w.lookup("visible").unwrap(),
        lrr::frontend::names::NameLookup::Defined(_)
    ));
}
#[test]
fn source_discovery_rejects_substituted_record_and_post_seal_admission() {
    let v = source("macro_rules! make{()=>{pub fn made(){}}}make!();");
    let mut w = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    let mut wrong = request.key().clone();
    wrong.consumer_compile_env = Digest([249; 32]);
    let bad = std::sync::Arc::new(
        lrr::expand::record::ExpansionRecord::from_tokens(wrong, &record.tokens().unwrap())
            .unwrap(),
    );
    assert_eq!(
        w.admit_expansion(bad).unwrap_err().code,
        "LRR-E-EXPANSION-INPUT"
    );
    let SourceDiscoveryStep::NeedExpansion { request: again } = w.poll().unwrap() else {
        panic!("valid demand lost")
    };
    assert_eq!(again.key(), request.key());
    w.admit_expansion(record.clone()).unwrap();
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    assert_eq!(
        w.admit_expansion(record).unwrap_err().code,
        "LRR-E-EXPANSION-SEAL-VIOLATION"
    );
}
#[test]
fn source_discovery_generated_unknown_producer_retains_incomplete_work() {
    let v = source("macro_rules! make{()=>{unknown!();pub fn provisional(){}}}make!();");
    let mut w = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("demand absent")
    };
    w.admit_expansion(std::sync::Arc::new(request.expand().unwrap()))
        .unwrap();
    let SourceDiscoveryStep::Incomplete {
        diagnostic,
        blocked,
    } = w.poll().unwrap()
    else {
        panic!("unknown generated producer sealed")
    };
    assert_eq!(diagnostic.code, "LRR-E-CATALOG-INCOMPLETE");
    assert!(!blocked.is_empty());
    assert!(w.lookup("provisional").is_err());
}

#[test]
fn source_discovery_registers_generated_import_chain_before_publishing_seal() {
    let v = source(
        "macro_rules! make{()=>{use emitted as chosen;use chosen as selected;pub fn emitted(){}}}make!();",
    );
    let mut w = discover(&v);
    let SourceDiscoveryStep::NeedExpansion { request } = w.poll().unwrap() else {
        panic!("demand absent")
    };
    let record = std::sync::Arc::new(request.expand().unwrap());
    let g = lrr::expand::generated_items::GeneratedItems::admit(request.key(), record.clone(), 1)
        .unwrap();
    let identity = g
        .items()
        .iter()
        .find(|h| h.name.as_ref().is_some_and(|n| n.text == "emitted"))
        .unwrap()
        .key
        .fingerprint();
    w.admit_expansion(record).unwrap();
    assert!(matches!(w.poll().unwrap(), SourceDiscoveryStep::Sealed(_)));
    let lrr::frontend::names::NameLookup::Defined(d) = w.lookup("selected").unwrap() else {
        panic!("generated alias absent")
    };
    assert_eq!(d.identity, identity);
}
