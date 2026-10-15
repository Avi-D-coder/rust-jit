//! Revalidate the pinned lexer's CRLF continuation errors without changing the
//! original syntax tree, source identity, or offsets. This is lexical evidence,
//! never type checking or permission to execute a macro.
use ra_ap_syntax::{AstNode, Edition, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, ast};

// The pinned lexer emits all diagnostics for one token consecutively. Keep a
// single lexical revalidation result, so many continuations in one large token
// cannot cause repeated whole-token parses. Other messages never use this cache.
#[derive(Default)]
pub(super) struct TokenErrors {
    last: Option<(TextRange, SyntaxKind, bool, bool)>,
}
impl TokenErrors {
    pub(super) fn normalized_literal_error(
        &mut self,
        root: &SyntaxNode,
        error_range: TextRange,
        message: &str,
        edition: Edition,
    ) -> bool {
        let Some(token) = error_token(root, error_range, message) else {
            return false;
        };
        let kind = token.kind();
        let token_range = token.text_range();
        if let Some((range, prior_kind, admitted, continuation)) = self.last
            && range == token_range
            && prior_kind == kind
        {
            return admitted && (error_range.is_empty() || continuation);
        }
        let continuation = token.text().contains("\\\r\n");
        let admitted = normalized_literal(&token, edition);
        self.last = Some((token_range, kind, admitted, continuation));
        admitted && (error_range.is_empty() || continuation)
    }
}

fn error_token(root: &SyntaxNode, error_range: TextRange, message: &str) -> Option<SyntaxToken> {
    if !root.text_range().contains_range(error_range) {
        return None;
    }
    if !error_range.is_empty() {
        let kind = match message {
            "unknown character escape" => SyntaxKind::STRING,
            "unknown byte escape" => SyntaxKind::BYTE_STRING,
            _ => return None,
        };
        let token = root.covering_element(error_range).into_token()?;
        return (token.kind() == kind && token.text_range() == error_range).then_some(token);
    }
    let expected: &[u8] = match message {
        "Invalid escape" => b"\\\r\n",
        "Character must be escaped: `\r`" => b"\r\n",
        _ => return None,
    };
    let token = root.token_at_offset(error_range.start()).right_biased()?;
    if !matches!(token.kind(), SyntaxKind::STRING | SyntaxKind::BYTE_STRING)
        || !token
            .parent()
            .is_some_and(|node| ast::Literal::can_cast(node.kind()))
    {
        return None;
    }
    // Pinned syntax0.0.356 validation.rs uses prefix_len=1 for both literal
    // kinds, although byte-string content starts after two bytes. Account for
    // that exact diagnostic convention without altering any original offset.
    let byte_prefix = usize::from(token.kind() == SyntaxKind::BYTE_STRING);
    let offset = usize::try_from(u32::from(error_range.start() - token.text_range().start()))
        .ok()?
        .checked_add(byte_prefix)?;
    token
        .text()
        .as_bytes()
        .get(offset..)?
        .starts_with(expected)
        .then_some(token)
}

fn normalized_literal(token: &SyntaxToken, edition: Edition) -> bool {
    let original = token.text();
    let kind = token.kind();
    if original.len() > 1024 * 1024 || !original.contains("\r\n") {
        return false;
    }
    // The wrapper is a lexical/grammar probe only. Retained ASTs always contain
    // the original bytes. Invalid escapes elsewhere in this same token must
    // still fail the complete normalized-token parse.
    let prefix = "const __LRR_LITERAL:()=";
    let Some(capacity) = original.len().checked_add(prefix.len() + 1) else {
        return false;
    };
    let mut wrapper = String::new();
    if wrapper.try_reserve_exact(capacity).is_err() {
        return false;
    }
    wrapper.push_str(prefix);
    for line in original.split_inclusive('\n') {
        if let Some(text) = line.strip_suffix("\r\n") {
            wrapper.push_str(text);
            wrapper.push('\n');
        } else {
            wrapper.push_str(line);
        }
    }
    let end = wrapper.len();
    wrapper.push(';');
    let parsed = ast::SourceFile::parse(&wrapper, edition);
    if !parsed.errors().is_empty() {
        return false;
    }
    parsed
        .tree()
        .syntax()
        .descendants_with_tokens()
        .filter_map(|item| item.into_token())
        .any(|item| {
            item.kind() == kind
                && u32::from(item.text_range().start()) as usize == prefix.len()
                && u32::from(item.text_range().end()) as usize == end
                && item.text() == &wrapper[prefix.len()..end]
        })
}

#[cfg(test)]
#[path = "parse_crlf_tests.rs"]
mod tests;
