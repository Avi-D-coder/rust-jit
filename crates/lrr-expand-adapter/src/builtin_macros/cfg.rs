//! `cfg!` predicate parsing and evaluation over owned input tokens
//! (rustc `rustc_builtin_macros/src/cfg.rs`, `rustc_attr_parsing/src/attributes/cfg.rs`:
//! `parse_cfg_entry`, `parse_name_value`, `eval_config_entry`). The caller owns the
//! configuration set; this module only reads the predicate syntax.
use super::tts::{is_path_segment_keyword, unquote_str};
use crate::token_wire::{Delimiter, Key, TokenTree};

/// rustc `CfgEntry`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CfgEntry {
    All(Vec<CfgEntry>),
    Any(Vec<CfgEntry>),
    Not(Box<CfgEntry>),
    Bool(bool),
    NameValue { name: String, value: Option<String> },
}

/// A predicate diagnostic: message and the offending token span.
pub(crate) type CfgError = (String, Option<Key>);

/// Flatten invisible groups (metavariable substitutions keep the inner tokens).
fn flatten(tokens: &[TokenTree], out: &mut Vec<TokenTree>) {
    for token in tokens {
        match token {
            TokenTree::Group {
                delimiter: Delimiter::Invisible,
                children,
                ..
            } => flatten(children, out),
            other => out.push(other.clone()),
        }
    }
}

fn span_of(token: &TokenTree) -> Key {
    match token {
        TokenTree::Group { open, .. } => *open,
        TokenTree::Ident { span, .. }
        | TokenTree::Punct { span, .. }
        | TokenTree::Literal { span, .. } => *span,
    }
}

struct Parser {
    tokens: Vec<TokenTree>,
    pos: usize,
}

impl Parser {
    fn new(tokens: &[TokenTree]) -> Self {
        let mut flat = Vec::new();
        flatten(tokens, &mut flat);
        Parser {
            tokens: flat,
            pos: 0,
        }
    }
    fn peek(&self) -> Option<&TokenTree> {
        self.tokens.get(self.pos)
    }
    fn at_punct(&self, ch: char) -> bool {
        matches!(self.peek(), Some(TokenTree::Punct { scalar, .. }) if *scalar == ch)
    }
    fn at_path_sep(&self) -> bool {
        matches!(
            (self.tokens.get(self.pos), self.tokens.get(self.pos + 1)),
            (
                Some(TokenTree::Punct {
                    scalar: ':',
                    spacing: crate::token_wire::Spacing::Joint,
                    ..
                }),
                Some(TokenTree::Punct { scalar: ':', .. })
            )
        )
    }
    fn error_here(&self, message: &str) -> CfgError {
        (message.to_owned(), self.peek().map(span_of))
    }

    /// rustc `MetaItemOrLitParser::parse_single` followed by `parse_cfg_entry`.
    fn entry(&mut self) -> Result<CfgEntry, CfgError> {
        let Some(first) = self.peek().cloned() else {
            return Err(self.error_here("expected a cfg-pattern"));
        };
        if let TokenTree::Literal { token_text, span } = &first {
            self.pos += 1;
            return Err((
                format!("expected a valid identifier here, found literal `{token_text}`"),
                Some(*span),
            ));
        }
        if let TokenTree::Ident {
            text, raw: false, ..
        } = &first
            && (text == "true" || text == "false")
            && !self.tokens.get(self.pos + 1).is_some_and(|next| {
                matches!(next, TokenTree::Punct { scalar: '=', .. })
                    || matches!(
                        next,
                        TokenTree::Group {
                            delimiter: Delimiter::Parenthesis,
                            ..
                        }
                    )
            })
        {
            self.pos += 1;
            return Ok(CfgEntry::Bool(text == "true"));
        }
        // Path.
        let path_span = span_of(&first);
        let mut segments = Vec::new();
        if self.at_path_sep() {
            self.pos += 2;
            segments.push(String::new());
        }
        loop {
            match self.peek() {
                Some(TokenTree::Ident { text, raw, .. }) => {
                    segments.push(if *raw {
                        format!("r#{text}")
                    } else {
                        text.clone()
                    });
                    self.pos += 1;
                }
                _ => return Err(self.error_here("expected identifier in cfg-pattern")),
            }
            if self.at_path_sep() {
                self.pos += 2;
                continue;
            }
            break;
        }
        let word = match segments.as_slice() {
            [single] if !single.is_empty() => Some(single.clone()),
            _ => None,
        };
        // Arguments.
        match self.peek().cloned() {
            Some(TokenTree::Group {
                delimiter: Delimiter::Parenthesis,
                children,
                open,
                ..
            }) => {
                self.pos += 1;
                let mut inner = Parser::new(&children);
                let mut items = Vec::new();
                while inner.peek().is_some() {
                    items.push(inner.entry()?);
                    if inner.peek().is_none() {
                        break;
                    }
                    if !inner.at_punct(',') {
                        return Err(inner.error_here("expected `,` in cfg-pattern list"));
                    }
                    inner.pos += 1;
                }
                match word.as_deref() {
                    Some("not") => match <[CfgEntry; 1]>::try_from(items) {
                        Ok([single]) => Ok(CfgEntry::Not(Box::new(single))),
                        Err(items) => Err((
                            format!("expected a single argument here, found {}", items.len()),
                            Some(open),
                        )),
                    },
                    Some("any") => Ok(CfgEntry::Any(items)),
                    Some("all") => Ok(CfgEntry::All(items)),
                    Some("target") => {
                        let mut result = Vec::new();
                        for item in items {
                            match item {
                                CfgEntry::NameValue {
                                    name,
                                    value: Some(value),
                                } => result.push(CfgEntry::NameValue {
                                    name: format!("target_{name}"),
                                    value: Some(value),
                                }),
                                _ => {
                                    return Err((
                                        "expected a name-value pair in `target(..)`".into(),
                                        Some(open),
                                    ));
                                }
                            }
                        }
                        Ok(CfgEntry::All(result))
                    }
                    Some("version") => Err((
                        "`cfg(version(..))` is unstable and not supported by this adapter".into(),
                        Some(path_span),
                    )),
                    _ => Err((
                        "expected one of `any`, `all`, `not` or `target` here".into(),
                        Some(path_span),
                    )),
                }
            }
            Some(TokenTree::Group { open, .. }) => {
                Err(("expected `(` delimited cfg arguments".into(), Some(open)))
            }
            Some(TokenTree::Punct { scalar: '=', .. }) => {
                self.pos += 1;
                let name = word
                    .filter(|w| !is_path_segment_keyword(w))
                    .ok_or_else(|| {
                        (
                            "expected a valid identifier here".to_owned(),
                            Some(path_span),
                        )
                    })?;
                let value = match self.peek() {
                    Some(TokenTree::Literal { token_text, span }) => {
                        let value = unquote_str(token_text).ok_or_else(|| {
                            (
                                format!("expected a string literal here, found `{token_text}`"),
                                Some(*span),
                            )
                        })?;
                        self.pos += 1;
                        value
                    }
                    _ => return Err(self.error_here("expected a string literal here")),
                };
                Ok(CfgEntry::NameValue {
                    name: name.trim_start_matches("r#").to_owned(),
                    value: Some(value),
                })
            }
            _ => {
                let name = word
                    .filter(|w| !is_path_segment_keyword(w))
                    .ok_or_else(|| {
                        (
                            "expected a valid identifier here".to_owned(),
                            Some(path_span),
                        )
                    })?;
                Ok(CfgEntry::NameValue {
                    name: name.trim_start_matches("r#").to_owned(),
                    value: None,
                })
            }
        }
    }
}

/// rustc `parse_cfg` for `cfg!`: one predicate, optional trailing comma.
pub(crate) fn parse(tokens: &[TokenTree]) -> Result<CfgEntry, CfgError> {
    let mut parser = Parser::new(tokens);
    if parser.peek().is_none() {
        return Err(("macro requires a cfg-pattern as an argument".into(), None));
    }
    let entry = parser.entry()?;
    if parser.at_punct(',') {
        parser.pos += 1;
    }
    if parser.peek().is_some() {
        return Err(parser.error_here("expected 1 cfg-pattern"));
    }
    Ok(entry)
}

/// Caller-owned `sess.config.contains(&(name, value))`.
pub(crate) type CfgOption<'a, E> = dyn FnMut(&str, Option<&str>) -> Result<bool, E> + 'a;

/// rustc `eval_config_entry` with a caller-owned configuration oracle.
pub(crate) fn eval<E>(entry: &CfgEntry, option: &mut CfgOption<'_, E>) -> Result<bool, E> {
    Ok(match entry {
        CfgEntry::All(subs) => {
            for sub in subs {
                if !eval(sub, option)? {
                    return Ok(false);
                }
            }
            true
        }
        CfgEntry::Any(subs) => {
            for sub in subs {
                if eval(sub, option)? {
                    return Ok(true);
                }
            }
            false
        }
        CfgEntry::Not(sub) => !eval(sub, option)?,
        CfgEntry::Bool(b) => *b,
        CfgEntry::NameValue { name, value } => option(name, value.as_deref())?,
    })
}
