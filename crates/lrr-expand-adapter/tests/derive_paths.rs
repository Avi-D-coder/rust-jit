use lrr_expand_adapter::{
    derive_paths,
    token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, TokenForest, TokenTree, Transparency,
    },
};
fn args(text: &str) -> TokenForest {
    let source = SourceIdentity {
        unit: Key([1; 32]),
        content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
        path: "derive.rs".into(),
    };
    let full = token_wire::parse_source(
        text,
        source,
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([1; 32]),
            edition: Edition::E2024,
        },
        Key([2; 32]),
    )
    .unwrap();
    if matches!(
        full.roots(),
        [TokenTree::Group {
            delimiter: token_wire::Delimiter::Parenthesis,
            ..
        }]
    ) {
        return full;
    }
    let [TokenTree::Group { children, .. }] = full.roots() else {
        unreachable!()
    };
    assert_eq!(children.len(), 1);
    TokenForest::new(
        children.clone(),
        full.contexts().to_vec(),
        full.spans().to_vec(),
    )
    .unwrap()
}
#[test]
fn derive_paths_preserve_order_absolute_raw_and_original_span_identity() {
    let text = "(First, ::renamed::r#gen, crate::Last,)";
    let input = args(text);
    let paths = derive_paths::parse(&input).unwrap();
    assert_eq!(
        paths.iter().map(|p| p.segments.clone()).collect::<Vec<_>>(),
        vec![
            vec!["First"],
            vec!["renamed", "r#gen"],
            vec!["crate", "Last"]
        ]
    );
    assert_eq!(
        paths.iter().map(|p| p.absolute).collect::<Vec<_>>(),
        [false, true, false]
    );
    for (path, expected) in paths
        .iter()
        .zip(["First", "::renamed::r#gen", "crate::Last"])
    {
        let first = input
            .spans()
            .iter()
            .find(|s| s.key() == path.first_span)
            .unwrap();
        let last = input
            .spans()
            .iter()
            .find(|s| s.key() == path.last_span)
            .unwrap();
        assert_eq!(&text[first.start as usize..last.end as usize], expected);
    }
    assert!(derive_paths::parse(&args("()")).unwrap().is_empty());
}
#[test]
fn derive_paths_reject_non_paths_generics_and_empty_interior_entries() {
    for text in [
        "(,)",
        "(First,,Last)",
        "(First<T>)",
        "(&Trait)",
        "(<T as Trait>::Assoc)",
        "(Fn())",
        "(First Last)",
    ] {
        assert!(derive_paths::parse(&args(text)).is_err(), "{text}");
    }
}
