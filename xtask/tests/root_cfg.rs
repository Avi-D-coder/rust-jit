//! Official fixture metadata adaptation in Rust2024; no stock run-pass claim.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    source::snapshot::{CfgSet, SourceBlob, SourcePath},
    syntax::{outline::ScopeKey, parse::ParsedFile},
};

fn outline(source: &str, cfg: &CfgSet) -> lrr::syntax::outline::Outline {
    ParsedFile::parse_declarations(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .outline(Digest([41; 32]), vec![], cfg)
    .unwrap()
}
#[test]
fn rustc_crate_cfg_false_excludes_test_headers_before_catalog_discovery() {
    let text =
        include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/test-attrs/issue-34932.rs");
    assert!(text.contains("#![cfg(false)]"));
    assert!(text.contains("fn dummy()"));
    let cfg = CfgSet::guest_default();
    let result = outline(text, &cfg);
    assert!(result.items.is_empty());
    assert!(result.imports.is_empty());
    assert!(result.item_invocations.is_empty());
    let parsed = ParsedFile::parse_declarations(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let scan = parsed
        .scan_declarations(
            SourcePath::new("upstream.rs").unwrap(),
            ScopeKey {
                crate_key: Digest([41; 32]),
                module_path: vec![],
                owner: None,
            },
            &cfg,
            Default::default(),
        )
        .unwrap();
    assert!(scan.producers().is_empty());
    assert!(scan.modules().is_empty());
    assert!(scan.impls().is_empty());
}
#[test]
fn root_cfg_attr_excludes_imports_and_external_module_demands() {
    let disabled = "#![cfg_attr(true, cfg(false))]\nmod unavailable; use unavailable::*; missing!(); fn cold(){missing();}";
    let cfg = CfgSet::guest_default();
    let result = outline(disabled, &cfg);
    assert!(result.items.is_empty());
    assert!(result.imports.is_empty());
    assert!(result.textual_events.is_empty());
    assert!(
        lrr::syntax::modules::external_modules(disabled, &cfg)
            .unwrap()
            .is_empty()
    );
    let active = disabled.replace("cfg_attr(true", "cfg_attr(false");
    let active_outline = outline(&active, &cfg);
    assert_eq!(
        active_outline
            .items
            .iter()
            .map(|i| i.def_key.item_kind)
            .collect::<Vec<_>>(),
        [
            lrr::source::manifest::ItemKind::Module,
            lrr::source::manifest::ItemKind::Import,
            lrr::source::manifest::ItemKind::Function
        ]
    );
    assert_eq!(active_outline.imports.len(), 1);
    assert_eq!(active_outline.item_invocations.len(), 1);
    assert_eq!(
        lrr::syntax::modules::external_modules(&active, &cfg)
            .unwrap()
            .len(),
        1
    );
    assert_ne!(result.fingerprint(), active_outline.fingerprint());
}
