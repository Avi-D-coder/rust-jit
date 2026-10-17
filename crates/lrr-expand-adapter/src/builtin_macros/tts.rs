//! rustc token-stream model and `print_tts` port (`rustc_ast_pretty/src/pprust/state.rs`:
//! `print_tt`, `print_tts`, `print_mac_common`, `space_between`, `idents_would_merge`).
//!
//! The owned wire stores Rust-analyzer token trees: multi-character operators are
//! split into single punctuation leaves and identifiers/literals carry no spacing.
//! rustc's printer needs glued tokens and lexer `Spacing`. Both are recovered from
//! the retained span records:
//! - glue: rustc `Token::glue` over punctuation that is `Joint` or source-adjacent;
//! - spacing of tokens in an unmarked (root) context: source adjacency to the next
//!   sibling, as rustc's lexer computes it;
//! - tokens carried by any expansion mark are `Spacing::Alone`, because rustc's
//!   `macro_rules` transcriber pushes every template token as `Alone`
//!   (`rustc_expand/src/mbe/transcribe.rs`), while delimiters keep their template
//!   spacing and `]` is forced `Alone`;
//! - Rust-analyzer wraps substituted `$e:expr` fragments in a parenthesis group
//!   whose open and close delimiters share the metavariable span; rustc uses an
//!   invisible group with `Alone` spacing. Such groups print as invisible groups.
//!
//! Known approximation: a root-context token whose next sibling is not its original
//! source neighbour (a captured `tt` followed by template tokens) is treated as
//! `Alone`; rustc keeps the original neighbour's spacing.
use super::pp::{BoxMarker, Printer};
use crate::{
    AdapterError,
    token_wire::{ContextRecord, Delimiter, Edition, Key, SpanRecord, TokenForest, TokenTree},
};
use std::collections::BTreeMap;

pub(crate) const INDENT_UNIT: isize = 4;

fn fault(message: impl Into<String>) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-BUILTIN",
        message: message.into(),
    }
}

/// Read-only view of an owned forest's span and context tables.
pub(crate) struct SpanIndex<'a> {
    spans: BTreeMap<Key, &'a SpanRecord>,
    contexts: BTreeMap<Key, &'a ContextRecord>,
}

impl<'a> SpanIndex<'a> {
    pub(crate) fn new(forest: &'a TokenForest) -> Self {
        Self {
            spans: forest.spans().iter().map(|s| (s.key(), s)).collect(),
            contexts: forest.contexts().iter().map(|c| (c.key(), c)).collect(),
        }
    }
    pub(crate) fn record(&self, key: Key) -> Result<&'a SpanRecord, AdapterError> {
        self.spans
            .get(&key)
            .copied()
            .ok_or_else(|| fault("token references an unknown span record"))
    }
    fn context(&self, key: Key) -> Result<&'a ContextRecord, AdapterError> {
        let span = self.record(key)?;
        self.contexts
            .get(&span.context)
            .copied()
            .ok_or_else(|| fault("span references an unknown context record"))
    }
    /// rustc `Span::edition`: the edition of the span's syntax context.
    pub(crate) fn edition(&self, key: Key) -> Result<Edition, AdapterError> {
        Ok(self.context(key)?.edition)
    }
    /// True for tokens never transcribed by an expansion (lexer spacing applies).
    pub(crate) fn is_root(&self, key: Key) -> Result<bool, AdapterError> {
        Ok(self.context(key)?.parent.is_none())
    }
    /// Source adjacency: `a` ends exactly where `b` starts in the same source.
    pub(crate) fn adjacent(&self, a: Key, b: Key) -> Result<bool, AdapterError> {
        let a = self.record(a)?;
        let b = self.record(b)?;
        Ok(a.source == b.source && a.start < a.end && a.end == b.start)
    }
}

/// rustc `tokenstream::Spacing`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Spacing {
    Alone,
    Joint,
    JointHidden,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Delim {
    Parenthesis,
    Bracket,
    Brace,
    Invisible,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum TokKind {
    /// `reserved`: rustc `Ident::is_reserved` in the identifier's edition.
    Ident {
        raw: bool,
        reserved: bool,
    },
    Lifetime,
    Literal,
    /// A glued rustc punctuation token, e.g. `::`, `..=`, `'`.
    Punct,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Tok {
    pub(crate) kind: TokKind,
    /// Printed spelling (identifiers without `r#`; see `kind`).
    pub(crate) text: String,
    pub(crate) first: Key,
    pub(crate) last: Key,
}

impl Tok {
    pub(crate) fn printed(&self) -> String {
        match self.kind {
            TokKind::Ident { raw: true, .. } => format!("r#{}", self.text),
            _ => self.text.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Tt {
    Token(Tok, Spacing),
    Delimited {
        delim: Delim,
        open: Spacing,
        close: Spacing,
        children: Vec<Tt>,
        /// Open delimiter span of a visible group (source adjacency).
        open_key: Option<Key>,
    },
}

/// rustc `Symbol::is_reserved` for an identifier spelling in an edition.
pub(crate) fn is_reserved(text: &str, edition: Edition) -> bool {
    const SPECIAL: &[&str] = &["$crate", "{{root}}", "_"];
    const USED: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while",
    ];
    const UNUSED: &[&str] = &[
        "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof",
        "unsized", "virtual", "yield",
    ];
    let at_least_2018 = edition != Edition::E2015;
    let at_least_2024 = edition == Edition::E2024;
    SPECIAL.contains(&text)
        || USED.contains(&text)
        || UNUSED.contains(&text)
        || (matches!(text, "async" | "await" | "dyn") && at_least_2018)
        || (text == "try" && at_least_2018)
        || (text == "gen" && at_least_2024)
}

/// rustc `Symbol::is_path_segment_keyword`.
pub(crate) fn is_path_segment_keyword(text: &str) -> bool {
    matches!(
        text,
        "super" | "self" | "Self" | "crate" | "{{root}}" | "$crate"
    )
}

/// rustc `Token::glue` over punctuation spellings.
fn glue(left: &str, right: char) -> Option<&'static str> {
    Some(match (left, right) {
        ("=", '=') => "==",
        ("=", '>') => "=>",
        ("<", '=') => "<=",
        ("<", '<') => "<<",
        ("<", '-') => "<-",
        ("<<", '=') => "<<=",
        (">", '=') => ">=",
        (">", '>') => ">>",
        (">>", '=') => ">>=",
        ("!", '=') => "!=",
        ("+", '=') => "+=",
        ("-", '=') => "-=",
        ("-", '>') => "->",
        ("*", '=') => "*=",
        ("/", '=') => "/=",
        ("%", '=') => "%=",
        ("^", '=') => "^=",
        ("&", '=') => "&=",
        ("&", '&') => "&&",
        ("|", '=') => "|=",
        ("|", '|') => "||",
        (".", '.') => "..",
        ("..", '.') => "...",
        ("..", '=') => "..=",
        (":", ':') => "::",
        _ => return None,
    })
}

/// `$crate` printing policy (`convert_dollar_crate` in rustc's printer).
pub(crate) type DollarCrate<'f> = Option<&'f dyn Fn(Key) -> Result<String, AdapterError>>;

/// rustc `IdentPrinter` conversion of `$crate`: `crate` stays a path keyword,
/// an extern crate name prints as `::name`.
pub(crate) fn converted_dollar_crate(name: String) -> String {
    if is_path_segment_keyword(&name) {
        name
    } else {
        format!("::{name}")
    }
}

enum Unit {
    Token {
        tok: Tok,
        /// Wire spacing of the last punctuation leaf (Joint means an adjacent op follows).
        joint: bool,
        fixed: Option<Spacing>,
    },
    Group {
        delim: Delim,
        open: Key,
        close: Key,
        children: Vec<Tt>,
        fixed: Option<(Spacing, Spacing)>,
    },
}

impl Unit {
    fn first_key(&self) -> Option<Key> {
        match self {
            Unit::Token { tok, .. } => Some(tok.first),
            Unit::Group {
                delim: Delim::Invisible,
                ..
            } => None,
            Unit::Group { open, .. } => Some(*open),
        }
    }
    fn is_punct(&self) -> bool {
        matches!(
            self,
            Unit::Token {
                tok: Tok {
                    kind: TokKind::Punct,
                    ..
                },
                ..
            }
        )
    }
}

/// Build rustc token trees from owned wire children.
pub(crate) fn build(
    children: &[TokenTree],
    index: &SpanIndex<'_>,
    dollar_crate: DollarCrate<'_>,
) -> Result<Vec<Tt>, AdapterError> {
    let mut units = Vec::new();
    let mut i = 0;
    while i < children.len() {
        if let Some((consumed, mut doc)) = desugared_doc_comment(&children[i..], index)? {
            units.append(&mut doc);
            i += consumed;
            continue;
        }
        match &children[i] {
            TokenTree::Group {
                delimiter,
                open,
                close,
                children: inner,
            } => {
                let synthetic = *delimiter == Delimiter::Parenthesis && open == close;
                let delim = match delimiter {
                    _ if synthetic => Delim::Invisible,
                    Delimiter::Parenthesis => Delim::Parenthesis,
                    Delimiter::Bracket => Delim::Bracket,
                    Delimiter::Brace => Delim::Brace,
                    Delimiter::Invisible => Delim::Invisible,
                };
                units.push(Unit::Group {
                    delim,
                    open: *open,
                    close: *close,
                    children: build(inner, index, dollar_crate)?,
                    fixed: None,
                });
                i += 1;
            }
            TokenTree::Ident { text, raw, span } => {
                let edition = index.edition(*span)?;
                let printed = if text == "$crate"
                    && !raw
                    && let Some(convert) = dollar_crate
                {
                    converted_dollar_crate(convert(*span)?)
                } else {
                    text.clone()
                };
                units.push(Unit::Token {
                    tok: Tok {
                        kind: TokKind::Ident {
                            raw: *raw,
                            reserved: is_reserved(text, edition),
                        },
                        text: printed,
                        first: *span,
                        last: *span,
                    },
                    joint: false,
                    fixed: None,
                });
                i += 1;
            }
            TokenTree::Literal { token_text, span } => {
                units.push(Unit::Token {
                    tok: Tok {
                        kind: TokKind::Literal,
                        text: token_text.clone(),
                        first: *span,
                        last: *span,
                    },
                    joint: false,
                    fixed: None,
                });
                i += 1;
            }
            TokenTree::Punct {
                scalar,
                spacing,
                span,
            } => {
                let joined = |spacing: &crate::token_wire::Spacing, a: Key, b: Key| {
                    Ok::<_, AdapterError>(
                        *spacing == crate::token_wire::Spacing::Joint || index.adjacent(a, b)?,
                    )
                };
                // Lifetimes: rustc lexes `'a` as one token.
                if *scalar == '\''
                    && let Some(TokenTree::Ident {
                        text,
                        raw,
                        span: name,
                    }) = children.get(i + 1)
                    && joined(spacing, *span, *name)?
                {
                    let text = if *raw {
                        format!("'r#{text}")
                    } else {
                        format!("'{text}")
                    };
                    units.push(Unit::Token {
                        tok: Tok {
                            kind: TokKind::Lifetime,
                            text,
                            first: *span,
                            last: *name,
                        },
                        joint: false,
                        fixed: None,
                    });
                    i += 2;
                    continue;
                }
                let mut glued = String::from(*scalar);
                let mut last = *span;
                let mut last_spacing = *spacing;
                let mut j = i + 1;
                while let Some(TokenTree::Punct {
                    scalar: next,
                    spacing: next_spacing,
                    span: next_span,
                }) = children.get(j)
                {
                    if !joined(&last_spacing, last, *next_span)? {
                        break;
                    }
                    let Some(combined) = glue(&glued, *next) else {
                        break;
                    };
                    glued = combined.to_owned();
                    last = *next_span;
                    last_spacing = *next_spacing;
                    j += 1;
                }
                units.push(Unit::Token {
                    tok: Tok {
                        kind: TokKind::Punct,
                        text: glued,
                        first: *span,
                        last,
                    },
                    joint: last_spacing == crate::token_wire::Spacing::Joint,
                    fixed: None,
                });
                i = j;
            }
        }
    }
    let mut result = Vec::with_capacity(units.len());
    for position in 0..units.len() {
        let next = units.get(position + 1);
        let tt = match &units[position] {
            Unit::Token { tok, joint, fixed } => {
                let spacing = match fixed {
                    Some(spacing) => *spacing,
                    None => token_spacing(tok, *joint, next, index)?,
                };
                Tt::Token(tok.clone(), spacing)
            }
            Unit::Group {
                delim,
                open,
                close,
                children,
                fixed,
            } => {
                let (open_spacing, close_spacing) = match (delim, fixed) {
                    (_, Some(fixed)) => *fixed,
                    (Delim::Invisible, None) => (Spacing::Alone, Spacing::Alone),
                    (_, None) => {
                        let open_spacing = match children.first().and_then(tt_first_key) {
                            Some(first) if index.adjacent(*open, first)? => Spacing::JointHidden,
                            _ => Spacing::Alone,
                        };
                        let forced_alone = *delim == Delim::Bracket && !index.is_root(*close)?;
                        let close_spacing = match next.and_then(Unit::first_key) {
                            Some(next_key)
                                if !forced_alone && index.adjacent(*close, next_key)? =>
                            {
                                if next.is_some_and(Unit::is_punct) {
                                    Spacing::Joint
                                } else {
                                    Spacing::JointHidden
                                }
                            }
                            _ => Spacing::Alone,
                        };
                        (open_spacing, close_spacing)
                    }
                };
                Tt::Delimited {
                    delim: *delim,
                    open: open_spacing,
                    close: close_spacing,
                    children: children.clone(),
                    open_key: (*delim != Delim::Invisible).then_some(*open),
                }
            }
        };
        result.push(tt);
    }
    Ok(result)
}

fn tt_first_key(tt: &Tt) -> Option<Key> {
    match tt {
        Tt::Token(tok, _) => Some(tok.first),
        Tt::Delimited { open_key, .. } => *open_key,
    }
}

fn token_spacing(
    tok: &Tok,
    joint: bool,
    next: Option<&Unit>,
    index: &SpanIndex<'_>,
) -> Result<Spacing, AdapterError> {
    let Some(next) = next else {
        return Ok(Spacing::Alone);
    };
    if !index.is_root(tok.last)? {
        return Ok(Spacing::Alone);
    }
    let lexed_adjacent = match next.first_key() {
        Some(key) => index.adjacent(tok.last, key)?,
        None => false,
    };
    if lexed_adjacent || (joint && next.is_punct()) {
        Ok(if next.is_punct() {
            Spacing::Joint
        } else {
            Spacing::JointHidden
        })
    } else {
        Ok(Spacing::Alone)
    }
}

/// Rust-analyzer desugars doc comments (`DocCommentDesugarMode::ProcMacro`) into
/// `#[doc = "..."]` whose tokens all share the comment span. rustc's
/// `TokenStream::desugar_doc_comments` produces a raw string literal with
/// `#` `JointHidden`, `[` `JointHidden`, `]` `Alone`.
fn desugared_doc_comment(
    rest: &[TokenTree],
    index: &SpanIndex<'_>,
) -> Result<Option<(usize, Vec<Unit>)>, AdapterError> {
    let Some(TokenTree::Punct {
        scalar: '#',
        span: pound,
        ..
    }) = rest.first()
    else {
        return Ok(None);
    };
    let (bang, group_at) = match rest.get(1) {
        Some(TokenTree::Punct {
            scalar: '!', span, ..
        }) if span == pound => (Some(*span), 2),
        _ => (None, 1),
    };
    let Some(TokenTree::Group {
        delimiter: Delimiter::Bracket,
        open,
        close,
        children,
    }) = rest.get(group_at)
    else {
        return Ok(None);
    };
    let [
        TokenTree::Ident {
            text: doc,
            raw: false,
            span: doc_span,
        },
        TokenTree::Punct {
            scalar: '=',
            span: eq_span,
            ..
        },
        TokenTree::Literal {
            token_text,
            span: literal_span,
        },
    ] = children.as_slice()
    else {
        return Ok(None);
    };
    if doc != "doc"
        || [open, close, doc_span, eq_span, literal_span]
            .iter()
            .any(|span| *span != pound)
    {
        return Ok(None);
    }
    let Some(data) = unquote_str(token_text) else {
        return Ok(None);
    };
    // rustc `desugared_tts`: minimum hashes wrapping the text.
    let mut hashes = 0usize;
    let mut count = 0usize;
    for ch in data.chars() {
        count = match ch {
            '"' => 1,
            '#' if count > 0 => count + 1,
            _ => 0,
        };
        hashes = hashes.max(count);
    }
    let fence = "#".repeat(hashes);
    let literal = format!("r{fence}\"{data}\"{fence}");
    let edition = index.edition(*pound)?;
    let token = |kind: TokKind, text: &str| Tok {
        kind,
        text: text.to_owned(),
        first: *pound,
        last: *pound,
    };
    let mut units = vec![Unit::Token {
        tok: token(TokKind::Punct, "#"),
        joint: false,
        fixed: Some(if bang.is_some() {
            Spacing::Joint
        } else {
            Spacing::JointHidden
        }),
    }];
    if bang.is_some() {
        units.push(Unit::Token {
            tok: token(TokKind::Punct, "!"),
            joint: false,
            fixed: Some(Spacing::JointHidden),
        });
    }
    units.push(Unit::Group {
        delim: Delim::Bracket,
        open: *open,
        close: *close,
        children: vec![
            Tt::Token(
                token(
                    TokKind::Ident {
                        raw: false,
                        reserved: is_reserved("doc", edition),
                    },
                    "doc",
                ),
                Spacing::Alone,
            ),
            Tt::Token(token(TokKind::Punct, "="), Spacing::Alone),
            Tt::Token(token(TokKind::Literal, &literal), Spacing::Alone),
        ],
        fixed: Some((Spacing::JointHidden, Spacing::Alone)),
    });
    Ok(Some((group_at + 1, units)))
}

/// Value of a (raw) string literal token, or `None` for other literals.
pub(crate) fn unquote_str(token: &str) -> Option<String> {
    if let Some(raw) = token.strip_prefix('r') {
        let hashes = raw.chars().take_while(|c| *c == '#').count();
        let body = raw.get(hashes..)?;
        let body = body.strip_prefix('"')?;
        let body = body.strip_suffix(&"#".repeat(hashes))?;
        let body = body.strip_suffix('"')?;
        return Some(body.to_owned());
    }
    let body = token.strip_prefix('"')?;
    let end = body.rfind('"')?;
    if !body[end + 1..].is_empty() {
        return None; // suffixed string literals are rejected by rustc here
    }
    unescape_str(&body[..end])
}

/// Cooked string unescaping (rustc_literal_escaper `unescape_str` rules).
pub(crate) fn unescape_str(body: &str) -> Option<String> {
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' && chars.peek() == Some(&'\n') {
            continue; // CRLF in source is normalized by the lexer
        }
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '\\' => out.push('\\'),
            '0' => out.push('\0'),
            '\'' => out.push('\''),
            '"' => out.push('"'),
            'x' => {
                let hi = chars.next()?.to_digit(16)?;
                let lo = chars.next()?.to_digit(16)?;
                let value = hi * 16 + lo;
                if value > 0x7f {
                    return None;
                }
                out.push(char::from_u32(value)?);
            }
            'u' => {
                if chars.next()? != '{' {
                    return None;
                }
                let mut value = 0u32;
                let mut digits = 0;
                loop {
                    let c = chars.next()?;
                    if c == '}' {
                        break;
                    }
                    if c == '_' {
                        continue;
                    }
                    value = value.checked_mul(16)?.checked_add(c.to_digit(16)?)?;
                    digits += 1;
                }
                if digits == 0 {
                    return None;
                }
                out.push(char::from_u32(value)?);
            }
            '\n' => {
                // Line continuation: skip the newline and leading whitespace.
                while chars
                    .peek()
                    .is_some_and(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
                {
                    chars.next();
                }
            }
            '\r' if chars.peek() == Some(&'\n') => {
                chars.next();
                while chars
                    .peek()
                    .is_some_and(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
                {
                    chars.next();
                }
            }
            _ => return None,
        }
    }
    Some(out)
}

fn is_punct(tt: &Tt) -> bool {
    matches!(
        tt,
        Tt::Token(
            Tok {
                kind: TokKind::Punct,
                ..
            },
            _
        )
    )
}

fn punct_is(tt: &Tt, spelling: &str) -> bool {
    matches!(tt, Tt::Token(Tok { kind: TokKind::Punct, text, .. }, _) if text == spelling)
}

/// rustc `space_between`.
fn space_between(tt1: &Tt, tt2: &Tt) -> bool {
    if punct_is(tt1, ".") && !is_punct(tt2) {
        return false;
    }
    if punct_is(tt1, "$")
        && matches!(
            tt2,
            Tt::Token(
                Tok {
                    kind: TokKind::Ident { .. },
                    ..
                },
                _
            )
        )
    {
        return false;
    }
    if !is_punct(tt1) && (punct_is(tt2, ",") || punct_is(tt2, ";") || punct_is(tt2, ".")) {
        return false;
    }
    if let Tt::Token(
        Tok {
            kind: TokKind::Ident { raw, reserved },
            text,
            ..
        },
        _,
    ) = tt1
    {
        if punct_is(tt2, "!") && (!reserved || *raw) {
            return false;
        }
        if matches!(
            tt2,
            Tt::Delimited {
                delim: Delim::Parenthesis,
                ..
            }
        ) && (*raw || matches!(text.as_str(), "fn" | "Self" | "pub") || !reserved)
        {
            return false;
        }
    }
    if punct_is(tt1, "#")
        && matches!(
            tt2,
            Tt::Delimited {
                delim: Delim::Bracket,
                ..
            }
        )
    {
        return false;
    }
    true
}

fn is_ident_like(tt: &Tt) -> bool {
    matches!(
        tt,
        Tt::Token(
            Tok {
                kind: TokKind::Ident { .. },
                ..
            },
            _
        )
    )
}

/// rustc `print_tt`, returning the tree's trailing spacing.
fn print_tt(p: &mut Printer, tt: &Tt) -> Spacing {
    match tt {
        Tt::Token(tok, spacing) => {
            p.word(tok.printed());
            *spacing
        }
        Tt::Delimited {
            delim,
            open,
            close,
            children,
            ..
        } => {
            print_delimited(p, *delim, Some(*open), children);
            *close
        }
    }
}

/// rustc `print_tts`.
pub(crate) fn print_tts(p: &mut Printer, tts: &[Tt]) {
    let mut iter = tts.iter().peekable();
    while let Some(tt) = iter.next() {
        let spacing = print_tt(p, tt);
        if let Some(next) = iter.peek() {
            let space = if spacing == Spacing::Alone {
                space_between(tt, next)
            } else {
                // Captured identifiers can keep `Joint` spacing; never merge them.
                is_ident_like(tt) && is_ident_like(next)
            };
            if space {
                p.space();
            }
        }
    }
}

/// Header of `print_mac_common`: an already printed path (and `!`).
pub(crate) enum MacHeader {
    None,
    /// The caller printed `path!`.
    Bang,
}

/// rustc `print_mac_common` with `ident: None`.
pub(crate) fn print_mac_common(
    p: &mut Printer,
    header: MacHeader,
    delim: Delim,
    open_spacing: Option<Spacing>,
    tts: &[Tt],
    header_printer: impl FnOnce(&mut Printer),
) {
    let cb: Option<BoxMarker> = (delim == Delim::Brace).then(|| p.cbox(INDENT_UNIT));
    header_printer(p);
    match delim {
        Delim::Brace => {
            if matches!(header, MacHeader::Bang) {
                p.nbsp();
            }
            p.word("{");
            let open_space = matches!(open_spacing, None | Some(Spacing::Alone)) && !tts.is_empty();
            if open_space {
                p.space();
            }
            let ib = p.ibox(0);
            print_tts(p, tts);
            p.end(ib);
            // rustc `bclose(span, !open_space, cb)`.
            if open_space {
                p.break_offset_if_not_bol(1, -INDENT_UNIT);
            }
            p.word("}");
            if let Some(cb) = cb {
                p.end(cb);
            }
        }
        delim => {
            let (open, close) = match delim {
                Delim::Parenthesis => ("(", ")"),
                Delim::Bracket => ("[", "]"),
                Delim::Brace | Delim::Invisible => ("", ""),
            };
            p.word(open);
            let ib = p.ibox(0);
            print_tts(p, tts);
            p.end(ib);
            p.word(close);
            if let Some(cb) = cb {
                p.end(cb);
            }
        }
    }
}

fn print_delimited(p: &mut Printer, delim: Delim, open: Option<Spacing>, children: &[Tt]) {
    print_mac_common(p, MacHeader::None, delim, open, children, |_| {});
}

/// rustc `pprust::tts_to_string`.
pub(crate) fn tts_to_string(tts: &[Tt]) -> String {
    let mut p = Printer::new();
    print_tts(&mut p, tts);
    p.eof()
}
