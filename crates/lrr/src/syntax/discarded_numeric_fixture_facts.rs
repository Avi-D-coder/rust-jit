//! Test-only owned facts from unchanged official discarded-expression bodies.
use crate::core::diagnostic::Diagnostic;
use ra_ap_syntax::{AstNode, ast, ast::HasName};

pub(crate) struct Facts {
    pub(crate) source_values: Vec<u64>,
    pub(crate) expected_binary: usize,
    pub(crate) sum: u64,
}
fn unsupported() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-UNSUPPORTED",
        "stock numeric fixture needs a decimal literal or single addition body".into(),
    )
}
pub(crate) fn capture(source: &str, function: &str) -> Result<Facts, Diagnostic> {
    let parsed = ra_ap_syntax::SourceFile::parse(source, ra_ap_syntax::Edition::Edition2024);
    if !parsed.errors().is_empty() {
        return Err(unsupported());
    }
    let body = parsed
        .syntax_node()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|item| item.name().is_some_and(|name| name.text() == function))
        .and_then(|item| item.body())
        .ok_or_else(unsupported)?;
    let mut source_values = Vec::new();
    let mut expected_binary = 0usize;
    for node in body.syntax().descendants() {
        if let Some(literal) = ast::Literal::cast(node.clone()) {
            source_values.try_reserve(1).map_err(|_| unsupported())?;
            source_values.push(literal.token().text().parse().map_err(|_| unsupported())?);
        }
        if let Some(binary) = ast::BinExpr::cast(node) {
            if !binary.op_token().is_some_and(|token| token.text() == "+") {
                return Err(unsupported());
            }
            expected_binary = expected_binary.checked_add(1).ok_or_else(unsupported)?;
            if expected_binary > 1 {
                return Err(unsupported());
            }
        }
    }
    if source_values.len() != expected_binary + 1 {
        return Err(unsupported());
    }
    let sum = source_values
        .iter()
        .try_fold(0u64, |sum, value| sum.checked_add(*value))
        .ok_or_else(unsupported)?;
    Ok(Facts {
        source_values,
        expected_binary,
        sum,
    })
}
