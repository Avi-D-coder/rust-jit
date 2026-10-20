//! Actual crate-level source settings, selected by the common cfg preprocessor.
use super::parse::ParsedFile;
use crate::{core::diagnostic::Diagnostic, source::snapshot::CfgSet};
use ra_ap_syntax::{AstNode, SyntaxKind, ast::{self, HasAttrs}};
use std::collections::BTreeSet;

#[derive(Debug)]
pub(crate) struct SolverCrateConfiguration {
    pub(crate) features: BTreeSet<String>,
    pub(crate) recursion_limit: usize,
    pub(crate) rustc_no_implicit_bounds: bool,
}
impl ParsedFile {
    pub(crate) fn solver_crate_configuration(&self, cfg: &CfgSet)
        -> Result<SolverCrateConfiguration, Diagnostic> {
        let tree = self.parsed.tree();
        let (_, metas) = super::attributes::effective_metas(tree.attrs().filter_map(|attr| attr.meta()).collect(), cfg)?;
        let mut features = BTreeSet::new(); let mut recursion_limit = None;
        let mut rustc_no_implicit_bounds = false;
        for meta in metas {
            let name = meta.path().map(|path| path.syntax().text().to_string())
                .or_else(|| meta.simple_name().map(|name| name.to_string())).unwrap_or_default();
            match (name.strip_prefix("r#").unwrap_or(&name), meta) {
                ("rustc_no_implicit_bounds", ast::Meta::PathMeta(_)) => {
                    rustc_no_implicit_bounds = true;
                }
                ("rustc_no_implicit_bounds", _) => {
                    return Err(fault("rustc_no_implicit_bounds configuration syntax is not represented"));
                }
                ("feature", ast::Meta::TokenTreeMeta(meta)) => {
                    let tree = meta.token_tree().ok_or_else(|| fault("feature token tree absent"))?;
                    for token in tree.syntax().children_with_tokens().filter_map(|node| node.into_token()) {
                        match token.kind() {
                            SyntaxKind::L_PAREN | SyntaxKind::R_PAREN | SyntaxKind::COMMA
                            | SyntaxKind::WHITESPACE | SyntaxKind::COMMENT => {},
                            SyntaxKind::IDENT => { features.insert(token.text().trim_start_matches("r#").to_owned()); },
                            _ => return Err(fault("feature attribute has unsupported source tokens")),
                        }
                    }
                }
                ("recursion_limit", ast::Meta::KeyValueMeta(meta)) if recursion_limit.is_none() => {
                    let Some(ast::Expr::Literal(literal)) = meta.expr() else { return Err(fault("recursion_limit source value is not a literal")); };
                    let ast::LiteralKind::String(value) = literal.kind() else { return Err(fault("recursion_limit source value is not a string")); };
                    recursion_limit = Some(value.value().map_err(|_| fault("recursion_limit string differs"))?
                        .parse().map_err(|_| fault("recursion_limit value is not representable"))?);
                }
                _ => {},
            }
        }
        // Pinned compiler/rustc_interface/src/limits.rs:33-41. No LRR limit.
        Ok(SolverCrateConfiguration { features, recursion_limit: recursion_limit.unwrap_or(128), rustc_no_implicit_bounds })
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into()) }
