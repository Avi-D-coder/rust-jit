use lrr_expand_adapter::{
    declaration_output::{self, DeclarationGrammar},
    generated_attribute::{import_binding_forest, import_path_forests},
    item_output::SyntaxEvent,
    token_wire::*,
};
fn input(text: &str) -> (TokenForest, Key) {
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([29; 32]),
        edition: Edition::E2015,
    };
    let key = context.key();
    (
        parse_source(
            text,
            SourceIdentity {
                unit: Key([29; 32]),
                content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
                path: "lib.rs".into(),
            },
            context,
            Key([30; 32]),
        )
        .unwrap(),
        key,
    )
}
fn leaves(input: &TokenForest) -> Vec<(u32, u32, String)> {
    let rendered = declaration_output::materialize(input, DeclarationGrammar::Items).unwrap();
    let mut offset = 0usize;
    let mut stack = Vec::new();
    let mut result = Vec::new();
    for event in rendered.syntax {
        match event {
            SyntaxEvent::Start(kind) => stack.push((kind, offset)),
            SyntaxEvent::Token { text, .. } => offset = offset.checked_add(text.len()).unwrap(),
            SyntaxEvent::Finish => {
                let (kind, start) = stack.pop().unwrap();
                if kind == syntax::SyntaxKind::USE_TREE as u16 {
                    result.push((
                        u32::try_from(start).unwrap(),
                        u32::try_from(offset).unwrap(),
                        rendered.text[start..offset].into(),
                    ));
                }
            }
        }
    }
    result
}
#[test]
fn generated_import_alias_preserves_original_raw_name_context_and_edition() {
    let (forest, context) = input("fn outer(){use host::{one as r#keep,two};}");
    let leaf = leaves(&forest)
        .into_iter()
        .find(|(_, _, text)| text.contains("r#keep") && !text.contains('{'))
        .unwrap();
    let selected = import_binding_forest(&forest, leaf.0, leaf.1).unwrap();
    let mut pending = selected.roots().iter().collect::<Vec<_>>();
    let mut identifiers = Vec::new();
    while let Some(token) = pending.pop() {
        match token {
            TokenTree::Group { children, .. } => pending.extend(children),
            TokenTree::Ident { text, raw, .. } => identifiers.push((text.as_str(), *raw)),
            _ => {}
        }
    }
    assert_eq!(identifiers, vec![("keep", true)]);
    assert!(selected.spans().iter().all(|span| span.context == context
        && span.edition == Edition::E2015
        && span.source.unit == Key([29; 32])));
    assert!(import_binding_forest(&forest, leaf.0 + 1, leaf.1).is_err());
}
#[test]
fn generated_import_unrenamed_and_glob_bindings_use_exact_pinned_leaf_spans() {
    let (forest, context) = input("fn outer(){use host::one;use host::*;}");
    for (start, end, _) in leaves(&forest) {
        let selected = import_binding_forest(&forest, start, end).unwrap();
        assert!(!selected.roots().is_empty());
        assert!(selected.spans().iter().all(|span| span.context == context));
    }
    assert!(import_binding_forest(&forest, 0, 0).is_err());
}

#[test]
fn generated_grouped_import_retains_target_prefixes_separately_from_alias() {
    let (forest, context) = input("fn outer(){use host::{one as keep};}");
    let (start, end, _) = leaves(&forest)
        .into_iter()
        .find(|(_, _, s)| !s.contains('{'))
        .unwrap();
    let paths = import_path_forests(&forest, start, end).unwrap();
    let mut names = Vec::new();
    for path in &paths {
        assert!(path.spans().iter().all(|s| s.context == context));
        let mut pending: Vec<_> = path.roots().iter().collect();
        while let Some(t) = pending.pop() {
            match t {
                TokenTree::Group { children, .. } => pending.extend(children),
                TokenTree::Ident { text, .. } => names.push(text.as_str()),
                _ => {}
            }
        }
    }
    names.sort();
    assert_eq!(names, ["host", "one"]);
    assert!(import_path_forests(&forest, start + 1, end).is_err());
}
#[test]
fn generated_import_target_retains_foreign_context_while_alias_stays_local() {
    let (original, local) = input("fn outer(){use host::{one as keep};}");
    let foreign = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([31; 32]),
        edition: Edition::E2015,
    };
    let mut roots = original.roots().to_vec();
    let mut spans = original.spans().to_vec();
    fn replace(tokens: &mut [TokenTree], spans: &mut Vec<SpanRecord>, foreign: Key) {
        for token in tokens {
            match token {
                TokenTree::Group { children, .. } => replace(children, spans, foreign),
                TokenTree::Ident { text, span, .. } if text == "host" => {
                    let old = spans.iter().find(|s| s.key() == *span).unwrap();
                    let changed = SpanRecord {
                        context: foreign,
                        ..old.clone()
                    };
                    *span = changed.key();
                    spans.push(changed);
                }
                _ => {}
            }
        }
    }
    replace(&mut roots, &mut spans, foreign.key());
    let mut contexts = original.contexts().to_vec();
    contexts.push(foreign.clone());
    let forest = TokenForest::new(roots, contexts, spans).unwrap();
    let (start, end, _) = leaves(&forest)
        .into_iter()
        .find(|(_, _, s)| !s.contains('{'))
        .unwrap();
    let alias = import_binding_forest(&forest, start, end).unwrap();
    assert!(alias.spans().iter().all(|s| s.context == local));
    let paths = import_path_forests(&forest, start, end).unwrap();
    assert!(
        paths
            .iter()
            .flat_map(|p| p.spans())
            .any(|s| s.context == foreign.key())
    );
    let mut hygiene = lrr_expand_adapter::hygiene::HygieneStore::default();
    hygiene.import(forest.contexts()).unwrap();
    let names = hygiene.name_contexts(&[local, foreign.key()]).unwrap();
    // Name comparison does not grant source/unit authority.
    assert_eq!(names[0].item, names[1].item);
    let resolution = hygiene
        .resolution_contexts(&[local, foreign.key()])
        .unwrap();
    assert_ne!(resolution[0].item, resolution[1].item);
}
