//! Isolated pinned macro component boundary. Upstream IDs remain private.
use serde::Serialize;
pub mod builtin_attributes;
pub mod builtin_derives;
pub mod builtin_macros;
pub mod compile_inputs;
pub mod concat;
pub mod declarative;
pub mod environment_arguments;
pub mod hygiene;
pub mod proc_client;
mod span_store;
pub mod token_wire;
use span::{
    Edition, EditionedFileId, FileId, ROOT_ERASED_FILE_AST_ID, Span, SpanAnchor, SyntaxContext,
    TextRange,
};

pub const UPSTREAM_COMMIT: &str = "bcf0514497034e1156f3cc2bb72b99cc9634a949";

/// E-R5-006: rustc imposes no time limit on expansion or procedural macros, so
/// every wait is bounded only by this unreachable (~136 year) duration, which
/// keeps `Instant` arithmetic well defined without counting time.
pub const NO_TIME_LIMIT: std::time::Duration = std::time::Duration::from_secs(u32::MAX as u64);

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapProbe {
    pub source_commit: String,
    pub editions: Vec<EditionProbe>,
}
#[derive(Debug, Clone, Serialize)]
pub struct EditionProbe {
    pub edition: u16,
    pub parsed_rules: u32,
}
pub use token_wire::WireError as AdapterError;

/// Synthetic syntax-only component probe, never an expansion of guest code.
pub fn bootstrap_probe() -> Result<BootstrapProbe, AdapterError> {
    let mut editions = Vec::new();
    for edition in Edition::iter() {
        let source = "($value:expr) => { $value }; ($name:ident, $($rest:tt)*) => { $name };";
        let span = Span {
            range: TextRange::empty(0.into()),
            anchor: SpanAnchor {
                file_id: EditionedFileId::new(FileId::from_raw(0), edition),
                ast_id: ROOT_ERASED_FILE_AST_ID,
            },
            ctx: SyntaxContext::root(edition),
        };
        let tokens = syntax_bridge::parse_to_token_tree_static_span(edition, span, source)
            .ok_or_else(|| AdapterError {
                code: "LRR-E-MACRO-ADAPTER",
                message: "pinned lexer rejected bootstrap tokens".into(),
            })?;
        let parsed = mbe::DeclarativeMacro::parse_macro_rules(&tokens, |_| edition);
        if let Some(error) = parsed.err() {
            return Err(AdapterError {
                code: "LRR-E-MACRO-ADAPTER",
                message: error.to_string(),
            });
        }
        let parsed_rules = u32::try_from(parsed.num_rules()).map_err(|_| AdapterError {
            code: "LRR-E-MACRO-ADAPTER",
            message: "rule count overflow".into(),
        })?;
        editions.push(EditionProbe {
            edition: edition.number() as u16,
            parsed_rules,
        });
    }
    Ok(BootstrapProbe {
        source_commit: UPSTREAM_COMMIT.into(),
        editions,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn unchanged_const_closure_tail_uses_pinned_ordinary_expression_parser() {
        use syntax::{AstNode, ast};
        let original = include_str!(
            "../../../fixtures/upstream/rustc-checkout/tests/ui/traits/const-traits/const-closure-parse-not-item.rs"
        );
        for edition in [syntax::Edition::Edition2015, syntax::Edition::Edition2018,
                        syntax::Edition::Edition2021, syntax::Edition::Edition2024] {
            let parsed = syntax::SourceFile::parse(original, edition);
            assert!(parsed.errors().is_empty(), "{edition:?}: {:?}", parsed.errors());
            let closures = parsed.tree().syntax().descendants()
                .filter_map(ast::ClosureExpr::cast).collect::<Vec<_>>();
            assert_eq!(closures.len(), 1);
            assert!(closures[0].const_token().is_some());
            assert!(closures[0].move_token().is_some());
        }
    }

    #[test]
    fn pinned_macro_components_parse_rules_in_all_supported_editions() {
        let probe = super::bootstrap_probe().unwrap();
        assert_eq!(probe.source_commit, super::UPSTREAM_COMMIT);
        assert_eq!(
            probe.editions.iter().map(|e| e.edition).collect::<Vec<_>>(),
            [2015, 2018, 2021, 2024]
        );
        assert!(probe.editions.iter().all(|e| e.parsed_rules == 2));
    }
}

pub mod derive_paths;

pub mod string_arguments;

pub mod nested_string;

pub mod string_output;

pub mod bytes_output;

pub mod concat_arguments;

pub mod grammar;

pub mod declaration_output;
pub mod item_output;

pub mod generated_attribute;
pub mod generated_derive;

pub mod runtime_arguments;

pub mod syntax_attachment;
