use lrr::{
    core::{digest::StableEncode, profile::ProfileIdentity},
    facade::macros::RuntimeMacroKind,
    source::snapshot::SourceBlob,
    syntax::{parse::ParsedFile, spans::SourceRange},
};
fn capture(
    source: &str,
    invocation: &str,
    kind: RuntimeMacroKind,
) -> lrr::syntax::runtime_macro_arguments::RuntimeMacroArguments {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let start = source.find(invocation).unwrap() as u32;
    let call = parsed
        .macro_call_at(SourceRange {
            start,
            end: start + invocation.len() as u32,
        })
        .unwrap();
    parsed.runtime_macro_arguments(&call, kind).unwrap()
}
fn slice(source: &str, r: SourceRange) -> &str {
    &source[r.start as usize..r.end as usize]
}
#[test]
fn runtime_macro_argument_capture_preserves_written_operands_and_lazy_format_tail() {
    let source = r#"fn case(){assert_eq!(first::<A,B>(), { second(); 2i64 }, "failure {}", missing(), value = unknown(),);}"#;
    let invocation = &source[source.find("assert_eq!").unwrap()..source.len() - 2];
    let args = capture(source, invocation, RuntimeMacroKind::AssertEq);
    assert_eq!(
        args.operands()
            .iter()
            .map(|r| slice(source, *r))
            .collect::<Vec<_>>(),
        ["first::<A,B>()", "{ second(); 2i64 }"]
    );
    assert_eq!(
        slice(source, args.message().unwrap()),
        "\"failure {}\", missing(), value = unknown(),"
    );
    assert_eq!(args.kind(), RuntimeMacroKind::AssertEq);
}
#[test]
fn runtime_macro_argument_capture_handles_delimiters_closure_commas_and_empty_panic() {
    for (source, call, kind, written) in [
        (
            "fn case(){assert![(|x,y| x+y)(1i64,2i64)==3i64,];}",
            "assert![(|x,y| x+y)(1i64,2i64)==3i64,]",
            RuntimeMacroKind::Assert,
            "(|x,y| x+y)(1i64,2i64)==3i64",
        ),
        (
            "fn case(){assert_ne!{1i64,2i64};}",
            "assert_ne!{1i64,2i64}",
            RuntimeMacroKind::AssertNe,
            "1i64",
        ),
    ] {
        let args = capture(source, call, kind);
        assert_eq!(slice(source, args.operands()[0]), written);
        assert!(args.message().is_none());
    }
    let args = capture("fn case(){panic!();}", "panic!()", RuntimeMacroKind::Panic);
    assert!(args.operands().is_empty());
    assert!(args.message().is_none());
}
#[test]
fn runtime_macro_argument_capture_is_kind_selected_and_source_stale_safe() {
    let source = "// 🦀\nfn case(){renamed!(false, \"message\");}";
    let args = capture(
        source,
        "renamed!(false, \"message\")",
        RuntimeMacroKind::Assert,
    );
    assert_eq!(slice(source, args.operands()[0]), "false");
    assert_eq!(slice(source, args.message().unwrap()), "\"message\"");
    let same = capture(
        source,
        "renamed!(false, \"message\")",
        RuntimeMacroKind::Assert,
    );
    assert_eq!(args.fingerprint(), same.fingerprint());
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.replace("false", "true").into_bytes()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(args.require_source(&parsed.key()).is_err());
}
#[test]
fn runtime_macro_argument_capture_rejects_missing_malformed_and_oversized_operands() {
    for (call, kind) in [
        ("assert!()", RuntimeMacroKind::Assert),
        ("assert_eq!(1i64)", RuntimeMacroKind::AssertEq),
        ("assert_ne!(,)", RuntimeMacroKind::AssertNe),
        ("assert!(true,,)", RuntimeMacroKind::Assert),
    ] {
        let source = format!("fn case(){{{call};}}");
        let parsed = ParsedFile::parse(
            &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let range = SourceRange {
            start: 10,
            end: 10 + call.len() as u32,
        };
        let selected = parsed.macro_call_at(range).unwrap();
        assert!(parsed.runtime_macro_arguments(&selected, kind).is_err());
    }
    let call = format!("panic!(\"{}\")", "x".repeat(131_073));
    let source = format!("fn case(){{{call};}}");
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let selected = parsed
        .macro_call_at(SourceRange {
            start: 10,
            end: 10 + call.len() as u32,
        })
        .unwrap();
    assert!(
        parsed
            .runtime_macro_arguments(&selected, RuntimeMacroKind::Panic)
            .is_err()
    );
}
