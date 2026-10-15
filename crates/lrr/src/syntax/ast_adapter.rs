//! Owned syntax facts; upstream AST values remain inside this module boundary.
use super::spans::SourceRange;
use crate::core::digest::{StableEncode, StableEncoder};
use ra_ap_syntax::{
    AstNode, Edition, SyntaxNode, TextRange,
    ast::{self, HasModuleItem, HasName},
};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct TokenRecipe {
    pub kind: u16,
    pub text: String,
}
impl StableEncode for TokenRecipe {
    const DOMAIN: &'static str = "lrr/v2/syntax-token";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u16(self.kind);
        e.string(&self.text);
    }
}
pub(crate) fn range(range: TextRange) -> SourceRange {
    SourceRange {
        start: u32::from(range.start()),
        end: u32::from(range.end()),
    }
}
pub(crate) fn tokens(node: &SyntaxNode, excluded: Option<TextRange>) -> Vec<TokenRecipe> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| {
            !t.kind().is_trivia() && !excluded.is_some_and(|r| r.contains_range(t.text_range()))
        })
        .map(|t| TokenRecipe {
            kind: t.kind() as u16,
            text: t.text().to_owned(),
        })
        .collect()
}
pub fn token_text(tokens: &[TokenRecipe]) -> String {
    tokens
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Validate one source identifier using the pinned parser. Keep upstream AST
/// handles inside the syntax boundary, including for edition-sensitive names.
pub(crate) fn macro_name(text: &str, edition: u16) -> Option<String> {
    let edition = match edition {
        2015 => Edition::Edition2015,
        2018 => Edition::Edition2018,
        2021 => Edition::Edition2021,
        2024 => Edition::Edition2024,
        _ => return None,
    };
    // The token wire ceiling bounds owned source spellings before parsing.
    if text.len() > 32 * 1024 * 1024 {
        return None;
    }
    // The pinned parser accepts some invalid path keywords structurally. Use
    // its lexer-backed identifier helpers as well as the exact AST spelling.
    let unescaped = text.strip_prefix("r#");
    let valid = match unescaped {
        Some(name) => {
            ra_ap_syntax::utils::is_identifier(name, edition)
                || ra_ap_syntax::utils::is_raw_identifier(name, edition)
        }
        None => ra_ap_syntax::utils::is_identifier(text, edition),
    };
    if !valid {
        return None;
    }
    let parsed = ast::SourceFile::parse(&format!("fn {text}() {{}}"), edition);
    if !parsed.errors().is_empty() {
        return None;
    }
    let tree = parsed.tree();
    let mut items = tree.items();
    let ast::Item::Fn(function) = items.next()? else {
        return None;
    };
    if items.next().is_some() {
        return None;
    }
    let name = function.name()?;
    if name.syntax().text() != text {
        return None;
    }
    semantic_name(text)
}

/// Canonicalize an already validated AST/owned semantic name. Semantic names
/// may be reserved source keywords after a raw prefix has been removed.
pub(crate) fn semantic_name(text: &str) -> Option<String> {
    if text.len() > 32 * 1024 * 1024 {
        return None;
    }
    // NFC is identity normalization, not compatibility folding. Keep the
    // original source token/range unchanged; normalize only the owned name.
    let mut normalized = String::new();
    normalized.try_reserve(text.len()).ok()?;
    for scalar in text.strip_prefix("r#").unwrap_or(text).nfc() {
        normalized
            .len()
            .checked_add(scalar.len_utf8())
            .filter(|size| *size <= 32 * 1024 * 1024)?;
        normalized.try_reserve(scalar.len_utf8()).ok()?;
        normalized.push(scalar);
    }
    Some(normalized)
}

/// Prepared module contexts are semantic paths, independent of source spelling.
pub(crate) fn semantic_module_path(path: &[String]) -> Option<Vec<String>> {
    if path.len() > 256 {
        return None;
    }
    let mut result = Vec::new();
    result.try_reserve(path.len()).ok()?;
    let mut bytes = 0usize;
    for part in path {
        let canonical = semantic_name(part)?;
        if canonical.is_empty() {
            return None;
        }
        bytes = bytes
            .checked_add(canonical.len())
            .filter(|n| *n <= 32 * 1024 * 1024)?;
        result.push(canonical);
    }
    Some(result)
}

/// Decode a bounded owned string token using the pinned AST implementation.
pub(crate) fn string_value(text: &str, edition: u16) -> Option<String> {
    use ra_ap_syntax::{AstToken, ast::IsString};
    if text.len() > 128 * 1024 {
        return None;
    }
    let edition = match edition {
        2015 => Edition::Edition2015,
        2018 => Edition::Edition2018,
        2021 => Edition::Edition2021,
        2024 => Edition::Edition2024,
        _ => return None,
    };
    // Rust normalizes CRLF before lexing, but source blobs and the caller's
    // spelling/span remain original. This scratch AST returns only text data.
    let mut normalized = String::new();
    let text = if text.contains("\r\n") {
        normalized.try_reserve_exact(text.len()).ok()?;
        for line in text.split_inclusive('\n') {
            if let Some(prefix) = line.strip_suffix("\r\n") {
                normalized.push_str(prefix);
                normalized.push('\n');
            } else {
                normalized.push_str(line);
            }
        }
        normalized.as_str()
    } else {
        text
    };
    let mut wrapper = String::new();
    wrapper
        .try_reserve_exact(text.len().checked_add("const V:&str=;".len())?)
        .ok()?;
    wrapper.push_str("const V:&str=");
    wrapper.push_str(text);
    wrapper.push(';');
    let parsed = ast::SourceFile::parse(&wrapper, edition);
    if !parsed.errors().is_empty() {
        return None;
    }
    let token = parsed
        .tree()
        .syntax()
        .descendants_with_tokens()
        .filter_map(|n| n.into_token())
        .find_map(ast::String::cast)?;
    if token.text() != text {
        return None;
    }
    let range = token.text_range_between_quotes()?;
    let content = &token.text()[range - token.syntax().text_range().start()];
    let mut value = String::new();
    value.try_reserve_exact(content.len()).ok()?;
    let mut failed = false;
    let mut emit = |_, item: Result<char, ra_ap_syntax::unescape::EscapeError>| match item {
        Ok(item) => value.push(item),
        Err(error) => failed |= error.is_fatal(),
    };
    if token.is_raw() {
        ra_ap_syntax::unescape::check_raw_str(content, &mut emit);
    } else {
        ra_ap_syntax::unescape::unescape_str(content, &mut emit);
    }
    (!failed).then_some(value)
}

#[cfg(test)]
#[path = "string_token_newline_tests.rs"]
mod string_token_newline_tests;
