use lrr_expand_adapter::token_wire::*;
use proptest::prelude::*;
fn fixture() -> (ContextRecord, SpanRecord, SpanRecord) {
    let context = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([7; 32]),
        edition: Edition::E2024,
    };
    let source = SourceIdentity {
        unit: Key([7; 32]),
        content: Key([9; 32]),
        path: "generated/value.rs".into(),
    };
    let open = SpanRecord {
        source: source.clone(),
        start: 2,
        end: 3,
        context: context.key(),
        edition: Edition::E2024,
        origin: Key([4; 32]),
    };
    let close = SpanRecord {
        start: 20,
        end: 21,
        ..open.clone()
    };
    (context, open, close)
}

#[test]
fn retained_token_allocations_count_capacities_without_changing_semantic_identity() {
    let (context, open, close) = fixture();
    let small = TokenForest::new(
        vec![TokenTree::Literal {
            token_text: "43_i64".into(),
            span: open.key(),
        }],
        vec![context.clone()],
        vec![open.clone(), close.clone()],
    )
    .unwrap();
    let mut text = String::with_capacity(1024);
    text.push_str("43_i64");
    let mut roots = Vec::with_capacity(32);
    roots.push(TokenTree::Literal {
        token_text: text,
        span: open.key(),
    });
    let mut contexts = Vec::with_capacity(32);
    contexts.push(context);
    let mut spans = Vec::with_capacity(32);
    spans.extend([open, close]);
    let large = TokenForest::new(roots, contexts, spans).unwrap();
    assert_eq!(small.fingerprint(), large.fingerprint());
    assert_eq!(small.encode().unwrap(), large.encode().unwrap());
    assert!(large.allocated_bytes().unwrap() > small.allocated_bytes().unwrap() + 1024);
    let decoded = TokenForest::decode(&large.encode().unwrap()).unwrap();
    assert!(large.allocated_bytes().unwrap() > decoded.allocated_bytes().unwrap());
}
#[test]
fn owned_tokens_round_trip_invisible_lifetimes_joint_raw_docs_and_nested_groups() {
    let (context, open, close) = fixture();
    let span = open.key();
    let roots = vec![TokenTree::Group {
        delimiter: Delimiter::Invisible,
        open: span,
        close: close.key(),
        children: vec![
            TokenTree::Punct {
                scalar: '\'',
                spacing: Spacing::Joint,
                span,
            },
            TokenTree::Ident {
                text: "life".into(),
                raw: false,
                span,
            },
            TokenTree::Punct {
                scalar: ':',
                spacing: Spacing::Joint,
                span,
            },
            TokenTree::Punct {
                scalar: ':',
                spacing: Spacing::Alone,
                span,
            },
            TokenTree::Ident {
                text: "type".into(),
                raw: true,
                span,
            },
            TokenTree::Group {
                delimiter: Delimiter::Bracket,
                open: span,
                close: close.key(),
                children: vec![
                    TokenTree::Ident {
                        text: "doc".into(),
                        raw: false,
                        span,
                    },
                    TokenTree::Punct {
                        scalar: '=',
                        spacing: Spacing::JointHidden,
                        span,
                    },
                    TokenTree::Literal {
                        token_text: "r##\"doc text\"##".into(),
                        span,
                    },
                ],
            },
            TokenTree::Group {
                delimiter: Delimiter::Brace,
                open: close.key(),
                close: span,
                children: vec![],
            },
        ],
    }];
    let forest = TokenForest::new(roots, vec![context], vec![close, open]).unwrap();
    let decoded = TokenForest::decode(&forest.encode().unwrap()).unwrap();
    assert_eq!(forest, decoded);
    assert_eq!(forest, transport_round_trip(&forest).unwrap());
    assert_eq!(forest.fingerprint(), decoded.fingerprint());
}
#[test]
fn token_wire_rejects_unknown_schema_missing_contexts_ranges_duplicates_and_depth() {
    let (context, open, close) = fixture();
    assert!(TokenForest::new(vec![], vec![context.clone(), context.clone()], vec![]).is_err());
    assert!(TokenForest::new(vec![], vec![], vec![open.clone()]).is_err());
    let mut bad = open.clone();
    bad.start = 50;
    assert!(TokenForest::new(vec![], vec![context.clone()], vec![bad]).is_err());
    assert!(
        TokenForest::new(
            vec![TokenTree::Ident {
                text: "a".into(),
                raw: false,
                span: Key([0; 32])
            }],
            vec![context.clone()],
            vec![open.clone()]
        )
        .is_err()
    );
    let mut nested = TokenTree::Ident {
        text: "a".into(),
        raw: false,
        span: open.key(),
    };
    for _ in 0..300 {
        nested = TokenTree::Group {
            delimiter: Delimiter::Invisible,
            open: open.key(),
            close: close.key(),
            children: vec![nested],
        };
    }
    assert!(
        TokenForest::new(
            vec![nested],
            vec![context.clone()],
            vec![open.clone(), close]
        )
        .is_err()
    );
    let forest = TokenForest::new(vec![], vec![context], vec![open]).unwrap();
    let mut wire = forest.encode().unwrap();
    wire[4..8].copy_from_slice(&99u32.to_le_bytes());
    assert!(TokenForest::decode(&wire).is_err());
}
proptest! {
    #[test]
    fn randomized_owned_token_round_trips_preserve_spelling_spacing_and_stable_contexts(values in prop::collection::vec((0u8..4, 0u8..3, "[a-z]{1,12}", any::<u32>()),0..80)) {
        let (root, open, close)=fixture();
        let marked=ContextRecord{parent:Some(root.key()),expansion:Some(Key([6;32])),transparency:Transparency::SemiTransparent,defining_unit:Key([8;32]),edition:Edition::E2021};
        let span=SpanRecord{context:marked.key(),edition:Edition::E2021,..open.clone()};
        let key=span.key();
        let roots=values.into_iter().map(|(kind,spacing,text,value)|match kind {
            0=>TokenTree::Ident{text,raw:value%2==0,span:key},
            1=>TokenTree::Literal{token_text:format!("{value}_u32"),span:key},
            2=>TokenTree::Punct{scalar:'>',spacing:[Spacing::Alone,Spacing::Joint,Spacing::JointHidden][usize::from(spacing)],span:key},
            _=>TokenTree::Group{delimiter:Delimiter::Parenthesis,open:key,close:close.key(),children:vec![]},
        }).collect::<Vec<_>>();
        let a=TokenForest::new(roots.clone(),vec![root.clone(),marked.clone()],vec![open.clone(),close.clone(),span.clone()]).unwrap();
        let b=TokenForest::new(roots,vec![marked,root],vec![span,close,open]).unwrap();
        prop_assert_eq!(a.fingerprint(),b.fingerprint());
        prop_assert_eq!(a.encoded_len().unwrap(), a.encode().unwrap().len());
        prop_assert_eq!(b.encoded_len().unwrap(), a.encoded_len().unwrap());
        prop_assert_eq!(a.encode().unwrap(),b.encode().unwrap());
        prop_assert_eq!(&a,&TokenForest::decode(&a.encode().unwrap()).unwrap());
    }
}

#[test]
fn checked_binary_transport_rejects_forged_lengths_tags_indices_and_truncation() {
    let (context, open, _) = fixture();
    let forest = TokenForest::new(
        vec![TokenTree::Ident {
            text: "value".into(),
            raw: false,
            span: open.key(),
        }],
        vec![context],
        vec![open],
    )
    .unwrap();
    let bytes = forest.encode().unwrap();
    for end in 0..bytes.len() {
        assert!(TokenForest::decode(&bytes[..end]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(TokenForest::decode(&trailing).is_err());
    let mut count = bytes.clone();
    count[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        TokenForest::decode(&count).unwrap_err().code,
        "LRR-E-MACRO-OUTPUT"
    );
    let mut span = bytes.clone();
    let end = span.len();
    span[end - 4..].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(TokenForest::decode(&span).is_err());
    let mut tag = bytes;
    let end = tag.len();
    tag[end - 15] = 99;
    assert!(TokenForest::decode(&tag).is_err());
}
#[test]
fn deep_token_transport_preserves_nested_groups_without_json_recursion_limits() {
    let (context, open, close) = fixture();
    let mut token = TokenTree::Literal {
        token_text: "42_u64".into(),
        span: open.key(),
    };
    for index in 0..180 {
        token = TokenTree::Group {
            delimiter: if index % 2 == 0 {
                Delimiter::Invisible
            } else {
                Delimiter::Parenthesis
            },
            open: open.key(),
            close: close.key(),
            children: vec![token],
        };
    }
    let forest = TokenForest::new(vec![token], vec![context], vec![open, close]).unwrap();
    let bytes = forest.encode().unwrap();
    assert_eq!(forest, TokenForest::decode(&bytes).unwrap());
    assert_eq!(forest, transport_round_trip(&forest).unwrap());
}

#[test]
fn token_wire_enforces_actual_revision4_token_and_wire_byte_boundaries() {
    const TOKENS: usize = 2_000_000;
    const BYTES: usize = 32 * 1024 * 1024;
    let (context, open, _) = fixture();
    let leaf = TokenTree::Punct {
        scalar: '.',
        spacing: Spacing::Alone,
        span: open.key(),
    };
    let forest = TokenForest::new(
        vec![leaf.clone(); TOKENS],
        vec![context.clone()],
        vec![open.clone()],
    )
    .unwrap();
    let bytes = forest.encode().unwrap();
    assert!(bytes.len() < BYTES);
    let decoded = TokenForest::decode(&bytes).unwrap();
    assert_eq!(decoded.roots().len(), TOKENS);
    assert_eq!(forest.fingerprint(), decoded.fingerprint());
    drop(decoded);
    drop(forest);
    drop(bytes);
    assert_eq!(
        TokenForest::new(
            vec![leaf; TOKENS + 1],
            vec![context.clone()],
            vec![open.clone()]
        )
        .unwrap_err()
        .code,
        "LRR-E-MACRO-OUTPUT"
    );
    let small = TokenForest::new(
        vec![TokenTree::Literal {
            token_text: "\"\"".into(),
            span: open.key(),
        }],
        vec![context.clone()],
        vec![open.clone()],
    )
    .unwrap();
    let overhead = small.encode().unwrap().len() - 2;
    let literal = format!("\"{}\"", "a".repeat(BYTES - overhead - 2));
    let forest = TokenForest::new(
        vec![TokenTree::Literal {
            token_text: literal,
            span: open.key(),
        }],
        vec![context.clone()],
        vec![open.clone()],
    )
    .unwrap();
    let bytes = forest.encode().unwrap();
    assert_eq!(bytes.len(), BYTES);
    assert_eq!(forest.encoded_len().unwrap(), BYTES);
    assert!(forest.encoded_len_with_limit(BYTES - 1).is_err());
    assert_eq!(
        forest.fingerprint(),
        TokenForest::decode(&bytes).unwrap().fingerprint()
    );
    drop(forest);
    drop(bytes);
    let oversized = format!("\"{}\"", "a".repeat(BYTES - overhead - 1));
    let forest = TokenForest::new(
        vec![TokenTree::Literal {
            token_text: oversized,
            span: open.key(),
        }],
        vec![context],
        vec![open],
    )
    .unwrap();
    assert_eq!(forest.encode().unwrap_err().code, "LRR-E-MACRO-OUTPUT");
    assert_eq!(forest.encoded_len().unwrap_err().code, "LRR-E-MACRO-OUTPUT");
}

#[test]
fn span_location_and_resolution_context_preserve_independent_editions_and_origins() {
    let (root, source, _) = fixture();
    let definition = ContextRecord {
        parent: None,
        expansion: None,
        transparency: Transparency::Opaque,
        defining_unit: Key([8; 32]),
        edition: Edition::E2021,
    };
    let other = SpanRecord {
        source: SourceIdentity {
            unit: Key([8; 32]),
            content: Key([10; 32]),
            path: "macro.rs".into(),
        },
        start: 50,
        end: 55,
        context: definition.key(),
        edition: Edition::E2021,
        origin: Key([5; 32]),
    };
    let resolved = source.resolved_at(&other);
    assert_eq!(resolved.source, source.source);
    assert_eq!(resolved.edition, Edition::E2024);
    assert_eq!(resolved.context, definition.key());
    assert_eq!(resolved.origin, source.origin);
    let located = source.located_at(&other);
    assert_eq!(located.source, other.source);
    assert_eq!(located.edition, Edition::E2021);
    assert_eq!(located.context, root.key());
    assert_eq!(located.origin, source.origin);
    assert_eq!((located.start, located.end), (50, 55));
    let forest = TokenForest::new(
        vec![],
        vec![root, definition],
        vec![source, other, resolved, located],
    )
    .unwrap();
    assert_eq!(
        forest,
        TokenForest::decode(&forest.encode().unwrap()).unwrap()
    );
}
#[test]
fn decoder_counts_pending_siblings_before_nested_reservations() {
    let (context, span, _) = fixture();
    let mut bytes = TokenForest::new(vec![], vec![context], vec![span])
        .unwrap()
        .encode()
        .unwrap();
    let last = bytes.len();
    bytes[last - 4..].copy_from_slice(&2_000_000u32.to_le_bytes());
    bytes.extend_from_slice(&[0, 3]); // group/invisible
    bytes.extend_from_slice(&0u32.to_le_bytes()); // open span
    bytes.extend_from_slice(&0u32.to_le_bytes()); // close span
    bytes.extend_from_slice(&2_000_000u32.to_le_bytes()); // nested children
    bytes.resize(bytes.len() + 2_000_000 * 7, 0);
    assert_eq!(
        TokenForest::decode(&bytes).unwrap_err().code,
        "LRR-E-MACRO-OUTPUT"
    );
}

#[test]
fn counting_codec_matches_wire_and_refuses_reduced_limits_without_output_buffer() {
    let (root, open, close) = fixture();
    let marked = ContextRecord {
        parent: Some(root.key()),
        expansion: Some(Key([3; 32])),
        transparency: Transparency::SemiTransparent,
        defining_unit: Key([4; 32]),
        edition: Edition::E2021,
    };
    let changed = SpanRecord {
        context: marked.key(),
        ..open.clone()
    };
    let forest = TokenForest::new(
        vec![TokenTree::Group {
            delimiter: Delimiter::Invisible,
            open: open.key(),
            close: close.key(),
            children: vec![
                TokenTree::Ident {
                    text: "type".into(),
                    raw: true,
                    span: changed.key(),
                },
                TokenTree::Literal {
                    token_text: "r###\"héllo\"###".into(),
                    span: changed.key(),
                },
                TokenTree::Punct {
                    scalar: '>',
                    spacing: Spacing::JointHidden,
                    span: close.key(),
                },
            ],
        }],
        vec![root, marked],
        vec![open, close, changed],
    )
    .unwrap();
    let wire = forest.encode().unwrap();
    assert_eq!(forest.encoded_len().unwrap(), wire.len());
    assert_eq!(
        forest.encoded_len_with_limit(wire.len()).unwrap(),
        wire.len()
    );
    assert_eq!(
        forest
            .encoded_len_with_limit(wire.len() - 1)
            .unwrap_err()
            .code,
        "LRR-E-MACRO-OUTPUT"
    );
    assert!(forest.encoded_len_with_limit(32 * 1024 * 1024 + 1).is_err());
    let repeated = TokenForest::decode(&wire).unwrap();
    assert_eq!(repeated.encoded_len().unwrap(), wire.len());
}
