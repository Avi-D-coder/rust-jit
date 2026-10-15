//! Reached ordinary scalar let conditions; no namespace, query or runtime effects.
use super::*;
use crate::ir::pc::SlotId;
use ra_ap_syntax::ast::{RangeItem, RangeOp};

fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "scalar condition source budget".into(),
    )
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Bool,
    Char,
    Integer,
    Floating,
}
fn family(pattern: &ast::LiteralPat) -> Option<Family> {
    let literal = pattern.literal()?;
    Some(match literal.token().kind() {
        SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW if pattern.minus_token().is_none() => {
            Family::Bool
        }
        SyntaxKind::CHAR if pattern.minus_token().is_none() => Family::Char,
        SyntaxKind::INT_NUMBER
            if ast::IntNumber::cast(literal.token())?
                .suffix()
                .is_some_and(|s| matches!(s, "f32" | "f64")) =>
        {
            Family::Floating
        }
        SyntaxKind::INT_NUMBER => Family::Integer,
        SyntaxKind::FLOAT_NUMBER => Family::Floating,
        _ => return None,
    })
}
pub(super) fn selected(pattern: &ast::Pat) -> bool {
    matches!(pattern, ast::Pat::RangePat(_))
        || matches!(pattern, ast::Pat::LiteralPat(p)
        if matches!(family(p), Some(Family::Char | Family::Integer | Family::Floating)))
}
fn span(
    body: &BodyHandle,
    key: &PcKey,
    syntax: &SyntaxNode,
) -> Result<crate::syntax::spans::BodySpan, Diagnostic> {
    if key.body != body.key() {
        return Err(error("condition literal owning body differs"));
    }
    let r = syntax.text_range();
    let start = u32::from(r.start());
    let end = u32::from(r.end());
    if start >= end || end > body.range.end || end - start > 65_536 {
        return Err(limit());
    }
    Ok(crate::syntax::spans::BodySpan {
        body_key: key.body.clone(),
        relative_start: start.checked_sub(body.range.start).ok_or_else(limit)?,
        relative_end: end.checked_sub(body.range.start).ok_or_else(limit)?,
    })
}
fn endpoint(pattern: Option<&ast::Pat>) -> Result<&ast::LiteralPat, Diagnostic> {
    let Some(ast::Pat::LiteralPat(pattern)) = pattern else {
        return Err(unsupported(
            "reached condition bound requires an original scalar literal",
        ));
    };
    if pattern
        .syntax()
        .children()
        .any(|n| ast::Attr::can_cast(n.kind()))
        || family(pattern).is_none()
    {
        return Err(unsupported(
            "reached condition bound requires an unattributed scalar literal",
        ));
    }
    Ok(pattern)
}
fn literal(
    pattern: &ast::LiteralPat,
    body: &BodyHandle,
    key: &PcKey,
) -> Result<LiteralRecipe, Diagnostic> {
    let literal = pattern
        .literal()
        .ok_or_else(|| error("condition literal absent"))?;
    let source = span(body, key, pattern.syntax())?;
    if pattern.minus_token().is_some() {
        match family(pattern) {
            Some(Family::Integer) => {
                let number = ast::IntNumber::cast(literal.token())
                    .ok_or_else(|| error("condition integer token absent"))?;
                return Ok(LiteralRecipe {
                    kind: LiteralKind::NegativeInteger,
                    spelling: pattern.syntax().text().to_string(),
                    decoded: Some(crate::syntax::literal_decode::integer(&number, true)),
                    source,
                });
            }
            Some(Family::Floating) => {
                let mut recipe = literal_recipe(&literal, source)?;
                let Some(DecodedLiteral::Float { decimal, .. }) = &mut recipe.decoded else {
                    return Err(error("condition floating decode differs"));
                };
                let capacity = decimal.len().checked_add(1).ok_or_else(limit)?;
                let mut signed = String::new();
                signed.try_reserve_exact(capacity).map_err(|_| limit())?;
                signed.push('-');
                signed.push_str(decimal);
                *decimal = signed;
                recipe.spelling = pattern.syntax().text().to_string();
                return Ok(recipe);
            }
            _ => {
                return Err(unsupported(
                    "condition signed pattern requires numeric source",
                ));
            }
        }
    }
    literal_recipe(&literal, source)
}
fn written(pattern: &ast::LiteralPat) -> Option<PrimitiveType> {
    let token = pattern.literal()?.token();
    if token.kind() == SyntaxKind::CHAR {
        return Some(PrimitiveType::Char);
    }
    let hint = match token.kind() {
        SyntaxKind::INT_NUMBER => ast::IntNumber::cast(token)?.suffix().and_then(hint_type),
        SyntaxKind::FLOAT_NUMBER => ast::FloatNumber::cast(token)?.suffix().and_then(hint_type),
        _ => return None,
    };
    match hint {
        Some(HintRecipe::Primitive(p)) => Some(p),
        _ => None,
    }
}
fn hint_type(raw: &str) -> Option<HintRecipe> {
    // This selects only the actual suffix token, never a Type-path spelling.
    Some(HintRecipe::Primitive(match raw {
        "i8" => PrimitiveType::I8,
        "i16" => PrimitiveType::I16,
        "i32" => PrimitiveType::I32,
        "i64" => PrimitiveType::I64,
        "isize" => PrimitiveType::Isize,
        "u8" => PrimitiveType::U8,
        "u16" => PrimitiveType::U16,
        "u32" => PrimitiveType::U32,
        "u64" => PrimitiveType::U64,
        "usize" => PrimitiveType::Usize,
        "f32" => PrimitiveType::F32,
        "f64" => PrimitiveType::F64,
        _ => return None,
    }))
}
fn pattern_context(pattern: &ast::Pat) -> Option<PrimitiveType> {
    match pattern {
        ast::Pat::LiteralPat(p) => written(p),
        ast::Pat::RangePat(p) => {
            let lower = p.start();
            let upper = p.end();
            let first = match lower.as_ref().or(upper.as_ref())? {
                ast::Pat::LiteralPat(p) => p,
                _ => return None,
            };
            if let Some(p) = written(first) {
                return Some(p);
            }
            let Some(ast::Pat::LiteralPat(upper)) = upper.as_ref() else {
                return None;
            };
            if family(first) == family(upper) {
                written(upper)
            } else {
                None
            }
        }
        _ => None,
    }
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
                let range = index.nodes()[binding.0 as usize].range;
                let root = parsed.entered_body_root(body)?;
                for (work, node) in root.descendants().enumerate() {
                    if work >= 100_000 {
                        return Err(limit());
                    }
                    let Some(statement) = ast::LetStmt::cast(node) else {
                        continue;
                    };
                    let Some(pattern) = statement.pat() else {
                        continue;
                    };
                    let r = pattern.syntax().text_range();
                    if u32::from(r.start()).checked_sub(body.range.start) != Some(range.start)
                        || u32::from(r.end()).checked_sub(body.range.start) != Some(range.end)
                    {
                        continue;
                    }
                    if statement.ty().is_some() {
                        return primitive_cast_target::resolve(
                            parsed,
                            index,
                            statement.ty(),
                            None,
                        )?
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
                                    span(
                                        body,
                                        &index.pc(NodeId(0), Phase::Entry)?,
                                        literal.syntax(),
                                    )?,
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
                                return source_hint(
                                    parsed,
                                    index,
                                    body,
                                    &initializer,
                                    None,
                                    depth + 1,
                                );
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
                return Err(unsupported(
                    "condition lexical binding source requires integration",
                ));
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
    literal: LiteralRecipe,
    operator: NumericBinary,
    source: SlotId,
    expected: SlotId,
    destination: SlotId,
) -> Vec<Op> {
    vec![
        Op::NumericHint {
            parent: destination,
            child: expected,
            left: Some(source),
            operator: Some(operator),
        },
        Op::Literal {
            literal,
            hint: HintRecipe::None,
            destination: expected,
        },
        Op::NumericBinary {
            operator,
            left: source,
            right: expected,
            destination,
        },
        Op::Discard { source: expected },
    ]
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
    if parsed.generated_origin.is_some() {
        return refuse("generated scalar condition requires its source producer");
    }
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
    let range = if let ast::Pat::RangePat(p) = &pattern {
        Some(p)
    } else {
        None
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
                        pattern_context(&pattern),
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
            let (literal, operator, upper_pending) = if let Some(range) = range {
                let lower = range.start();
                let upper = range.end();
                if lower.is_none() && upper.is_none() {
                    return Err(unsupported("condition range requires a literal endpoint"));
                }
                let bound = endpoint(lower.as_ref().or(upper.as_ref()))?;
                if family(bound) == Some(Family::Bool) {
                    return Err(unsupported(
                        "condition ranges require numeric or char literals",
                    ));
                }
                let op = if lower.is_some() {
                    NumericBinary::Ge
                } else {
                    match range.op_kind() {
                        Some(RangeOp::Inclusive) => NumericBinary::Le,
                        Some(RangeOp::Exclusive) => NumericBinary::Lt,
                        None => return Err(unsupported("condition range operator absent")),
                    }
                };
                (
                    literal(bound, body, key)?,
                    op,
                    lower.is_some() && upper.is_some(),
                )
            } else {
                (
                    literal(endpoint(Some(&pattern))?, body, key)?,
                    NumericBinary::Eq,
                    false,
                )
            };
            let operations = compare(literal, operator, source, expected, destination);
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
        Phase::Commit if range.is_some() => {
            let range = range.ok_or_else(|| error("condition range disappeared"))?;
            let lower = range.start();
            let upper = range.end();
            let lo = endpoint(lower.as_ref())?;
            let hi = endpoint(upper.as_ref())?;
            if family(lo) != family(hi) {
                return Err(unsupported(
                    "condition range endpoints require the same literal family",
                ));
            }
            let operator = match range.op_kind() {
                Some(RangeOp::Inclusive) => NumericBinary::Le,
                Some(RangeOp::Exclusive) => NumericBinary::Lt,
                None => return Err(unsupported("condition range operator absent")),
            };
            let mut operations = vec![Op::Discard {
                source: destination,
            }];
            operations.extend(compare(
                literal(hi, body, key)?,
                operator,
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
