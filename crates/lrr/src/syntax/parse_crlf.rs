//! Revalidate the pinned lexer's CRLF continuation errors without changing the
//! original syntax tree, source identity, or offsets. This is lexical evidence,
//! never type checking or permission to execute a macro.
use ra_ap_syntax::{AstNode, Edition, SyntaxKind, SyntaxNode, TextRange, ast};

// The pinned lexer emits all diagnostics for one token consecutively. Keep a
// single lexical revalidation result, so many continuations in one large token
// cannot cause repeated whole-token parses. Other messages never use this cache.
#[derive(Default)]
pub(super) struct TokenErrors {
    last: Option<(TextRange, SyntaxKind, bool)>,
}
impl TokenErrors {
    pub(super) fn normalized_literal_error(
        &mut self,
        root: &SyntaxNode,
        error_range: TextRange,
        message: &str,
        edition: Edition,
    ) -> bool {
        let kind = match message {
            "unknown character escape" => SyntaxKind::STRING,
            "unknown byte escape" => SyntaxKind::BYTE_STRING,
            _ => return false,
        };
        if let Some((range, prior_kind, admitted)) = self.last
            && range == error_range
            && prior_kind == kind
        {
            return admitted;
        }
        let admitted = normalized_literal_error(root, error_range, message, edition);
        self.last = Some((error_range, kind, admitted));
        admitted
    }
}

fn normalized_literal_error(
    root: &SyntaxNode,
    error_range: TextRange,
    message: &str,
    edition: Edition,
) -> bool {
    let kind = match message {
        "unknown character escape" => SyntaxKind::STRING,
        "unknown byte escape" => SyntaxKind::BYTE_STRING,
        _ => return false,
    };
    if error_range.is_empty() || !root.text_range().contains_range(error_range) {
        return false;
    }
    let Some(token) = root.covering_element(error_range).into_token() else {
        return false;
    };
    let original = token.text();
    if token.text_range() != error_range
        || token.kind() != kind
        || original.len() > 1024 * 1024
        || !original.contains("\\\r\n")
    {
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
