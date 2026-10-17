//! Pinned AST macro operands with original owned argument tokens; no name resolution.
use crate::{
    AdapterError,
    span_store::SpanStore,
    token_wire::{Key, TokenForest, TokenTree},
};
use syntax::ast::HasGenericArgs;
use syntax::{AstNode, ast};
pub struct NestedStringInvocation {
    pub absolute: bool,
    pub segments: Vec<String>,
    pub arguments: TokenForest,
    pub first_span: Key,
    pub last_span: Key,
}
fn fault(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-COMPILE-INPUT",
        message: message.into(),
    }
}
pub fn parse(input: &TokenForest) -> Result<NestedStringInvocation, AdapterError> {
    if !matches!(
        crate::string_arguments::parse_single(input)?,
        crate::string_arguments::StringOperand::NeedExpansion { .. }
    ) {
        return Err(fault("string operand is not a macro invocation"));
    }
    let [
        TokenTree::Group {
            delimiter,
            open,
            close,
            children,
        },
    ] = input.roots()
    else {
        return Err(fault("nested macro input root is absent"));
    };
    let part = children
        .split(|t| matches!(t, TokenTree::Punct { scalar: ',', .. }))
        .next()
        .ok_or_else(|| fault("nested operand absent"))?;
    let operand = TokenForest::new(
        vec![TokenTree::Group {
            delimiter: *delimiter,
            open: *open,
            close: *close,
            children: part.to_vec(),
        }],
        input.contexts().to_vec(),
        input.spans().to_vec(),
    )
    .map_err(|e| fault(&e.to_string()))?;
    let mut store = SpanStore::default();
    let tree = store.import(&operand)?;
    let view = tree.token_trees();
    let first = view
        .first_span()
        .ok_or_else(|| fault("nested first span absent"))?;
    let last = view
        .last_span()
        .ok_or_else(|| fault("nested last span absent"))?;
    let mut builder = tt::TopSubtreeBuilder::new(tt::Delimiter {
        kind: tt::DelimiterKind::Invisible,
        open: first,
        close: last,
    });
    builder.extend_with_tt(view);
    let syntax_tokens = builder.build();
    let (parsed, _) = syntax_bridge::token_tree_to_syntax_node(
        &syntax_tokens,
        parser::TopEntryPoint::Expr,
        &mut |ctx| ctx.edition(store.database()),
    );
    if !parsed.errors().is_empty() {
        return Err(fault("invalid nested string macro syntax"));
    }
    let expression = ast::MacroExpr::cast(parsed.syntax_node())
        .ok_or_else(|| fault("string operand is not a macro invocation"))?;
    let call = expression
        .macro_call()
        .ok_or_else(|| fault("nested macro call is absent"))?;
    let mut path = call
        .path()
        .ok_or_else(|| fault("nested macro path is absent"))?;
    let mut segments = Vec::new();
    let absolute;
    loop {
        let segment = path
            .segment()
            .ok_or_else(|| fault("nested macro path segment is absent"))?;
        if segment.generic_arg_list().is_some()
            || segment.type_anchor().is_some()
            || segment.parenthesized_arg_list().is_some()
        {
            return Err(fault("nested macro path has type arguments"));
        }
        let name = segment
            .name_ref()
            .ok_or_else(|| fault("nested macro path name is absent"))?
            .text()
            .to_string();
        segments.push(name);
        match path.qualifier() {
            Some(parent) => path = parent,
            None => {
                absolute =
                    path.coloncolon_token().is_some() || segment.coloncolon_token().is_some();
                break;
            }
        }
    }
    segments.reverse();
    let [TokenTree::Group { children, .. }] = input.roots() else {
        return Err(fault("nested macro input root is absent"));
    };
    let mut parts = children.split(|t| matches!(t, TokenTree::Punct { scalar: ',', .. }));
    let part = parts.next().ok_or_else(|| fault("nested operand absent"))?;
    if parts.any(|p| !p.is_empty()) {
        return Err(fault("nested string requires one operand"));
    }
    let Some(arguments @ TokenTree::Group { .. }) = part.last() else {
        return Err(fault("nested macro argument group is absent"));
    };
    let arguments = TokenForest::new(
        vec![arguments.clone()],
        input.contexts().to_vec(),
        input.spans().to_vec(),
    )
    .map_err(|e| fault(&e.to_string()))?;
    let first = tree
        .token_trees()
        .first_span()
        .ok_or_else(|| fault("nested first span absent"))?;
    let last = tree
        .token_trees()
        .last_span()
        .ok_or_else(|| fault("nested last span absent"))?;
    Ok(NestedStringInvocation {
        absolute,
        segments,
        arguments,
        first_span: store.owned_span_key(first)?,
        last_span: store.owned_span_key(last)?,
    })
}
