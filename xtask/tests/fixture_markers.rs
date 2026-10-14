use lrr::{
    core::{digest::StableEncode, profile::ProfileIdentity},
    source::snapshot::SourceBlob,
    syntax::{fixture_markers::MarkerTarget, parse::ParsedFile},
};
fn parsed(text: &str) -> ParsedFile {
    ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
}
#[test]
fn fixture_markers_bind_full_following_expressions_with_byte_ranges_and_trivia() {
    let source = "// 🦀 UTF-8 before ranges\nfn case(){/*lrr-marker:sum*/ 2i64+3i64*4i64; if true {} else { /*lrr-marker:cold*/ /* ordinary */ absent::<Missing>(1i64).other(); } let x=false && /*lrr-marker:right*/ absent_bool();}";
    let parse = parsed(source);
    let markers = parse.fixture_markers().unwrap();
    for (name, written) in [
        ("sum", "2i64+3i64*4i64"),
        ("cold", "absent::<Missing>(1i64).other()"),
        ("right", "absent_bool()"),
    ] {
        let MarkerTarget::Expression(r) = markers.get(name).unwrap().target else {
            panic!("unexpected expansion demand")
        };
        assert_eq!(&source[r.start as usize..r.end as usize], written);
        assert!(markers.get(name).unwrap().comment.end <= r.start);
    }
    assert_eq!(
        markers
            .markers()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        ["cold", "right", "sum"]
    );
    assert_eq!(
        markers.fingerprint(),
        parse.fixture_markers().unwrap().fingerprint()
    );
}
#[test]
fn fixture_markers_keep_macro_arguments_pending_and_ignore_string_lookalikes() {
    let source = r#"fn case(){let s="/*lrr-marker:not_real*/ 1"; assert!(/*lrr-marker:inner*/ false && missing()); /*lrr-marker:outer*/ panic!("x");}"#;
    let markers = parsed(source).fixture_markers().unwrap();
    assert!(markers.get("not_real").is_none());
    let MarkerTarget::NeedsMacroExpansion { invocation } = markers.get("inner").unwrap().target
    else {
        panic!("token-tree marker must remain pending")
    };
    assert_eq!(
        &source[invocation.start as usize..invocation.end as usize],
        "assert!(/*lrr-marker:inner*/ false && missing())"
    );
    let MarkerTarget::Expression(r) = markers.get("outer").unwrap().target else {
        panic!("outer invocation is an expression")
    };
    assert_eq!(&source[r.start as usize..r.end as usize], "panic!(\"x\")");
}
#[test]
fn fixture_markers_reject_duplicate_orphan_malformed_and_stale_sources() {
    for source in [
        "fn case(){/*lrr-marker:same*/1;/*lrr-marker:same*/2;}",
        "fn case(){1;/*lrr-marker:end*/}",
        "fn case(){/*lrr-marker:bad name*/1;}",
        "fn case(){/*lrr-marker:bad*/ let x=1;}",
    ] {
        assert_eq!(
            parsed(source).fixture_markers().err().unwrap().code,
            "LRR-E-FIXTURE-MARKER"
        );
    }
    assert!(
        parsed("fn case(){/*lrr-marker:invalid*/1+;}")
            .fixture_markers()
            .is_err()
    );
    let cold_error = parsed("fn broken(){let =;} fn case(){/*lrr-marker:valid*/1;}");
    assert!(!cold_error.errors.is_empty());
    assert!(matches!(
        cold_error
            .fixture_markers()
            .unwrap()
            .get("valid")
            .unwrap()
            .target,
        MarkerTarget::Expression(_)
    ));
    let parse = parsed("fn case(){/*lrr-marker:value*/1;}");
    let markers = parse.fixture_markers().unwrap();
    markers.require_source(&parse.key()).unwrap();
    let edited = parsed("fn case(){/*lrr-marker:value*/2;}");
    assert!(markers.require_source(&edited.key()).is_err());
    assert_ne!(
        markers.fingerprint(),
        edited.fixture_markers().unwrap().fingerprint()
    );
}
#[test]
fn fixture_markers_load_actual_on_disk_contract_source_without_claiming_execution() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../compat/fixtures/T034-FORMAT/main.rs"),
    )
    .unwrap();
    let markers = parsed(&source).fixture_markers().unwrap();
    let MarkerTarget::Expression(r) = markers.get("cold_branch").unwrap().target else {
        panic!("source expression must bind")
    };
    assert_eq!(&source[r.start as usize..r.end as usize], "unknown()");
}
