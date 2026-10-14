//! M010 source-facet helper declarations from the pinned parser, not rustc metadata.
use lrr::{
    core::profile::ProfileIdentity,
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{parse::ParsedFile, spans::SourceRange},
};
fn parse(
    source: &str,
    cfg: &CfgSet,
) -> Result<
    lrr::expand::derive_declaration::SourceDeriveDeclaration,
    lrr::core::diagnostic::Diagnostic,
> {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    parsed.source_derive_declaration_at(
        SourceRange {
            start: 0,
            end: source.len().try_into().unwrap(),
        },
        cfg,
    )
}
#[test]
fn source_derive_declaration_reads_cfg_attr_raw_helpers_and_leaves_body_cold() {
    let source = "#[cfg_attr(on, proc_macro_derive(r#gen, attributes(first, r#type, first,)))] pub fn implementation(input: TokenStream)->TokenStream { missing_cold_body(input) }";
    let mut cfg = CfgSet::default();
    cfg.atoms.insert("on".into());
    let declaration = parse(source, &cfg).unwrap();
    assert_eq!(declaration.names().export(), "gen");
    assert_eq!(
        declaration
            .names()
            .helpers()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["first", "type"]
    );
    assert!(parse(source, &CfgSet::default()).is_err());
    assert_eq!(
        parse(
            "#[proc_macro_derive(Empty,)] pub fn implementation()->TokenStream{missing()}",
            &cfg
        )
        .unwrap()
        .names()
        .helpers()
        .len(),
        0
    );
    assert_eq!(parse("#[proc_macro_derive(Empty, attributes(),)] pub fn implementation()->TokenStream{missing()}",&cfg).unwrap().names().helpers().len(),0);
}
#[test]
fn source_derive_declarations_refuse_unexpanded_or_malformed_headers() {
    for source in [
        "#[proc_macro_derive(Name, attributes($helper))] pub fn f() {}",
        "#[proc_macro_derive(mod::Name)] pub fn f() {}",
        "#[proc_macro_derive(Name, attributes(one::two))] pub fn f() {}",
        "#[proc_macro_derive(Name, attributes(one two))] pub fn f() {}",
        "#[proc_macro_derive(Name, attributes(\"literal\"))] pub fn f() {}",
        "#[proc_macro_derive(Name, attributes(,))] pub fn f() {}",
        "#[unknown_transform] #[proc_macro_derive(Name)] pub fn f() {}",
        "#[proc_macro_derive(Name)] #[proc_macro_derive(Other)] pub fn f() {}",
        "#[cfg(any())] #[proc_macro_derive(Name)] pub fn f() {}",
        "#[proc_macro_derive(Name)] struct Wrong;",
    ] {
        assert!(parse(source, &CfgSet::default()).is_err(), "{source}");
    }
}

#[test]
fn source_catalog_collects_enabled_declarations_and_suspends_root_producers() {
    let source = "#[cfg(any())] generate_hidden!(); macro_rules! declare {()=>{}} #[proc_macro_derive(One,attributes(helper))] pub fn first()->TokenStream{unresolved_body!()} #[cfg_attr(all(),proc_macro_derive(Two))] pub fn second()->TokenStream{missing()} #[cfg(any())] #[proc_macro_derive(One)] pub fn disabled(){}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let catalog = parsed.source_derive_catalog(&CfgSet::default()).unwrap();
    assert!(catalog.pending().is_empty());
    assert_eq!(
        catalog
            .declarations()
            .iter()
            .map(|(_, d)| d.names().export())
            .collect::<Vec<_>>(),
        ["One", "Two"]
    );
    let pending = "declare!(); #[unknown_transform] #[proc_macro_derive(One)] pub fn first(){} #[derive(UserGenerate)] struct Other; #[proc_macro_derive(Two)] pub fn second(){}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(pending.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let catalog = parsed.source_derive_catalog(&CfgSet::default()).unwrap();
    assert_eq!(catalog.pending().len(), 3);
    assert_eq!(catalog.declarations().len(), 1);
    assert_eq!(catalog.declarations()[0].1.names().export(), "Two");
    let inner = "#![unknown_transform] #[proc_macro_derive(One)] pub fn first(){}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(inner.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let catalog = parsed.source_derive_catalog(&CfgSet::default()).unwrap();
    assert_eq!(catalog.pending().len(), 1);
    assert!(catalog.declarations().is_empty());
    let duplicate =
        "#[proc_macro_derive(Same)] pub fn first(){} #[proc_macro_derive(Same)] pub fn second(){}";
    let parsed = ParsedFile::parse(
        &SourceBlob::new(duplicate.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.source_derive_catalog(&CfgSet::default()).is_err());
}
