//! Test-only owned length coordinates; parser objects stay in syntax.
use super::*;
pub(crate) fn length_sites(text: &str, name: &str, edition: u16) -> Vec<SourceRange> {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(
        &blob,
        ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        },
    )
    .unwrap();
    let function = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|f| f.name().is_some_and(|n| n.text() == name))
        .unwrap();
    function
        .syntax()
        .descendants()
        .filter_map(|node| {
            if let Some(a) = ast::ArrayType::cast(node.clone()) {
                a.const_arg()
                    .and_then(|c| c.syntax().children().find_map(ast::Expr::cast))
            } else if let Some(a) = ast::ArrayExpr::cast(node) {
                match a.kind() {
                    ast::ArrayExprKind::Repeat { repeat, .. } => repeat,
                    _ => None,
                }
            } else {
                None
            }
        })
        .map(|e| range(e.syntax().text_range()))
        .collect()
}
