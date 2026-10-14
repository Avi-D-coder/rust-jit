use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    expand::scopes::{MacroDefinition, MacroName},
    frontend::{
        declaration_imports::DeclarationImportIndex,
        macro_names::{MacroNamespaces, MacroPath, PathLookup},
    },
    source::snapshot::{CfgSet, SourceBlob, SourcePath},
    syntax::{
        declaration_scan::DeclarationScan, imports::ImportSegment, outline::ScopeKey,
        parse::ParsedFile,
    },
};
fn scan(source: &str) -> DeclarationScan {
    ParsedFile::parse_declarations(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .scan_declarations(
        SourcePath::new("main.rs").unwrap(),
        ScopeKey {
            crate_key: Digest([1; 32]),
            module_path: vec![],
            owner: None,
        },
        &CfgSet::guest_default(),
        Default::default(),
    )
    .unwrap()
}
fn graph() -> MacroNamespaces {
    let mut g = MacroNamespaces::default();
    g.dependency(
        Digest([1; 32]),
        MacroName::parse("host", 2024).unwrap(),
        Digest([2; 32]),
    )
    .unwrap();
    for (name, byte) in [("one", 9), ("two", 10)] {
        g.define(
            MacroPath::new(Digest([2; 32]), vec![MacroName::parse(name, 2024).unwrap()]).unwrap(),
            MacroDefinition {
                identity: Digest([byte; 32]),
                defining_unit: Digest([2; 32]),
                definition_edition: 2015,
            },
        )
        .unwrap();
    }
    g
}
fn lookup(s: &DeclarationScan, index: usize, g: &MacroNamespaces) -> PathLookup {
    DeclarationImportIndex::new(s)
        .unwrap()
        .lookup(
            s.producers()[index].identity,
            &[ImportSegment::Name("keep".into())],
            false,
            g,
        )
        .unwrap()
}
#[test]
fn declaration_imports_bind_actual_prepared_definitions() {
    let s = scan("fn f(){use host::one as keep;#[keep]fn inner(){}}");
    assert!(matches!(lookup(&s,0,&graph()),PathLookup::Defined(d) if d.identity==Digest([9;32])));
}
#[test]
fn declaration_imports_shadow_inside_blocks_and_do_not_leak_to_siblings() {
    let s = scan(
        "fn f(){use host::one as keep;{use host::two as keep;#[keep]fn inner(){}}#[keep]fn outer(){}}fn other(){#[keep]fn sibling(){}}",
    );
    let g = graph();
    assert!(matches!(lookup(&s,0,&g),PathLookup::Defined(d) if d.identity==Digest([10;32])));
    assert!(matches!(lookup(&s,1,&g),PathLookup::Defined(d) if d.identity==Digest([9;32])));
    assert_eq!(lookup(&s, 2, &g), PathLookup::Absent);
}
#[test]
fn declaration_imports_follow_named_module_and_macro_alias_chains() {
    let s = scan(
        "fn f(){use host as module;use module::one as first;use first as keep;#[keep]fn inner(){}}",
    );
    assert!(matches!(lookup(&s,0,&graph()),PathLookup::Defined(d) if d.identity==Digest([9;32])));
}
#[test]
fn declaration_import_cycles_and_missing_targets_remain_typed_demands() {
    for text in [
        "fn f(){use second as keep;use keep as second;#[keep]fn inner(){}}",
        "fn f(){use unknown::one as keep;#[keep]fn inner(){}}",
    ] {
        let s = scan(text);
        assert!(matches!(lookup(&s, 0, &graph()), PathLookup::NeedInput(_)));
    }
}
#[test]
fn declaration_import_annotations_block_lookup_before_alias_adoption() {
    let s = scan("fn f(){#[change]use host::one as keep;#[keep]fn inner(){}}");
    assert_eq!(
        lookup(&s, 1, &graph()),
        PathLookup::NeedInput(s.producers()[0].identity)
    );
}
#[test]
fn declaration_import_duplicate_named_bindings_refuse_even_identical_targets() {
    let s = scan("fn f(){use host::one as keep;use host::one as keep;#[keep]fn inner(){}}");
    let e = DeclarationImportIndex::new(&s)
        .unwrap()
        .lookup(
            s.producers()[0].identity,
            &[ImportSegment::Name("keep".into())],
            false,
            &graph(),
        )
        .unwrap_err();
    assert_eq!(e.code, "LRR-E-MACRO-AMBIGUOUS");
}
#[test]
fn declaration_import_absolute_path_bypasses_local_aliases() {
    let s = scan("fn f(){use other as host;#[keep]fn inner(){}}");
    let result = DeclarationImportIndex::new(&s)
        .unwrap()
        .lookup(
            s.producers()[0].identity,
            &[
                ImportSegment::Name("host".into()),
                ImportSegment::Name("one".into()),
            ],
            true,
            &graph(),
        )
        .unwrap();
    assert!(matches!(result,PathLookup::Defined(d) if d.identity==Digest([9;32])));
}
