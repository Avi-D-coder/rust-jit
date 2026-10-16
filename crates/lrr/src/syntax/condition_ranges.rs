//! Reached ordinary scalar let conditions; no namespace, query or runtime effects.
use super::pattern_tests::{
    self as patterns, ScalarComparison, ScalarTestPlan, source_span as span,
};
use super::*;
use crate::ir::pc::SlotId;

fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "scalar condition source budget".into(),
    )
}
pub(super) fn selected(pattern: &ast::Pat) -> bool {
    matches!(pattern, ast::Pat::LiteralPat(_) | ast::Pat::RangePat(_))
}
fn source_hint(
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
            &p.expr().ok_or_else(|| error("condition group absent"))?,
            expected,
            depth + 1,
        ),
        ast::Expr::PrefixExpr(p) if p.op_token().is_some_and(|t| matches!(t.text(), "-" | "!")) => {
            source_hint(
                parsed,
                index,
                body,
                &p.expr().ok_or_else(|| error("condition prefix absent"))?,
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
                        "condition scrutinee literal family requires source integration",
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
                        unsupported("condition lexical binding source requires integration")
                    })?;
                if statement.ty().is_some() {
                    return primitive_cast_target::resolve(parsed, index, statement.ty(), None)?
                        .map(HintRecipe::Primitive)
                        .ok_or_else(|| {
                            unsupported(
                                "condition local requires admitted primitive source annotation",
                            )
                        });
                }
                let Some(initializer) = statement.initializer() else {
                    return Err(unsupported("condition local initializer source absent"));
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
                                .ok_or_else(|| error("condition local group absent"))?
                        }
                        ast::Expr::PrefixExpr(prefix)
                            if prefix
                                .op_token()
                                .is_some_and(|t| matches!(t.text(), "-" | "!")) =>
                        {
                            initializer = prefix
                                .expr()
                                .ok_or_else(|| error("condition local prefix absent"))?
                        }
                        _ => {
                            return Err(unsupported(
                                "condition local requires closed primitive initializer source",
                            ));
                        }
                    }
                }
                return Err(limit());
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
fn compare(
    comparison: ScalarComparison,
    source: SlotId,
    expected: SlotId,
    destination: SlotId,
) -> Vec<Op> {
    // The reached source supplies its runtime numeric constraint. Pattern-test
    // semantics and decoding are shared with ordinary match; this is only slot
    // context forwarding for a condition's independently indexed source.
    let mut operations = vec![Op::NumericHint {
        parent: destination,
        child: expected,
        left: Some(source),
        operator: Some(comparison.operator),
    }];
    operations.extend(comparison.operations(source, expected, destination, &HintRecipe::None));
    operations
}
pub(super) fn prepare(
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    key: &PcKey,
    expression: &ast::LetExpr,
) -> Result<VerifiedRecipe, Diagnostic> {
    let node = &index.nodes()[key.node as usize];
    let id = node.id;
    let fallback = index.pc(id, Phase::Exit)?;
    let refuse =
        |message: &str| thunks::unsupported(index, key.clone(), message.into(), fallback.clone());
    if key.body != body.key() || index.body() != &body.key() || index.profile() != &parsed.profile {
        return Err(error("condition owning source/profile differs"));
    }
    let pattern = expression
        .pat()
        .ok_or_else(|| error("condition pattern absent"))?;
    if pattern
        .syntax()
        .children()
        .any(|n| ast::Attr::can_cast(n.kind()))
    {
        return refuse("condition pattern attributes require source admission");
    }
    let scrutinee = expression
        .expr()
        .ok_or_else(|| error("condition scrutinee absent"))?;
    let source_node = id_for(index, body, scrutinee.syntax())?;
    let source = index.nodes()[source_node.0 as usize]
        .value
        .ok_or_else(|| error("condition scrutinee value absent"))?;
    let expected =
        node.runtime_temporaries[0].ok_or_else(|| error("condition expected temporary absent"))?;
    let destination = node
        .value
        .ok_or_else(|| error("condition Bool result absent"))?;
    let next = || return_pc(index, body, expression.syntax());
    let range = matches!(pattern, ast::Pat::RangePat(_));
    let scalar_test = match ScalarTestPlan::from_source(&pattern) {
        Ok(test) => test,
        Err(error) if error.code == "LRR-E-UNSUPPORTED" => return refuse(&error.message),
        Err(error) => return Err(error),
    };
    let phase = Phase::from_code(key.phase);
    let result = (|| match phase {
        Phase::Entry => draft(
            index,
            key,
            vec![
                Op::ScopeEnter {
                    scope: node
                        .clause_scope
                        .ok_or_else(|| error("condition clause scope absent"))?,
                },
                Op::SetHint {
                    child: source,
                    hint: source_hint(
                        parsed,
                        index,
                        body,
                        &scrutinee,
                        patterns::source_type(&pattern),
                        0,
                    )?,
                },
            ],
            RecipeExit::Next(index.pc(source_node, Phase::Entry)?),
        ),
        Phase::Exit => draft(
            index,
            key,
            vec![Op::Discard { source }],
            RecipeExit::Next(next()?),
        ),
        Phase::Operand(n) if node.operands.get(usize::from(n)) == Some(&source_node) => {
            let comparison = scalar_test.first(body, key)?;
            let upper_pending = comparison.upper_pending;
            let operations = compare(comparison, source, expected, destination);
            draft(
                index,
                key,
                operations,
                if upper_pending {
                    RecipeExit::Branch {
                        condition: destination,
                        yes: index.pc(id, Phase::Commit)?,
                        no: index.pc(id, Phase::Exit)?,
                    }
                } else {
                    RecipeExit::Next(index.pc(id, Phase::Exit)?)
                },
            )
        }
        Phase::Commit if range => {
            let mut operations = vec![Op::Discard {
                source: destination,
            }];
            operations.extend(compare(
                scalar_test.upper(body, key)?,
                source,
                expected,
                destination,
            ));
            draft(
                index,
                key,
                operations,
                RecipeExit::Next(index.pc(id, Phase::Exit)?),
            )
        }
        _ => Err(unsupported("invalid scalar condition phase")),
    })();
    match result {
        Err(error) if error.code == "LRR-E-UNSUPPORTED" => refuse(&error.message),
        other => other,
    }
}
#[cfg(test)]
#[path = "condition_ranges_tests.rs"]
mod tests;
