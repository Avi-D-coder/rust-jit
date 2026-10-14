//! Exact grammar/span bridge control; full rustc fixtures and execution remain separate.
use lrr_expand_adapter::{
    declaration_output::{self, DeclarationGrammar},
    generated_attribute::{
        AttributeRange, import_binding_forest_for_grammar, import_path_forests_for_grammar,
        materialize_for_grammar, path_forest_for_grammar,
    },
    item_output::SyntaxEvent,
    token_wire::*,
};
fn input(text: &str) -> TokenForest {
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([29; 32]),
        edition: Edition::E2015,
    };
    parse_source(
        text,
        SourceIdentity {
            unit: Key([29; 32]),
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: "grammar.rs".into(),
        },
        context,
        Key([30; 32]),
    )
    .unwrap()
}
fn ranges(
    forest: &TokenForest,
    grammar: DeclarationGrammar,
) -> (u32, u32, u32, u32, u32, u32, u32, u32) {
    let captured = declaration_output::materialize(forest, grammar).unwrap();
    let mut stack = Vec::new();
    let mut offset = 0u32;
    let mut item = None;
    let mut meta = None;
    let mut leaf = None;
    let mut wrapper = None;
    for event in captured.syntax {
        match event {
            SyntaxEvent::Start(kind) => stack.push((kind, offset)),
            SyntaxEvent::Token { text, .. } => {
                offset = offset
                    .checked_add(u32::try_from(text.len()).unwrap())
                    .unwrap()
            }
            SyntaxEvent::Finish => {
                let (kind, start) = stack.pop().unwrap();
                if kind == syntax::SyntaxKind::FN as u16 {
                    item = Some((start, offset));
                }
                if kind == syntax::SyntaxKind::PATH_META as u16
                    || kind == syntax::SyntaxKind::TOKEN_TREE_META as u16
                {
                    meta = Some((start, offset));
                }
                if kind == syntax::SyntaxKind::ATTR as u16 {
                    wrapper = Some((start, offset));
                }
                if kind == syntax::SyntaxKind::USE_TREE as u16 {
                    leaf = Some((start, offset));
                }
            }
        }
    }
    let (a, b) = item.unwrap();
    let (c, d) = meta.unwrap();
    let (e, f) = leaf.unwrap();
    (a, b, c, d, e, f, wrapper.unwrap().0, wrapper.unwrap().1)
}
#[test]
fn generated_expression_attribute_and_import_bridge_preserves_exact_grammar_spans() {
    for (text, grammar) in [
        (
            "{use host::one as keep;#[keep]fn inner(){}0}",
            DeclarationGrammar::Expression,
        ),
        (
            "use host::one as keep;#[keep]fn inner(){}",
            DeclarationGrammar::Statements,
        ),
        (
            "use host::one as keep;#[keep]fn inner(){}",
            DeclarationGrammar::Items,
        ),
    ] {
        let forest = input(text);
        let (a, b, c, d, e, f, ws, we) = ranges(&forest, grammar);
        let path = path_forest_for_grammar(&forest, a, b, c, d, grammar).unwrap();
        assert!(!path.spans().is_empty());
        let alias = import_binding_forest_for_grammar(&forest, e, f, grammar).unwrap();
        assert!(!alias.spans().is_empty());
        let paths = import_path_forests_for_grammar(&forest, e, f, grammar).unwrap();
        assert_eq!(paths.len(), 1);
        let prepared = materialize_for_grammar(
            &forest,
            a,
            b,
            0,
            &[AttributeRange {
                start: c,
                end: d,
                wrapper_start: ws,
                wrapper_end: we,
            }],
            grammar,
        )
        .unwrap();
        assert!(prepared.arguments.is_none());
        assert!(!prepared.item.spans().is_empty());
        assert!(path_forest_for_grammar(&forest, a, b, c + 1, d, grammar).is_err());
        assert!(import_binding_forest_for_grammar(&forest, e + 1, f, grammar).is_err());
        for span in path
            .spans()
            .iter()
            .chain(alias.spans())
            .chain(paths[0].spans())
        {
            assert_eq!(span.source.unit, Key([29; 32]));
            assert_eq!(span.edition, Edition::E2015);
        }
    }
}

#[test]
fn generated_argument_bridge_preserves_original_children_and_delimiter_spans() {
    use lrr_expand_adapter::generated_attribute::argument_forest_for_grammar;
    for (text, grammar) in [
        (
            "{use host::one as keep;#[keep(1 + 2)]fn inner(){}0}",
            DeclarationGrammar::Expression,
        ),
        (
            "use host::one as keep;#[keep(1 + 2)]fn inner(){}",
            DeclarationGrammar::Statements,
        ),
        (
            "use host::one as keep;#[keep(1 + 2)]fn inner(){}",
            DeclarationGrammar::Items,
        ),
    ] {
        let forest = input(text);
        let (a, b, c, d, _, _, ws, we) = ranges(&forest, grammar);
        let arguments = argument_forest_for_grammar(&forest, a, b, c, d, grammar).unwrap();
        let prepared = materialize_for_grammar(
            &forest,
            a,
            b,
            0,
            &[AttributeRange {
                start: c,
                end: d,
                wrapper_start: ws,
                wrapper_end: we,
            }],
            grammar,
        )
        .unwrap();
        let prepared = prepared.arguments.unwrap();
        let [
            TokenTree::Group {
                delimiter: Delimiter::Parenthesis,
                open,
                close,
                children,
            },
        ] = arguments.roots()
        else {
            panic!("original argument delimiter lost")
        };
        let [
            TokenTree::Group {
                delimiter: Delimiter::Invisible,
                open: input_open,
                close: input_close,
                children: input_children,
            },
        ] = prepared.roots()
        else {
            panic!("attribute input envelope lost")
        };
        assert_eq!(
            (open, close, children),
            (input_open, input_close, input_children)
        );
        for span in arguments.spans().iter().chain(prepared.spans()) {
            assert_eq!(span.source.unit, Key([29; 32]));
            assert_eq!(span.edition, Edition::E2015);
            assert!(
                forest
                    .spans()
                    .iter()
                    .any(|original| original.key() == span.key())
            );
        }
        assert!(argument_forest_for_grammar(&forest, a, b, c + 1, d, grammar).is_err());
    }
}
#[test]
fn generated_module_binding_bridge_preserves_exact_raw_name_span_and_grammar() {
    use lrr_expand_adapter::generated_attribute::module_binding_forest_for_grammar;
    for (text, grammar) in [
        ("pub mod r#type {}", DeclarationGrammar::Items),
        ("pub mod r#type {}", DeclarationGrammar::Statements),
        ("{pub mod r#type {} 0}", DeclarationGrammar::Expression),
    ] {
        let forest = input(text);
        let capture = declaration_output::materialize(&forest, grammar).unwrap();
        let mut offset = 0u32;
        let mut stack = Vec::new();
        let mut selected = None;
        for event in capture.syntax {
            match event {
                SyntaxEvent::Start(kind) => stack.push((kind, offset)),
                SyntaxEvent::Token { text, .. } => {
                    offset = offset
                        .checked_add(u32::try_from(text.len()).unwrap())
                        .unwrap()
                }
                SyntaxEvent::Finish => {
                    let (kind, start) = stack.pop().unwrap();
                    if kind == syntax::SyntaxKind::MODULE as u16 {
                        selected = Some((start, offset));
                    }
                }
            }
        }
        let (start, end) = selected.unwrap();
        let name = module_binding_forest_for_grammar(&forest, start, end, grammar).unwrap();
        let roots = match name.roots() {
            [
                TokenTree::Group {
                    delimiter: Delimiter::Invisible,
                    children,
                    ..
                },
            ] => children.as_slice(),
            roots => roots,
        };
        let [TokenTree::Ident { text, raw, span }] = roots else {
            panic!("original raw module name required")
        };
        assert_eq!(text, "type");
        assert!(*raw);
        let original = forest
            .spans()
            .iter()
            .find(|row| row.key() == *span)
            .unwrap();
        let retained = name.spans().iter().find(|row| row.key() == *span).unwrap();
        assert_eq!(original, retained);
        assert_eq!(retained.source.unit, Key([29; 32]));
        assert_eq!(retained.edition, Edition::E2015);
        assert!(
            module_binding_forest_for_grammar(&forest, start.checked_add(1).unwrap(), end, grammar)
                .is_err()
        );
    }
    let opaque = input("emit!{mod hidden{}}");
    let captured = declaration_output::materialize(&opaque, DeclarationGrammar::Items).unwrap();
    assert!(
        module_binding_forest_for_grammar(
            &opaque,
            0,
            u32::try_from(captured.text.len()).unwrap(),
            DeclarationGrammar::Items
        )
        .is_err()
    );
}
