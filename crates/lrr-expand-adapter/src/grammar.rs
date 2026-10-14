//! Shared pure pinned grammar validation for owned expansion outputs.
use crate::{AdapterError, span_store::SpanStore, token_wire::TokenForest};
fn fault(code: &'static str, message: String) -> AdapterError {
    AdapterError { code, message }
}
pub(crate) fn validate_complete_syntax(root: &syntax::SyntaxNode) -> Result<(), AdapterError> {
    // TopEntryPoint consumes trailing tokens into an ERROR tree without always
    // adding a Parse error entry. Such a tree cannot certify a complete fragment.
    if root.kind() == syntax::SyntaxKind::ERROR
        || root
            .descendants()
            .any(|node| node.kind() == syntax::SyntaxKind::ERROR)
    {
        return Err(fault(
            "LRR-E-MACRO-OUTPUT",
            "pinned fragment retains malformed or trailing tokens".into(),
        ));
    }
    Ok(())
}
pub fn validate(input: &TokenForest, grammar: Grammar) -> Result<(), AdapterError> {
    let mut store = SpanStore::default();
    let tree = store.import(input)?;
    validate_tree(&store, &tree, grammar)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grammar {
    Items,
    Expression,
    Pattern,
    Type,
    TraitItems,
    ImplItems,
    Statements,
}
impl Grammar {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::Items => 0,
            Self::Expression => 1,
            Self::Pattern => 2,
            Self::Type => 3,
            Self::TraitItems => 4,
            Self::ImplItems => 5,
            Self::Statements => 6,
        }
    }
}

pub(crate) fn validate_tree(
    store: &SpanStore,
    tree: &tt::TopSubtree,
    grammar: Grammar,
) -> Result<(), AdapterError> {
    let associated;
    let tree = if matches!(grammar, Grammar::TraitItems | Grammar::ImplItems) {
        // Preserve the returned token trees/spans verbatim inside their actual
        // associated-item context. The wrapper is parser scaffolding only.
        let span = tree.top_subtree().delimiter.open;
        let mut builder = tt::TopSubtreeBuilder::new(tt::Delimiter {
            kind: tt::DelimiterKind::Invisible,
            open: span,
            close: span,
        });
        builder.push(
            tt::Ident::new(
                if grammar == Grammar::TraitItems {
                    "trait"
                } else {
                    "impl"
                },
                span,
            )
            .into(),
        );
        builder.push(tt::Ident::new("__LrrGrammarContext", span).into());
        builder.open(tt::DelimiterKind::Brace, span);
        builder.extend_with_tt(tree.view().strip_invisible());
        builder.close(span);
        associated = builder.build();
        &associated
    } else {
        tree
    };
    let entry = match grammar {
        Grammar::Items => parser::TopEntryPoint::MacroItems,
        Grammar::Expression => parser::TopEntryPoint::Expr,
        Grammar::Pattern => parser::TopEntryPoint::Pattern,
        Grammar::Type => parser::TopEntryPoint::Type,
        Grammar::Statements => parser::TopEntryPoint::MacroStmts,
        Grammar::TraitItems | Grammar::ImplItems => parser::TopEntryPoint::SourceFile,
    };
    let (parse, _) = syntax_bridge::token_tree_to_syntax_node(tree, entry, &mut |context| {
        context.edition(store.database())
    });
    if let Some(error) = parse.errors().first() {
        return Err(fault(
            "LRR-E-MACRO-OUTPUT",
            format!("invalid {grammar:?} expansion: {error}"),
        ));
    }
    validate_complete_syntax(&parse.syntax_node())
}
