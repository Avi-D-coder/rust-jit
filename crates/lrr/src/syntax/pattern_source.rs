//! Cold scalar pattern source constraints shared by ordinary match and conditions.
//! A hint constrains an actual source; it never grants a binding or runtime value.
use super::super::{error, literal_recipe, local_binding, parameter_at, primitive_cast_target};
use super::source_span as span;
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    ir::{
        op::{DecodedLiteral, HintRecipe, LiteralKind, PrimitiveType},
        pc::{NodeId, Phase},
    },
    syntax::{body_index::BodyIndex, outline::BodyHandle, parse::ParsedFile},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs},
};
fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "scalar pattern source constraint budget".into(),
    )
}
pub(in crate::syntax::thunks) fn binding_source_hint(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    statement: &ast::LetStmt,
    depth: usize,
) -> Result<HintRecipe, Diagnostic> {
    if depth >= 128 {
        return Err(limit());
    }
    if statement.ty().is_some() {
        return primitive_cast_target::resolve(parsed, index, statement.ty(), None)?
            .map(HintRecipe::Primitive)
            .ok_or_else(|| {
                unsupported("pattern local requires admitted primitive source annotation")
            });
    }
    let Some(initializer) = statement.initializer() else {
        return Err(unsupported("pattern local initializer source absent"));
    };
    let mut initializer = initializer;
    for _ in 0..128 {
        match initializer {
            ast::Expr::Literal(ref literal) => {
                let recipe = literal_recipe(
                    literal,
                    span(body, &index.pc(NodeId(0), Phase::Entry)?, literal.syntax())?,
                )?;
                if matches!(
                    recipe.decoded,
                    Some(
                        DecodedLiteral::Integer { suffix: None, .. }
                            | DecodedLiteral::Float { suffix: None, .. }
                    )
                ) {
                    // The reached binding may have a later concrete
                    // numeric constraint; do not re-default it here.
                    return Ok(HintRecipe::None);
                }
                return source_hint(parsed, index, body, &initializer, None, depth + 1);
            }
            ast::Expr::ParenExpr(group) => {
                initializer = group
                    .expr()
                    .ok_or_else(|| error("pattern local group absent"))?
            }
            ast::Expr::PrefixExpr(prefix)
                if prefix
                    .op_token()
                    .is_some_and(|t| matches!(t.text(), "-" | "!")) =>
            {
                initializer = prefix
                    .expr()
                    .ok_or_else(|| error("pattern local prefix absent"))?
            }
            _ => {
                return Err(unsupported(
                    "pattern local requires closed primitive initializer source",
                ));
            }
        }
    }
    Err(limit())
}

pub(in crate::syntax::thunks) fn source_hint(
    parsed: &ParsedFile,
    index: &BodyIndex,
    body: &BodyHandle,
    expr: &ast::Expr,
    expected: Option<PrimitiveType>,
    depth: usize,
) -> Result<HintRecipe, Diagnostic> {
    if depth >= 128 {
        return Err(limit());
    }
    match expr {
        ast::Expr::ParenExpr(p) => source_hint(
            parsed,
            index,
            body,
            &p.expr().ok_or_else(|| error("pattern group absent"))?,
            expected,
            depth + 1,
        ),
        ast::Expr::PrefixExpr(p) if p.op_token().is_some_and(|t| matches!(t.text(), "-" | "!")) => {
            source_hint(
                parsed,
                index,
                body,
                &p.expr().ok_or_else(|| error("pattern prefix absent"))?,
                expected,
                depth + 1,
            )
        }
        ast::Expr::Literal(p) => {
            let recipe = literal_recipe(
                p,
                span(body, &index.pc(NodeId(0), Phase::Entry)?, p.syntax())?,
            )?;
            let primitive = match recipe.decoded {
                Some(DecodedLiteral::Integer {
                    suffix: Some(p), ..
                })
                | Some(DecodedLiteral::Float {
                    suffix: Some(p), ..
                }) => p,
                Some(DecodedLiteral::Integer { suffix: None, .. }) => expected
                    .filter(|p| {
                        matches!(
                            p,
                            PrimitiveType::I8
                                | PrimitiveType::I16
                                | PrimitiveType::I32
                                | PrimitiveType::I64
                                | PrimitiveType::Isize
                                | PrimitiveType::U8
                                | PrimitiveType::U16
                                | PrimitiveType::U32
                                | PrimitiveType::U64
                                | PrimitiveType::Usize
                        )
                    })
                    .unwrap_or(PrimitiveType::I32),
                Some(DecodedLiteral::Float { suffix: None, .. }) => expected
                    .filter(|p| matches!(p, PrimitiveType::F32 | PrimitiveType::F64))
                    .unwrap_or(PrimitiveType::F64),
                Some(DecodedLiteral::Char(_)) => PrimitiveType::Char,
                Some(DecodedLiteral::Byte(_)) => PrimitiveType::U8,
                _ if recipe.kind == LiteralKind::Bool => PrimitiveType::Bool,
                _ => {
                    return Err(unsupported(
                        "pattern scrutinee literal family requires source integration",
                    ));
                }
            };
            Ok(HintRecipe::Primitive(primitive))
        }
        ast::Expr::PathExpr(path) => {
            if let Some(binding) = local_binding(parsed, index, body, path)? {
                let statement = parsed
                    .binding_let_statement(body, index, binding)?
                    .ok_or_else(|| {
                        unsupported("pattern lexical binding source requires integration")
                    })?;
                return binding_source_hint(parsed, index, body, &statement, depth);
            }
            let Some(p) = path.path().filter(|p| p.qualifier().is_none()) else {
                return Ok(HintRecipe::None);
            };
            let Some(segment) = p.segment() else {
                return Ok(HintRecipe::None);
            };
            if segment.coloncolon_token().is_some()
                || segment.generic_arg_list().is_some()
                || segment.type_anchor().is_some()
                || segment.parenthesized_arg_list().is_some()
            {
                return Ok(HintRecipe::None);
            }
            let Some(name) = segment.name_ref() else {
                return Ok(HintRecipe::None);
            };
            let name = MacroName::parse(name.text(), parsed.edition)?;
            let Some(ordinal) = parameter_at(parsed, path.syntax(), &name)? else {
                return Ok(HintRecipe::None);
            };
            // A nested const/static/closure source body cannot capture the
            // parameter list of an enclosing function. Bind the actual AST
            // function body to this selected source owner before the ordinal.
            let owns_parameters = path
                .syntax()
                .ancestors()
                .find_map(ast::Fn::cast)
                .and_then(|function| function.body())
                .is_some_and(|block| {
                    let range = block.syntax().text_range();
                    u32::from(range.start()) == body.range.start
                        && u32::from(range.end()) == body.range.end
                });
            if !owns_parameters {
                return Err(unsupported(
                    "condition parameter belongs to another source body",
                ));
            }
            Ok(HintRecipe::DefinitionParameter(ordinal))
        }
        // A reached call/macro keeps ordinary eager execution and its own
        // source admission. Unknown syntax cannot manufacture a source view.
        _ => Ok(HintRecipe::None),
    }
}
