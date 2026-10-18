//! Controls for the incremental attachment surface: the captures of one edit,
//! the owner of a call, the rooted forest and the buffered identity hash. They
//! compare each fast path with the complete export it replaces.
use lrr_expand_adapter::{
    declaration_output::{CallCapture, DeclarationGrammar},
    declarative::DeclarativeMacro,
    syntax_attachment::{BodyOwners, FragmentSlot, ListSlot, SyntaxAttachment},
    token_wire::*,
};

fn input(text: &str, edition: Edition) -> TokenForest {
    parse_source(
        text,
        SourceIdentity {
            unit: Key([71; 32]),
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: "incremental.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([71; 32]),
            edition,
        },
        Key([72; 32]),
    )
    .unwrap()
}
fn same_call(a: &CallCapture, b: &CallCapture) -> bool {
    (a.start, a.end, a.name_span, a.site, a.edition) == (b.start, b.end, b.name_span, b.site, b.edition)
        && a.path == b.path
        && a.input == b.input
}
fn call_named(tree: &SyntaxAttachment, name: &str) -> CallCapture {
    tree.materialize()
        .unwrap()
        .calls
        .into_iter()
        .find(|call| {
            lrr_expand_adapter::declaration_output::materialize(
                &call.path,
                DeclarationGrammar::Expression,
            )
            .unwrap()
            .text
            .trim()
                == name
        })
        .unwrap()
}
fn expansion(tree: &SyntaxAttachment, name: &str, call: &CallCapture) -> TokenForest {
    let captured = tree.materialize().unwrap();
    let definition = captured
        .definitions
        .iter()
        .find(|definition| {
            captured.text[definition.start as usize..definition.end as usize]
                .starts_with(&format!("macro_rules!{name}"))
        })
        .unwrap();
    DeclarativeMacro::new(definition.body.clone())
        .unwrap()
        .expand(&call.input, call.site)
        .unwrap()
        .unwrap()
        .tokens
}

const SOURCE: &str = "macro_rules! outer { () => { inner!(); inner!(); let x = 1; } }\n\
macro_rules! inner { () => { macro_rules! made { () => { 7 } } } }\n\
fn first() { outer!(); }\n\
fn second() { fn nested() { inner!(); } nested(); }\n";

#[test]
fn an_edits_captures_equal_the_complete_export_restricted_to_its_range() {
    for edition in [Edition::E2015, Edition::E2021, Edition::E2024] {
        let mut tree =
            SyntaxAttachment::parse(&input(SOURCE, edition), DeclarationGrammar::Items).unwrap();
        for (name, slot) in [("outer", ListSlot::Statements), ("inner", ListSlot::Statements)] {
            let call = call_named(&tree, name);
            let output = expansion(&tree, name, &call);
            let edit = tree
                .attach_list_with_edit(call.start, call.end, &output, slot)
                .unwrap();
            let (start, end) = edit.inserted();
            let ranged = tree.captures_in(start, end).unwrap();
            let complete = tree.materialize().unwrap();
            let expected_calls = complete
                .calls
                .iter()
                .filter(|call| start <= call.start && call.end <= end)
                .collect::<Vec<_>>();
            assert_eq!(ranged.calls.len(), expected_calls.len());
            assert!(
                ranged
                    .calls
                    .iter()
                    .zip(&expected_calls)
                    .all(|(a, b)| same_call(a, b))
            );
            let expected_definitions = complete
                .definitions
                .iter()
                .filter(|definition| start <= definition.start && definition.end <= end)
                .collect::<Vec<_>>();
            assert_eq!(ranged.definitions.len(), expected_definitions.len());
            for (a, b) in ranged.definitions.iter().zip(&expected_definitions) {
                assert_eq!((a.start, a.end, a.name_span), (b.start, b.end, b.name_span));
                assert_eq!(a.body, b.body);
            }
        }
        // The whole text as the range is the complete export.
        let complete = tree.materialize().unwrap();
        let all = tree.captures_in(0, complete.text.len() as u32).unwrap();
        assert_eq!(all.calls.len(), complete.calls.len());
        assert_eq!(all.definitions.len(), complete.definitions.len());
        assert!(tree.captures_in(3, 3).unwrap().calls.is_empty());
        assert!(tree.captures_in(0, complete.text.len() as u32 + 1).is_err());
    }
}

#[test]
fn a_call_is_owned_by_its_innermost_function_body() {
    let tree = SyntaxAttachment::parse(
        &input(
            "fn outer() { a!(); fn inner() { b!(); } c!(); }\nconst K: u32 = d!();\nfn bodyless();\n",
            Edition::E2021,
        ),
        DeclarationGrammar::Items,
    )
    .unwrap();
    let text = tree.materialize().unwrap().text;
    let range = |needle: &str| {
        let start = text.find(needle).unwrap() as u32;
        (start, start + needle.len() as u32)
    };
    let body = |open: &str, close_after: &str| {
        let start = text.find(open).unwrap() as u32;
        let end = start + text[start as usize..].find(close_after).unwrap() as u32 + close_after.len() as u32;
        (start, end)
    };
    let inner_body = body("{b!();}", "}");
    let outer_body = body("{a!();", "c!();}");
    for (name, owner) in [("a!()", outer_body), ("b!()", inner_body), ("c!()", outer_body)] {
        let (start, end) = range(name);
        assert_eq!(
            tree.call_owner_body(start, end, BodyOwners::Functions).unwrap(),
            Some(owner),
            "{name}"
        );
        assert_eq!(
            tree.call_owner_body(start, end, BodyOwners::Entered).unwrap(),
            Some(owner),
            "{name}"
        );
    }
    // A constant initializer owns its call only as an entered body.
    let (start, end) = range("d!()");
    assert_eq!(
        tree.call_owner_body(start, end, BodyOwners::Functions).unwrap(),
        None
    );
    assert!(
        tree.call_owner_body(start, end, BodyOwners::Entered)
            .unwrap()
            .is_some()
    );
    // A range that is not exactly one call is a typed error.
    assert!(tree.call_owner_body(start, end + 1, BodyOwners::Entered).is_err());
    // The body of the declaration occupying a range; none for a bodyless one.
    let (start, end) = range("fninner(){b!();}");
    assert_eq!(tree.declaration_body_at(start, end).unwrap(), Some(Some(inner_body)));
    let (start, end) = range("fnbodyless();");
    assert_eq!(tree.declaration_body_at(start, end).unwrap(), Some(None));
    let (start, end) = range("a!()");
    assert_eq!(tree.declaration_body_at(start, end).unwrap(), None);
}

#[test]
fn a_rooted_forest_equals_one_rebuilt_from_the_same_tables() {
    let forest = input("fn a() { call!(1, (2, 3)); }", Edition::E2021);
    let [TokenTree::Group { children, .. }] = forest.roots() else {
        panic!("one explicit group");
    };
    for child in children {
        let fast = forest.with_roots(vec![child.clone()]).unwrap();
        let rebuilt = TokenForest::new(
            vec![child.clone()],
            forest.contexts().to_vec(),
            forest.spans().to_vec(),
        )
        .unwrap();
        assert_eq!(fast, rebuilt);
        assert_eq!(fast.fingerprint(), rebuilt.fingerprint());
        assert_eq!(fast.encode().unwrap(), rebuilt.encode().unwrap());
    }
    // A root naming a span outside the tables is still refused.
    let [first, ..] = children.as_slice() else {
        panic!("children");
    };
    let mut foreign = first.clone();
    if let TokenTree::Ident { span, .. } | TokenTree::Literal { span, .. } = &mut foreign {
        *span = Key([9; 32]);
        assert!(forest.with_roots(vec![foreign]).is_err());
    }
    for record in forest.spans() {
        assert_eq!(forest.span_by_key(&record.key()), Some(record));
    }
    assert!(forest.span_by_key(&Key([9; 32])).is_none());
}

#[test]
fn span_keys_hash_the_same_stream_with_or_without_the_inline_buffer() {
    // The key is BLAKE3 of: len+domain, unit, content, len+path, start, end,
    // context, edition tag, origin. A long path crosses the inline buffer.
    for path in ["short.rs".to_string(), "p/".repeat(900)] {
        let record = SpanRecord {
            source: SourceIdentity {
                unit: Key([1; 32]),
                content: Key([2; 32]),
                path: path.clone(),
            },
            start: 5,
            end: 9,
            context: Key([3; 32]),
            edition: Edition::E2021,
            origin: Key([4; 32]),
        };
        let mut hasher = blake3::Hasher::new();
        let domain = "lrr/v2/token-span";
        hasher.update(&(domain.len() as u64).to_le_bytes());
        hasher.update(domain.as_bytes());
        hasher.update(&[1; 32]);
        hasher.update(&[2; 32]);
        hasher.update(&(path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update(&5u32.to_le_bytes());
        hasher.update(&9u32.to_le_bytes());
        hasher.update(&[3; 32]);
        hasher.update(&[2]);
        hasher.update(&[4; 32]);
        assert_eq!(record.key(), Key(*hasher.finalize().as_bytes()));
    }
}

#[test]
fn a_fragment_edit_is_found_without_visiting_earlier_nodes() {
    let mut tree = SyntaxAttachment::parse(
        &input(
            "macro_rules! one { () => { 1 } }\nfn main() { let a = one!(); let b = one!(); }\n",
            Edition::E2021,
        ),
        DeclarationGrammar::Items,
    )
    .unwrap();
    let mut length = tree.materialize().unwrap().text.len();
    for _ in 0..2 {
        let call = call_named(&tree, "one");
        let output = expansion(&tree, "one", &call);
        let edit = tree
            .attach_with_edit(call.start, call.end, &output, FragmentSlot::Expression)
            .unwrap();
        let (start, end) = edit.inserted();
        assert_eq!(
            tree.captures_in(start, end).unwrap().calls.len(),
            0,
            "the expansion of `one` has no call"
        );
        let text = tree.materialize().unwrap().text;
        assert_eq!(&text[start as usize..end as usize], "1");
        assert!(text.len() < length);
        length = text.len();
    }
    assert!(tree.materialize().unwrap().calls.is_empty());
}
