use lrr_expand_adapter::{declarative::DeclarativeMacro, token_wire::*};
#[path = "support/declarative.rs"]
mod fixture;
#[test]
fn source_identifiers_are_canonical_but_spans_and_literals_preserve_source() {
    let text = "r#cafe\u{301} \"cafe\u{301}\"";
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([1; 32]),
        edition: Edition::E2024,
    };
    let source = SourceIdentity {
        unit: Key([1; 32]),
        content: Key([2; 32]),
        path: "unicode.rs".into(),
    };
    let forest = parse_source(text, source.clone(), context.clone(), Key([3; 32])).unwrap();
    let [TokenTree::Group { children, .. }] = forest.roots() else {
        panic!("missing root")
    };
    let TokenTree::Ident {
        text: name,
        raw,
        span,
    } = &children[0]
    else {
        panic!("missing identifier")
    };
    assert_eq!(name, "café");
    assert!(*raw);
    let record = forest.spans().iter().find(|r| r.key() == *span).unwrap();
    assert_eq!(
        &text[record.start as usize..record.end as usize],
        "r#cafe\u{301}"
    );
    assert_eq!(record.source, source);
    assert_eq!(record.context, context.key());
    assert_eq!(record.origin, Key([3; 32]));
    assert_eq!(
        fixture::literal_texts(&forest),
        vec!["\"cafe\u{301}\"".to_owned()]
    );
    assert_eq!(forest, transport_round_trip(&forest).unwrap());
}
#[test]
fn canonical_identifier_input_matches_literal_rules_in_all_definition_editions() {
    for edition in [
        Edition::E2015,
        Edition::E2018,
        Edition::E2021,
        Edition::E2024,
    ] {
        let mac = DeclarativeMacro::new(fixture::definition(
            "(café) => {17}; ($other:ident) => {29};",
            edition,
        ))
        .unwrap();
        let input = fixture::input("cafe\u{301}");
        let result = mac
            .expand(&input, fixture::call_site(&input))
            .unwrap()
            .unwrap();
        assert_eq!(
            fixture::literal_texts(&result.tokens),
            vec!["17".to_owned()]
        );
    }
}
#[test]
fn manually_transported_identifier_is_canonicalized_without_replacing_span() {
    let original = fixture::input("café");
    let mut roots = original.roots().to_vec();
    let TokenTree::Group { children, .. } = &mut roots[0] else {
        panic!("missing root")
    };
    let TokenTree::Ident { text, .. } = &mut children[0] else {
        panic!("missing identifier")
    };
    *text = "cafe\u{301}".into();
    let input = TokenForest::new(
        roots,
        original.contexts().to_vec(),
        original.spans().to_vec(),
    )
    .unwrap();
    assert_eq!(transport_round_trip(&input).unwrap(), original);
}

#[test]
fn canonical_identifier_wire_admission_matches_marked_transcription() {
    use lrr_expand_adapter::declarative::{DeclarativeSite, FailureKind};
    let mac = DeclarativeMacro::new(fixture::definition(
        "($name:ident) => {($name,cafe\u{301})};",
        Edition::E2024,
    ))
    .unwrap();
    let site = DeclarativeSite {
        expansion: Key([8; 32]),
        defining_unit: Key([1; 32]),
        definition_edition: Edition::E2024,
    };
    // First spelling shrinks under NFC; the second expands. Both retain the
    // original source span, and counting uses their canonical semantic bytes.
    for spelling in ["cafe\u{301}", "x\u{344}"] {
        let input = fixture::input(spelling);
        let call = fixture::call_site(&input);
        let (result, stats) = mac
            .expand_hygienic_with_limits(&input, call, site, 100, 4096)
            .unwrap();
        let output = result.unwrap().tokens;
        let size = output.encode().unwrap().len();
        assert_eq!(stats.admitted_bytes, size);
        let (exact, stats) = mac
            .expand_hygienic_with_limits(&input, call, site, 100, size)
            .unwrap();
        assert_eq!(exact.unwrap().tokens, output);
        assert_eq!(stats.admitted_bytes, size);
        let (refused, stats) = mac
            .expand_hygienic_with_limits(&input, call, site, 100, size - 1)
            .unwrap();
        assert_eq!(refused.err().unwrap().kind, FailureKind::Limit);
        assert_eq!(stats.materializations, 0);
    }
}
