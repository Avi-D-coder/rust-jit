use ra_ap_syntax::{AstNode, Edition, ast};

/// Parse syntax only. This intentionally never resolves the cold missing call.
pub fn parser_smoke() -> Result<(), String> {
    let text = "fn live() -> i64 { 4_i64 } fn cold() { absent(); }";
    let parsed = ast::SourceFile::parse(text, Edition::Edition2024);
    if !parsed.errors().is_empty() {
        return Err(format!("parse errors: {:?}", parsed.errors()));
    }
    let count = parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .count();
    if count != 2 {
        return Err(format!("expected 2 functions, got {count}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parser_counts_cold_function_without_resolving() {
        super::parser_smoke().unwrap();
    }
}
