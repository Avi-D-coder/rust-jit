//! Pure scalar operand lowering after the frontend resolves a compiler intrinsic.
//! Uses the pinned parser, lexical source identities and existing LRIR operations.
//! An explicit work stack preserves written order without recursive guest execution.
use super::{
    body_index::{BodyIndex, ExpressionKind, NodeRole},
    outline::BodyHandle,
    parse::ParsedFile,
    runtime_macro_arguments::RuntimeMacroArguments,
    spans::BodySpan,
    thunks::{binary_operator, hint, literal_recipe},
};
use crate::{
    core::diagnostic::Diagnostic,
    facade::macros::RuntimeMacroKind,
    ir::{
        op::{
            HintRecipe, LiteralKind, LiteralRecipe, NumericBinary, NumericUnary, Op, PrimitiveType,
        },
        pc::{PcKey, SlotId},
    },
};
use ra_ap_syntax::{
    AstNode, AstToken, Edition, SyntaxKind, TextRange, TextSize,
    ast::{self, HasGenericArgs},
};

fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-RUNTIME-MACRO-ARGUMENTS", message.into())
}
fn unsupported() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-UNSUPPORTED",
        "reached assertion operand requires pending non-scalar expression integration".into(),
    )
}
enum Work {
    Expression(ast::Expr, SlotId, u32),
    Operation(Op),
}
impl ParsedFile {
    pub(crate) fn runtime_macro_scalar_operations(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        arguments: &RuntimeMacroArguments,
    ) -> Result<Vec<Op>, Diagnostic> {
        arguments.require_source(&self.key())?;
        if body.key() != key.body || *index.body() != key.body {
            return Err(invalid("scalar assertion body differs"));
        }
        let node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| invalid("scalar assertion node absent"))?;
        if node.role != NodeRole::Expression(ExpressionKind::Macro)
            || node.range.start.checked_add(body.range.start) != Some(arguments.invocation().start)
            || node.range.end.checked_add(body.range.start) != Some(arguments.invocation().end)
        {
            return Err(invalid("scalar assertion invocation differs"));
        }
        let condition = node.runtime_temporaries[0]
            .ok_or_else(|| invalid("scalar assertion condition absent"))?;
        let edition = match self.edition {
            2015 => Edition::Edition2015,
            2018 => Edition::Edition2018,
            2021 => Edition::Edition2021,
            2024 => Edition::Edition2024,
            _ => return Err(invalid("scalar assertion edition unsupported")),
        };
        let required = match arguments.kind() {
            RuntimeMacroKind::Assert => 1,
            RuntimeMacroKind::AssertEq | RuntimeMacroKind::AssertNe => 2,
            _ => return Err(invalid("scalar assertion requires an assertion intrinsic")),
        };
        if arguments.operands().len() != required {
            return Err(invalid("scalar assertion operand count differs"));
        }
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(required)
            .map_err(|_| invalid("scalar assertion root allocation"))?;
        let tree = self.parsed.tree();
        for operand in arguments.operands() {
            let original = tree.syntax().text();
            if operand.start > operand.end || operand.end > u32::from(original.len()) {
                return Err(invalid("scalar assertion operand outside source"));
            }
            let range = TextRange::new(TextSize::from(operand.start), TextSize::from(operand.end));
            let text = original.slice(range).to_string();
            let prefix = "fn __lrr_operand(){let __lrr_value=";
            let wrapped = format!("{prefix}{text};}}");
            let parsed = ast::SourceFile::parse(&wrapped, edition);
            if !parsed.errors().is_empty() {
                return Err(invalid("scalar assertion operand grammar differs"));
            }
            let expression = parsed
                .tree()
                .syntax()
                .descendants()
                .find_map(ast::LetStmt::cast)
                .and_then(|statement| statement.initializer())
                .ok_or_else(|| invalid("scalar assertion parsed operand absent"))?;
            let base = operand
                .start
                .checked_sub(body.range.start)
                .ok_or_else(|| invalid("scalar assertion operand precedes body"))?;
            // Store an offset into original body bytes, not a synthetic parser ID.
            roots.push((expression, base));
        }
        let mut available = node.runtime_scalar_slots.iter().copied();
        let mut allocate = || {
            available
                .next()
                .ok_or_else(|| invalid("scalar assertion slot reservation exhausted"))
        };
        let mut stack = Vec::new();
        let mut operations = Vec::new();
        stack
            .try_reserve_exact(usize::from(node.runtime_scalar_phases))
            .map_err(|_| invalid("scalar assertion work allocation"))?;
        operations
            .try_reserve_exact(usize::from(node.runtime_scalar_phases))
            .map_err(|_| invalid("scalar assertion operation allocation"))?;
        if required == 1 {
            let (expression, base) = roots
                .pop()
                .ok_or_else(|| invalid("assertion predicate absent"))?;
            stack.push(Work::Expression(expression, condition, base));
        } else {
            let right = allocate()?;
            let left = allocate()?;
            let operator = if arguments.kind() == RuntimeMacroKind::AssertEq {
                NumericBinary::Eq
            } else {
                NumericBinary::Ne
            };
            let default = super::numeric_fallback::pair(&roots[0].0, &roots[1].0)?;
            let (right_expr, right_base) = roots
                .pop()
                .ok_or_else(|| invalid("assertion right operand absent"))?;
            let (left_expr, left_base) = roots
                .pop()
                .ok_or_else(|| invalid("assertion left operand absent"))?;
            stack.push(Work::Operation(Op::NumericBinary {
                operator,
                left,
                right,
                destination: condition,
            }));
            stack.push(Work::Expression(right_expr, right, right_base));
            stack.push(Work::Operation(super::numeric_fallback::operand_hint(
                default,
                condition,
                right,
                Some(left),
                operator,
            )));
            stack.push(Work::Expression(left_expr, left, left_base));
            stack.push(Work::Operation(super::numeric_fallback::operand_hint(
                default, condition, left, None, operator,
            )));
        }
        operations.push(Op::SetHint {
            child: condition,
            hint: HintRecipe::Primitive(PrimitiveType::Bool),
        });
        let prefix_len = u32::try_from("fn __lrr_operand(){let __lrr_value=".len())
            .map_err(|_| invalid("operand prefix overflow"))?;
        while let Some(work) = stack.pop() {
            if operations.len() >= usize::from(node.runtime_scalar_phases)
                || stack.len() > usize::from(node.runtime_scalar_phases)
            {
                return Err(invalid("scalar assertion phase reservation exhausted"));
            }
            let Work::Expression(expression, destination, base) = work else {
                if let Work::Operation(operation) = work {
                    operations.push(operation);
                }
                continue;
            };
            if expression
                .syntax()
                .children()
                .any(|child| ast::Attr::can_cast(child.kind()))
            {
                return Err(unsupported());
            }
            let original_span = |expression: &ast::Expr| -> Result<BodySpan, Diagnostic> {
                let range = expression.syntax().text_range();
                let map = |offset: TextSize| {
                    u32::from(offset)
                        .checked_sub(prefix_len)
                        .and_then(|offset| base.checked_add(offset))
                        .ok_or_else(|| invalid("scalar operand origin overflow"))
                };
                Ok(BodySpan {
                    body_key: key.body.clone(),
                    relative_start: map(range.start())?,
                    relative_end: map(range.end())?,
                })
            };
            match expression {
                ast::Expr::TupleExpr(tuple) if tuple.fields().next().is_none() => {
                    // Unit is a closed primitive operand; use the existing
                    // checked Unit operation and ordinary comparison semantics.
                    stack.push(Work::Operation(Op::Unit { destination }));
                }
                ast::Expr::Literal(literal) => {
                    let span = original_span(&ast::Expr::Literal(literal.clone()))?;
                    stack.push(Work::Operation(Op::Literal {
                        literal: literal_recipe(&literal, span)?,
                        hint: HintRecipe::None,
                        destination,
                    }));
                }
                ast::Expr::PathExpr(path) => {
                    let path = path.path().ok_or_else(unsupported)?;
                    let segment = path.segment().ok_or_else(unsupported)?;
                    if path.qualifier().is_some()
                        || segment.generic_arg_list().is_some()
                        || segment.coloncolon_token().is_some()
                    {
                        return Err(unsupported());
                    }
                    let name = segment.name_ref().ok_or_else(unsupported)?;
                    stack.push(Work::Operation(self.runtime_macro_binding_read(
                        body,
                        index,
                        key,
                        name.text(),
                        destination,
                    )?));
                }
                ast::Expr::ParenExpr(group) => {
                    stack.push(Work::Expression(
                        group.expr().ok_or_else(unsupported)?,
                        destination,
                        base,
                    ));
                }
                ast::Expr::BinExpr(binary) => {
                    let operator = binary
                        .op_token()
                        .and_then(|token| binary_operator(token.text()))
                        .ok_or_else(unsupported)?;
                    let default = super::numeric_fallback::comparison(&binary)?;
                    let left_expr = binary.lhs().ok_or_else(unsupported)?;
                    let right_expr = binary.rhs().ok_or_else(unsupported)?;
                    let right_default = super::numeric_fallback::right_operand_default(
                        &right_expr,
                        operator,
                        default,
                    )?;
                    let left = allocate()?;
                    let right = allocate()?;
                    stack.push(Work::Operation(Op::NumericBinary {
                        operator,
                        left,
                        right,
                        destination,
                    }));
                    stack.push(Work::Expression(right_expr, right, base));
                    stack.push(Work::Operation(super::numeric_fallback::operand_hint(
                        right_default,
                        destination,
                        right,
                        Some(left),
                        operator,
                    )));
                    stack.push(Work::Expression(left_expr, left, base));
                    stack.push(Work::Operation(super::numeric_fallback::operand_hint(
                        default,
                        destination,
                        left,
                        None,
                        operator,
                    )));
                }
                ast::Expr::PrefixExpr(prefix) => {
                    if prefix.op_token().is_some_and(|token| token.text() == "*") {
                        let mut referent = prefix.expr().ok_or_else(unsupported)?;
                        while let ast::Expr::ParenExpr(group) = referent {
                            if group
                                .syntax()
                                .children()
                                .any(|child| ast::Attr::can_cast(child.kind()))
                            {
                                return Err(unsupported());
                            }
                            referent = group.expr().ok_or_else(unsupported)?;
                        }
                        let ast::Expr::PathExpr(path) = referent else {
                            return Err(unsupported());
                        };
                        let path = path.path().ok_or_else(unsupported)?;
                        let segment = path.segment().ok_or_else(unsupported)?;
                        if path.qualifier().is_some()
                            || segment.generic_arg_list().is_some()
                            || segment.coloncolon_token().is_some()
                        {
                            return Err(unsupported());
                        }
                        let name = segment.name_ref().ok_or_else(unsupported)?;
                        let reference = allocate()?;
                        let reference = SlotId(
                            reference
                                .0
                                .checked_add(1)
                                .ok_or_else(|| invalid("assertion reference place overflow"))?,
                        );
                        let place = SlotId(
                            destination
                                .0
                                .checked_add(1)
                                .ok_or_else(|| invalid("assertion dereference place overflow"))?,
                        );
                        // A checked address read preserves the local reference
                        // owner, including mutable leases. No reference value is
                        // copied or moved just to observe the primitive target.
                        stack.push(Work::Operation(Op::ReadPlace { place, destination }));
                        stack.push(Work::Operation(Op::DerefPlace {
                            reference,
                            destination: place,
                        }));
                        stack.push(Work::Operation(self.runtime_macro_binding_place(
                            body,
                            index,
                            key,
                            name.text(),
                            reference,
                        )?));
                        continue;
                    }
                    let operator = match prefix
                        .op_token()
                        .map(|token| token.text().to_string())
                        .as_deref()
                    {
                        Some("-") => NumericUnary::Neg,
                        Some("!") => NumericUnary::Not,
                        _ => return Err(unsupported()),
                    };
                    let child_expr = prefix.expr().ok_or_else(unsupported)?;
                    let mut selected = child_expr.clone();
                    while let ast::Expr::ParenExpr(group) = selected {
                        selected = group.expr().ok_or_else(unsupported)?;
                    }
                    if operator == NumericUnary::Neg
                        && let ast::Expr::Literal(literal) = selected
                        && literal.token().kind() == SyntaxKind::INT_NUMBER
                        && let Some(number) = ast::IntNumber::cast(literal.token())
                        && matches!(
                            number.suffix(),
                            None | Some("i8" | "i16" | "i32" | "i64" | "isize" | "i128")
                        )
                    {
                        let spelling = prefix.syntax().text().to_string();
                        let span = original_span(&ast::Expr::PrefixExpr(prefix))?;
                        stack.push(Work::Operation(Op::Literal {
                            literal: LiteralRecipe {
                                decoded: Some(super::literal_decode::integer(&number, true)),
                                kind: LiteralKind::NegativeInteger,
                                spelling,
                                source: span,
                            },
                            hint: HintRecipe::None,
                            destination,
                        }));
                    } else {
                        let source = allocate()?;
                        stack.push(Work::Operation(Op::NumericUnary {
                            operator,
                            source,
                            destination,
                        }));
                        stack.push(Work::Expression(child_expr, source, base));
                        stack.push(Work::Operation(Op::NumericHint {
                            parent: destination,
                            child: source,
                            left: None,
                            operator: None,
                        }));
                    }
                }
                ast::Expr::CastExpr(cast) => {
                    let Some(HintRecipe::Primitive(target)) = hint(cast.ty()) else {
                        return Err(unsupported());
                    };
                    let child = cast.expr().ok_or_else(unsupported)?;
                    let default = super::numeric_fallback::literal_only(&child)?;
                    let source = allocate()?;
                    stack.push(Work::Operation(Op::PrimitiveCast {
                        source,
                        target,
                        destination,
                    }));
                    stack.push(Work::Expression(child, source, base));
                    stack.push(Work::Operation(Op::SetHint {
                        child: source,
                        hint: default
                            .map(HintRecipe::Primitive)
                            .unwrap_or(HintRecipe::None),
                    }));
                }
                _ => return Err(unsupported()),
            }
        }
        Ok(operations)
    }
    pub(crate) fn runtime_assertion_predicate_text(
        &self,
        arguments: &RuntimeMacroArguments,
    ) -> Result<String, Diagnostic> {
        arguments.require_source(&self.key())?;
        let range = arguments
            .operands()
            .first()
            .ok_or_else(|| invalid("assertion predicate source absent"))?;
        if range.end < range.start || range.end - range.start > 65_500 {
            return Err(invalid("assertion predicate spelling budget"));
        }
        Ok(self
            .parsed
            .tree()
            .syntax()
            .text()
            .slice(TextRange::new(
                TextSize::from(range.start),
                TextSize::from(range.end),
            ))
            .to_string())
    }
}
