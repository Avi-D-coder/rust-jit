//! Owned syntax facts for architecture audits; no upstream nodes escape.
use ra_ap_syntax::{AstNode, Edition, SyntaxKind, ast};

#[derive(Debug, Default)]
pub struct SourceFacts {
    pub paths: Vec<String>,
    pub unsafe_tokens: usize,
    pub parse_errors: usize,
}

pub fn source_facts(text: &str) -> SourceFacts {
    let parsed = ast::SourceFile::parse(text, Edition::Edition2024);
    let root = parsed.tree();
    let mut facts = SourceFacts {
        parse_errors: parsed.errors().len(),
        ..SourceFacts::default()
    };
    for path in root.syntax().descendants().filter_map(ast::Path::cast) {
        facts.paths.push(
            path.syntax()
                .text()
                .to_string()
                .split_whitespace()
                .collect(),
        );
    }
    fn collect(tree: ast::UseTree, prefix: &str, paths: &mut Vec<String>) {
        let own = tree
            .path()
            .map(|p| {
                p.syntax()
                    .text()
                    .to_string()
                    .split_whitespace()
                    .collect::<String>()
            })
            .unwrap_or_default();
        let path = match (prefix.is_empty(), own.is_empty()) {
            (true, _) => own,
            (_, true) => prefix.to_owned(),
            _ => format!("{prefix}::{own}"),
        };
        if let Some(list) = tree.use_tree_list() {
            for child in list.use_trees() {
                collect(child, &path, paths);
            }
        } else if !path.is_empty() {
            paths.push(path);
        }
    }
    for item in root.syntax().descendants().filter_map(ast::Use::cast) {
        if let Some(tree) = item.use_tree() {
            collect(tree, "", &mut facts.paths);
        }
    }
    facts.paths.sort();
    facts.paths.dedup();
    facts.unsafe_tokens = root
        .syntax()
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == SyntaxKind::UNSAFE_KW)
        .count();
    facts
}

#[cfg(test)]
mod tests {
    #[test]
    fn grouped_imports_and_aliases_are_owned_paths() {
        let facts = super::source_facts("use std::{fs as files, process::{Command}};");
        assert_eq!(facts.parse_errors, 0);
        assert!(facts.paths.contains(&"std::fs".to_owned()));
        assert!(facts.paths.contains(&"std::process::Command".to_owned()));
    }
    #[test]
    fn strings_and_comments_do_not_create_unsafe_tokens() {
        let facts = super::source_facts("// unsafe fn bad() {}\nconst S: &str = \"unsafe\";");
        assert_eq!(facts.unsafe_tokens, 0);
    }
}
