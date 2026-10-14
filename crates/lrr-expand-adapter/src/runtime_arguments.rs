//! Pinned expression-list parsing of actual captured argument tokens.
//! Parser scaffolding is owned token syntax; displayed output is never relexed.
use crate::{
    AdapterError,
    span_store::SpanStore,
    token_wire::{Delimiter, TokenForest, TokenTree},
};
use syntax::{
    AstNode,
    ast::{self, HasArgList},
};
fn fault(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-RUNTIME-MACRO-ARGUMENTS",
        message: message.into(),
    }
}
pub fn expressions(input: &TokenForest) -> Result<Vec<TokenForest>, AdapterError> {
    if input.encoded_len()? > 32 * 1024 * 1024 || input.allocated_bytes()? > 64 * 1024 * 1024 {
        return Err(fault("runtime argument token budget"));
    }
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    let delimiter = tree.top_subtree().delimiter;
    let mut builder = tt::TopSubtreeBuilder::new(tt::Delimiter {
        kind: tt::DelimiterKind::Invisible,
        open: delimiter.open,
        close: delimiter.close,
    });
    builder.push(tt::Ident::new("__lrr_arguments", delimiter.open).into());
    builder.open(tt::DelimiterKind::Parenthesis, delimiter.open);
    builder.extend_with_tt(tree.token_trees());
    builder.close(delimiter.close);
    let wrapped = builder.build();
    let (parsed, map) = syntax_bridge::token_tree_to_syntax_node(
        &wrapped,
        parser::TopEntryPoint::Expr,
        &mut |context| context.edition(store.database()),
    );
    if !parsed.errors().is_empty() {
        return Err(fault(
            "runtime arguments are not a complete expression list",
        ));
    }
    let call = ast::CallExpr::cast(parsed.syntax_node())
        .ok_or_else(|| fault("runtime argument call scaffold absent"))?;
    let args = call
        .arg_list()
        .ok_or_else(|| fault("runtime argument list scaffold absent"))?;
    let mut result = Vec::new();
    let mut bytes = 0usize;
    for arg in args.args() {
        if result.len() >= 10_000 {
            return Err(fault("runtime argument count limit"));
        }
        let span = map.span_at(arg.syntax().text_range().start());
        let capture = store.export(&syntax_bridge::syntax_node_to_token_tree(
            arg.syntax(),
            &map,
            span,
            syntax_bridge::DocCommentDesugarMode::Mbe,
        ))?;
        let capture = match capture.roots() {
            [
                TokenTree::Group {
                    delimiter: Delimiter::Invisible,
                    ..
                },
            ] => capture,
            _ => {
                let endpoint = |token: &TokenTree| match token {
                    TokenTree::Group { open, close, .. } => (*open, *close),
                    TokenTree::Ident { span, .. }
                    | TokenTree::Literal { span, .. }
                    | TokenTree::Punct { span, .. } => (*span, *span),
                };
                let open = endpoint(
                    capture
                        .roots()
                        .first()
                        .ok_or_else(|| fault("runtime argument opening token absent"))?,
                )
                .0;
                let close = endpoint(
                    capture
                        .roots()
                        .last()
                        .ok_or_else(|| fault("runtime argument closing token absent"))?,
                )
                .1;
                TokenForest::new(
                    vec![TokenTree::Group {
                        delimiter: Delimiter::Invisible,
                        open,
                        close,
                        children: capture.roots().to_vec(),
                    }],
                    capture.contexts().to_vec(),
                    capture.spans().to_vec(),
                )?
            }
        };
        bytes = bytes
            .checked_add(capture.allocated_bytes()?)
            .filter(|n| *n <= 128 * 1024 * 1024)
            .ok_or_else(|| fault("runtime argument capture metadata limit"))?;
        result
            .try_reserve(1)
            .map_err(|_| fault("runtime argument allocation"))?;
        result.push(capture);
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn captured_arguments_preserve_blocks_generic_commas_and_closure_commas() {
        use crate::token_wire::{ContextRecord, Edition, Key, SourceIdentity, Transparency};
        let input = crate::token_wire::parse_source(
            "({impl S{}true}, call::<A,B>(), (|x,y|x+y)(1,2), value=7)",
            SourceIdentity {
                unit: Key([1; 32]),
                content: Key([2; 32]),
                path: "case.rs".into(),
            },
            ContextRecord {
                parent: None,
                expansion: None,
                transparency: Transparency::Opaque,
                defining_unit: Key([1; 32]),
                edition: Edition::E2024,
            },
            Key([3; 32]),
        )
        .unwrap();
        let selected = input;
        let args = expressions(&selected).unwrap();
        assert_eq!(args.len(), 4);
        for argument in args {
            assert!(
                argument
                    .spans()
                    .iter()
                    .all(|span| span.source.content == Key([2; 32]))
            );
            crate::declaration_output::materialize(
                &argument,
                crate::declaration_output::DeclarationGrammar::Expression,
            )
            .unwrap();
        }
    }
}
