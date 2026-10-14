//! Private ephemeral upstream remapping. Stable records are supplied, not derived from IDs.
use super::*;
use span::{
    EditionedFileId, FileId, ROOT_ERASED_FILE_AST_ID, Span, SpanAnchor, SyntaxContext, TextRange,
};
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;
// Identifiers carry canonical semantic spelling, while their span continues
// to address the original source bytes. Literals never use this conversion.
fn canonical_identifier(text: &str) -> Result<String, AdapterError> {
    if text.len() > MAX_TEXT {
        return Err(limit("identifier text limit"));
    }
    let mut result = String::new();
    result
        .try_reserve(text.len())
        .map_err(|_| limit("identifier allocation"))?;
    for scalar in text.nfc() {
        result
            .len()
            .checked_add(scalar.len_utf8())
            .filter(|size| *size <= MAX_TEXT)
            .ok_or_else(|| limit("normalized identifier text limit"))?;
        result
            .try_reserve(scalar.len_utf8())
            .map_err(|_| limit("normalized identifier allocation"))?;
        result.push(scalar);
    }
    Ok(result)
}
pub(crate) fn edition(e: Edition) -> span::Edition {
    match e {
        Edition::E2015 => span::Edition::Edition2015,
        Edition::E2018 => span::Edition::Edition2018,
        Edition::E2021 => span::Edition::Edition2021,
        Edition::E2024 => span::Edition::Edition2024,
    }
}
pub(crate) fn delimiter(d: Delimiter) -> tt::DelimiterKind {
    match d {
        Delimiter::Parenthesis => tt::DelimiterKind::Parenthesis,
        Delimiter::Brace => tt::DelimiterKind::Brace,
        Delimiter::Bracket => tt::DelimiterKind::Bracket,
        Delimiter::Invisible => tt::DelimiterKind::Invisible,
    }
}
fn owned_delimiter(d: tt::DelimiterKind) -> Delimiter {
    match d {
        tt::DelimiterKind::Parenthesis => Delimiter::Parenthesis,
        tt::DelimiterKind::Brace => Delimiter::Brace,
        tt::DelimiterKind::Bracket => Delimiter::Bracket,
        tt::DelimiterKind::Invisible => Delimiter::Invisible,
    }
}
fn span(record: &SpanRecord, id: u32) -> Span {
    Span {
        range: TextRange::new(record.start.into(), record.end.into()),
        anchor: SpanAnchor {
            file_id: EditionedFileId::new(FileId::from_raw(id), edition(record.edition)),
            ast_id: ROOT_ERASED_FILE_AST_ID,
        },
        ctx: SyntaxContext::root(edition(record.edition)),
    }
}
pub(crate) fn group(
    d: tt::Delimiter,
    children: tt::TtIter<'_>,
    depth: usize,
    count: &mut usize,
    resolve: &mut impl FnMut(Span) -> Result<Key, AdapterError>,
) -> Result<TokenTree, AdapterError> {
    *count = count
        .checked_add(1)
        .ok_or_else(|| error("upstream token count overflow"))?;
    if depth > MAX_DEPTH || *count > MAX_ITEMS {
        return Err(limit("upstream token limit"));
    }
    let open = resolve(d.open)?;
    let close = resolve(d.close)?;
    let mut tokens = Vec::new();
    for token in children {
        tokens.push(match token {
            tt::TtElement::Subtree(s, c) => group(s.delimiter, c, depth + 1, count, resolve)?,
            tt::TtElement::Leaf(leaf) => {
                *count = count
                    .checked_add(1)
                    .ok_or_else(|| error("upstream token count overflow"))?;
                if *count > MAX_ITEMS {
                    return Err(limit("upstream token limit"));
                }
                let key = resolve(*leaf.span())?;
                match leaf {
                    tt::Leaf::Ident(i) => TokenTree::Ident {
                        text: canonical_identifier(i.sym.as_str())?,
                        raw: i.is_raw.yes(),
                        span: key,
                    },
                    tt::Leaf::Literal(l) => TokenTree::Literal {
                        token_text: l.to_string(),
                        span: key,
                    },
                    tt::Leaf::Punct(p) => TokenTree::Punct {
                        scalar: p.char,
                        spacing: match p.spacing {
                            tt::Spacing::Alone => Spacing::Alone,
                            tt::Spacing::Joint => Spacing::Joint,
                            tt::Spacing::JointHidden => Spacing::JointHidden,
                        },
                        span: key,
                    },
                }
            }
        });
    }
    Ok(TokenTree::Group {
        delimiter: owned_delimiter(d.kind),
        open,
        close,
        children: tokens,
    })
}
pub(super) fn parse(
    text: &str,
    source: SourceIdentity,
    context: ContextRecord,
    origin: Key,
    retained: &[ContextRecord],
    file_slot: u32,
) -> Result<TokenForest, AdapterError> {
    if text.len() > MAX_TEXT {
        return Err(limit("source token byte limit"));
    }
    if retained.is_empty() && (context.parent.is_some() || context.expansion.is_some()) {
        return Err(error("source tokens require a root context"));
    }
    let anchor = SpanAnchor {
        file_id: EditionedFileId::new(FileId::from_raw(file_slot), edition(context.edition)),
        ast_id: ROOT_ERASED_FILE_AST_ID,
    };
    // RawConverter reparses a standalone outer doc comment as a whole source file
    // and panics on this pinned snapshot. The supported syntax-node converter consumes
    // the actual comment token and retains the same pinned lexer/parser/frontend.
    struct SourceSpans {
        anchor: SpanAnchor,
        context: SyntaxContext,
    }
    impl syntax_bridge::SpanMapper for SourceSpans {
        fn span_for(&self, range: TextRange) -> Span {
            Span {
                range,
                anchor: self.anchor,
                ctx: self.context,
            }
        }
    }
    use syntax::AstNode;
    let parsed = syntax::SourceFile::parse(text, edition(context.edition));
    let root = Span {
        range: TextRange::new(
            0.into(),
            u32::try_from(text.len())
                .map_err(|_| error("source range overflow"))?
                .into(),
        ),
        anchor,
        ctx: SyntaxContext::root(edition(context.edition)),
    };
    let tree = syntax_bridge::syntax_node_to_token_tree(
        parsed.tree().syntax(),
        SourceSpans {
            anchor,
            context: root.ctx,
        },
        root,
        syntax_bridge::DocCommentDesugarMode::ProcMacro,
    );
    let mut records = BTreeMap::new();
    let root = group(
        tree.top_subtree().delimiter,
        tree.iter(),
        0,
        &mut 0,
        &mut |s| {
            let record = SpanRecord {
                source: source.clone(),
                start: s.range.start().into(),
                end: s.range.end().into(),
                context: context.key(),
                edition: context.edition,
                origin,
            };
            let key = record.key();
            records.insert(key, record);
            Ok(key)
        },
    )?;
    let mut contexts = retained
        .iter()
        .map(|c| (c.key(), c.clone()))
        .collect::<BTreeMap<_, _>>();
    contexts.insert(context.key(), context);
    TokenForest::new(
        vec![root],
        contexts.into_values().collect(),
        records.into_values().collect(),
    )
}
pub(crate) fn emit(
    tokens: &[TokenTree],
    builder: &mut tt::TopSubtreeBuilder,
    spans: &BTreeMap<Key, Span>,
) -> Result<(), AdapterError> {
    let get = |key: &Key| {
        spans
            .get(key)
            .copied()
            .ok_or_else(|| error("unmapped stable span"))
    };
    for token in tokens {
        match token {
            TokenTree::Group {
                delimiter: d,
                open,
                close,
                children,
            } => {
                builder.open(delimiter(*d), get(open)?);
                emit(children, builder, spans)?;
                builder.close(get(close)?);
            }
            TokenTree::Ident { text, raw, span } => {
                let canonical = canonical_identifier(text)?;
                let spelling = if *raw {
                    format!("r#{canonical}")
                } else {
                    canonical
                };
                builder.push(tt::Ident::new(&spelling, get(span)?).into());
            }
            TokenTree::Literal { token_text, span } => {
                let literal = tt::token_to_literal(token_text, get(span)?);
                if literal.to_string() != *token_text {
                    return Err(error("pinned literal conversion changed spelling"));
                }
                builder.push(literal.into());
            }
            TokenTree::Punct {
                scalar,
                spacing,
                span,
            } => builder.push(
                tt::Punct {
                    char: *scalar,
                    spacing: match spacing {
                        Spacing::Alone => tt::Spacing::Alone,
                        Spacing::Joint => tt::Spacing::Joint,
                        Spacing::JointHidden => tt::Spacing::JointHidden,
                    },
                    span: get(span)?,
                }
                .into(),
            ),
        }
    }
    Ok(())
}
/// Structural transport uses unique temporary file anchors as span slots, without constructing
/// upstream hygiene marks. M006/M007 attach marks in their private hygiene store.
pub(super) fn transport(f: &TokenForest, file_offset: u32) -> Result<TokenForest, AdapterError> {
    f.validate()?;
    let mut forward = BTreeMap::new();
    let mut reverse = HashMap::new();
    for (index, record) in f.spans().iter().enumerate() {
        let id = u32::try_from(index)
            .ok()
            .and_then(|i| i.checked_add(file_offset))
            .ok_or_else(|| error("temporary span slot overflow"))?;
        let s = span(record, id);
        forward.insert(record.key(), s);
        if reverse.insert(s, record.key()).is_some() {
            return Err(error("ambiguous temporary span binding"));
        }
    }
    let [
        TokenTree::Group {
            delimiter: d,
            open,
            close,
            children,
        },
    ] = f.roots()
    else {
        return Err(error("upstream transport requires one explicit root group"));
    };
    let get = |key: &Key| {
        forward
            .get(key)
            .copied()
            .ok_or_else(|| error("unmapped root span"))
    };
    let mut builder = tt::TopSubtreeBuilder::new(tt::Delimiter {
        kind: delimiter(*d),
        open: get(open)?,
        close: get(close)?,
    });
    emit(children, &mut builder, &forward)?;
    let tree = builder.build();
    let root = group(
        tree.top_subtree().delimiter,
        tree.iter(),
        0,
        &mut 0,
        &mut |s| {
            reverse
                .get(&s)
                .copied()
                .ok_or_else(|| error("upstream output has an unregistered span"))
        },
    )?;
    TokenForest::new(vec![root], f.contexts().to_vec(), f.spans().to_vec())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_token_remapping_is_independent_of_upstream_file_slots_and_preserves_spans() {
        let source = SourceIdentity {
            unit: Key([1; 32]),
            content: Key([2; 32]),
            path: "lib.rs".into(),
        };
        let context = ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: source.unit,
            edition: Edition::E2024,
        };
        let text =
            "/// docs\nfn r#type<'life>() { let _ = (1_000u32, r##\"text\"##); x::y >>= 2; }";
        let a = parse(text, source.clone(), context.clone(), Key([3; 32]), &[], 0).unwrap();
        let b = parse(text, source, context, Key([3; 32]), &[], 97).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_eq!(a, transport(&a, 200).unwrap());
        assert_eq!(a, transport(&a, 400).unwrap());
        assert!(a.spans().iter().any(|s| s.start != s.end));
        assert!(
            a.spans()
                .iter()
                .all(|s| usize::try_from(s.end).unwrap() <= text.len())
        );
    }
}
