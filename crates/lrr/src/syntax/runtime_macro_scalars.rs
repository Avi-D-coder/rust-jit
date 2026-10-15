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
            BorrowAccess, HintRecipe, LiteralKind, LiteralRecipe, NumericBinary, NumericUnary, Op,
            PrimitiveType,
        },
        pc::{PcKey, SlotId},
    },
};
use ra_ap_syntax::{
    AstNode, AstToken, Edition, SyntaxKind, TextRange, TextSize,
    ast::{self, HasArgList, HasGenericArgs},
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
#[path = "runtime_macro_calls.rs"]
mod calls;
use calls::{ArgumentBorrowDraft, CallDraft};
#[cfg(test)]
#[path = "runtime_macro_call_tests.rs"]
mod call_tests;
#[cfg(test)]
#[path = "runtime_macro_reference_tests.rs"]
mod reference_tests;
pub(crate) use calls::{
    RuntimeMacroCall, RuntimeMacroCallPhase, RuntimeScalarPlan, RuntimeScalarStep,
};

#[cfg(test)]
#[path = "runtime_macro_control_tests.rs"]
mod control_tests;
#[path = "runtime_macro_controls.rs"]
mod controls;
pub(crate) use controls::{RuntimeScalarControl, RuntimeScalarMatchExhaustion};
enum Work {
    Expression(ast::Expr, SlotId, u32, Option<u32>),
    Operation(Op),
    Step(RuntimeScalarStep),
    Mark(controls::Label),
    Control(controls::Draft),
    Fault(Diagnostic, BodySpan),
}
impl ParsedFile {
    pub(crate) fn runtime_macro_scalar_plan(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        arguments: &RuntimeMacroArguments,
    ) -> Result<RuntimeScalarPlan, Diagnostic> {
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
        let mut fault_origins = Vec::new();
        let mut calls: Vec<CallDraft> = Vec::new();
        let mut controls = controls::Builder::new(usize::from(node.runtime_scalar_phases));
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
            stack.push(Work::Expression(expression, condition, base, None));
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
            stack.push(Work::Expression(right_expr, right, right_base, None));
            stack.push(Work::Operation(super::numeric_fallback::operand_hint(
                default,
                condition,
                right,
                Some(left),
                operator,
            )));
            stack.push(Work::Expression(left_expr, left, left_base, None));
            stack.push(Work::Operation(super::numeric_fallback::operand_hint(
                default, condition, left, None, operator,
            )));
        }
        operations.push(RuntimeScalarStep::Operation(Op::SetHint {
            child: condition,
            hint: HintRecipe::Primitive(PrimitiveType::Bool),
        }));
        let prefix_len = u32::try_from("fn __lrr_operand(){let __lrr_value=".len())
            .map_err(|_| invalid("operand prefix overflow"))?;
        while let Some(work) = stack.pop() {
            if operations.len() >= usize::from(node.runtime_scalar_phases)
                || stack.len() > usize::from(node.runtime_scalar_phases)
            {
                return Err(invalid("scalar assertion phase reservation exhausted"));
            }
            let Work::Expression(expression, destination, base, parent) = work else {
                match work {
                    Work::Operation(operation) => {
                        operations.push(RuntimeScalarStep::Operation(operation))
                    }
                    Work::Step(step) => operations.push(step),
                    Work::Mark(label) => controls.mark(label, operations.len())?,
                    Work::Control(draft) => controls.emit(draft, &mut operations)?,
                    Work::Fault(error, span) => {
                        fault_origins
                            .try_reserve(1)
                            .map_err(|_| invalid("scalar fault origin allocation"))?;
                        fault_origins.push((operations.len(), span));
                        operations.push(RuntimeScalarStep::Fault(error));
                    }
                    _ => unreachable!(),
                }
                continue;
            };
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
            let fault_span = original_span(&expression)?;
            let lowered = (|| -> Result<(), Diagnostic> {
                if expression
                    .syntax()
                    .children()
                    .any(|child| ast::Attr::can_cast(child.kind()))
                {
                    return Err(unsupported());
                }
                match expression {
                    ast::Expr::MatchExpr(expression) => {
                        let written = original_span(&ast::Expr::MatchExpr(expression.clone()))?;
                        controls::require_written(self, body, &written, expression.syntax())?;
                        controls.match_expression(
                            expression,
                            controls::Context {
                                destination,
                                base,
                                parent,
                            },
                            &mut allocate,
                            &mut stack,
                            &original_span,
                        )?;
                    }
                    ast::Expr::IfExpr(expression) => {
                        let written = original_span(&ast::Expr::IfExpr(expression.clone()))?;
                        controls::require_written(self, body, &written, expression.syntax())?;
                        controls.if_expression(
                            expression,
                            controls::Context {
                                destination,
                                base,
                                parent,
                            },
                            &mut allocate,
                            &mut stack,
                            &original_span,
                        )?;
                    }
                    ast::Expr::BlockExpr(block) => {
                        if block.modifier().is_some() {
                            return Err(unsupported());
                        }
                        let statements = block.stmt_list().ok_or_else(unsupported)?;
                        if statements.statements().next().is_some()
                            || statements
                                .syntax()
                                .children()
                                .any(|node| ast::Attr::can_cast(node.kind()))
                        {
                            return Err(unsupported());
                        }
                        if let Some(tail) = statements.tail_expr() {
                            stack.push(Work::Expression(tail, destination, base, parent));
                        } else {
                            stack.push(Work::Operation(Op::Unit { destination }));
                        }
                    }
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
                    ast::Expr::RefExpr(reference) => {
                        if reference.raw_token().is_some() {
                            return Err(unsupported());
                        }
                        let borrow_span = original_span(&ast::Expr::RefExpr(reference.clone()))?;
                        let written_operand = reference.expr().ok_or_else(unsupported)?;
                        let operand_span = original_span(&written_operand)?;
                        let mut operand = written_operand;
                        while let ast::Expr::ParenExpr(group) = operand {
                            if group
                                .syntax()
                                .children()
                                .any(|child| ast::Attr::can_cast(child.kind()))
                            {
                                return Err(unsupported());
                            }
                            operand = group.expr().ok_or_else(unsupported)?;
                        }
                        // Only the actual whole call argument can mint a caller
                        // borrow fact. Nested aggregate/projected places retain
                        // their typed integration frontier, never a copied value.
                        let owner = parent
                            .and_then(|ordinal| calls.get_mut(ordinal as usize))
                            .ok_or_else(unsupported)?;
                        let argument = owner
                            .arguments
                            .iter()
                            .position(|slot| *slot == destination)
                            .ok_or_else(unsupported)?;
                        let mut whole = ast::Expr::RefExpr(reference.clone());
                        while let Some(paren) =
                            whole.syntax().parent().and_then(ast::ParenExpr::cast)
                        {
                            whole = ast::Expr::ParenExpr(paren);
                        }
                        if original_span(&whole)? != owner.argument_spans[argument] {
                            return Err(unsupported());
                        }
                        if owner.argument_borrows[argument].is_some() {
                            return Err(unsupported());
                        }
                        let access = if reference.mut_token().is_some() {
                            BorrowAccess::Mutable
                        } else {
                            BorrowAccess::Shared
                        };
                        let temporary;
                        let place;
                        if let ast::Expr::PathExpr(path) = &operand {
                            let path = path.path().ok_or_else(unsupported)?;
                            let segment = path.segment().ok_or_else(unsupported)?;
                            if path.qualifier().is_some()
                                || segment.generic_arg_list().is_some()
                                || segment.coloncolon_token().is_some()
                            {
                                return Err(unsupported());
                            }
                            let name = segment.name_ref().ok_or_else(unsupported)?;
                            let reserved = allocate()?;
                            place =
                                SlotId(reserved.0.checked_add(1).ok_or_else(|| {
                                    invalid("scalar borrow local place overflow")
                                })?);
                            let binding = self.runtime_macro_binding_place(
                                body,
                                index,
                                key,
                                name.text(),
                                place,
                            )?;
                            temporary = None;
                            stack.push(Work::Operation(Op::Borrow {
                                place,
                                access,
                                destination,
                            }));
                            stack.push(Work::Operation(binding));
                        } else {
                            if !matches!(
                                operand,
                                ast::Expr::Literal(_)
                                    | ast::Expr::PrefixExpr(_)
                                    | ast::Expr::BinExpr(_)
                                    | ast::Expr::CallExpr(_)
                                    | ast::Expr::CastExpr(_)
                            ) {
                                return Err(unsupported());
                            }
                            // A dereference is an existing place, not a fresh
                            // scalar temporary. Reborrow/projected-place support
                            // must preserve its actual address and lease.
                            if matches!(&operand, ast::Expr::PrefixExpr(prefix)
                                if prefix.op_token().is_some_and(|token| token.text()=="*"))
                            {
                                return Err(unsupported());
                            }
                            let value = allocate()?;
                            place = SlotId(value.0.checked_add(1).ok_or_else(|| {
                                invalid("scalar borrow temporary place overflow")
                            })?);
                            temporary = Some(value);
                            stack.push(Work::Operation(Op::Borrow {
                                place,
                                access,
                                destination,
                            }));
                            stack.push(Work::Operation(Op::ValuePlace {
                                source: value,
                                destination: place,
                            }));
                            stack.push(Work::Expression(operand, value, base, parent));
                            stack.push(Work::Operation(Op::SetHint {
                                child: value,
                                hint: HintRecipe::BorrowTarget {
                                    reference: destination,
                                },
                            }));
                        }
                        owner.argument_borrows[argument] = Some(ArgumentBorrowDraft {
                            argument: u32::try_from(argument)
                                .map_err(|_| invalid("scalar borrow argument overflow"))?,
                            borrow_span,
                            operand_span,
                            access,
                            temporary,
                            place,
                        });
                    }
                    ast::Expr::ParenExpr(group) => {
                        stack.push(Work::Expression(
                            group.expr().ok_or_else(unsupported)?,
                            destination,
                            base,
                            parent,
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
                        stack.push(Work::Expression(right_expr, right, base, parent));
                        stack.push(Work::Operation(super::numeric_fallback::operand_hint(
                            right_default,
                            destination,
                            right,
                            Some(left),
                            operator,
                        )));
                        stack.push(Work::Expression(left_expr, left, base, parent));
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
                            let reference =
                                SlotId(reference.0.checked_add(1).ok_or_else(|| {
                                    invalid("assertion reference place overflow")
                                })?);
                            let place =
                                SlotId(destination.0.checked_add(1).ok_or_else(|| {
                                    invalid("assertion dereference place overflow")
                                })?);
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
                            return Ok(());
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
                            stack.push(Work::Expression(child_expr, source, base, parent));
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
                        stack.push(Work::Expression(child, source, base, parent));
                        stack.push(Work::Operation(Op::SetHint {
                            child: source,
                            hint: default
                                .map(HintRecipe::Primitive)
                                .unwrap_or(HintRecipe::None),
                        }));
                    }
                    ast::Expr::CallExpr(call) => {
                        let written_call = call.syntax().text().to_string();
                        let (name, explicit) = calls::source_call_path(&call)?;
                        let mut written = Vec::new();
                        for argument in call.arg_list().ok_or_else(unsupported)?.args() {
                            if written.len() >= 4096 {
                                return Err(invalid("scalar call arguments limit"));
                            }
                            written
                                .try_reserve(1)
                                .map_err(|_| invalid("scalar call arguments allocation"))?;
                            written.push(argument);
                        }
                        if calls.len() >= usize::from(node.runtime_scalar_phases)
                            || written.len() > 4096
                        {
                            return Err(invalid("scalar call count/arguments limit"));
                        }
                        written
                            .len()
                            .checked_mul(2)
                            .and_then(|n| n.checked_add(2))
                            .and_then(|n| n.checked_add(stack.len()))
                            .filter(|n| *n <= usize::from(node.runtime_scalar_phases))
                            .ok_or_else(|| invalid("scalar call work reservation exhausted"))?;
                        let ordinal = u32::try_from(calls.len())
                            .map_err(|_| invalid("scalar call ordinal overflow"))?;
                        let mut slots = Vec::new();
                        let mut spans = Vec::new();
                        let mut borrows = Vec::new();
                        slots
                            .try_reserve_exact(written.len())
                            .map_err(|_| invalid("scalar call argument slots allocation"))?;
                        spans
                            .try_reserve_exact(written.len())
                            .map_err(|_| invalid("scalar call argument spans allocation"))?;
                        written
                            .len()
                            .checked_mul(std::mem::size_of::<Option<ArgumentBorrowDraft>>())
                            .filter(|n| *n <= 32 * 1024 * 1024)
                            .ok_or_else(|| invalid("scalar borrow draft metadata limit"))?;
                        borrows
                            .try_reserve_exact(written.len())
                            .map_err(|_| invalid("scalar borrow draft allocation"))?;
                        borrows.resize_with(written.len(), || None);
                        for argument in &written {
                            slots.push(allocate()?);
                            spans.push(original_span(argument)?);
                        }
                        let span = original_span(&ast::Expr::CallExpr(call))?;
                        let original_start = body
                            .range
                            .start
                            .checked_add(span.relative_start)
                            .ok_or_else(|| invalid("scalar call source overflow"))?;
                        let original_end = body
                            .range
                            .start
                            .checked_add(span.relative_end)
                            .ok_or_else(|| invalid("scalar call source overflow"))?;
                        if tree
                            .syntax()
                            .text()
                            .slice(TextRange::new(original_start.into(), original_end.into()))
                            .to_string()
                            != written_call
                        {
                            return Err(invalid("scalar call original source differs"));
                        }
                        calls
                            .try_reserve(1)
                            .map_err(|_| invalid("scalar call draft allocation"))?;
                        let mut retained_slots = Vec::new();
                        retained_slots
                            .try_reserve_exact(slots.len())
                            .map_err(|_| invalid("scalar call retained slot allocation"))?;
                        retained_slots.extend_from_slice(&slots);
                        calls.push(CallDraft {
                            ordinal,
                            parent,
                            span,
                            name,
                            explicit,
                            arguments: retained_slots,
                            argument_spans: spans,
                            argument_borrows: borrows,
                            result: destination,
                        });
                        stack.push(Work::Step(RuntimeScalarStep::CallSeal(ordinal)));
                        for (arg, (expression, slot)) in
                            written.into_iter().zip(slots).enumerate().rev()
                        {
                            stack.push(Work::Step(RuntimeScalarStep::CallAdvance(
                                ordinal,
                                u32::try_from(arg)
                                    .map_err(|_| invalid("scalar argument ordinal overflow"))?,
                            )));
                            stack.push(Work::Expression(expression, slot, base, Some(ordinal)));
                        }
                        stack.push(Work::Step(RuntimeScalarStep::CallPrepare(ordinal)));
                    }
                    _ => return Err(unsupported()),
                }
                Ok(())
            })();
            if let Err(error) = lowered {
                if error.code != "LRR-E-UNSUPPORTED" {
                    return Err(error);
                }
                fault_origins
                    .try_reserve(1)
                    .map_err(|_| invalid("scalar fault origin allocation"))?;
                fault_origins.push((operations.len(), fault_span));
                operations.push(RuntimeScalarStep::Fault(error));
            }
        }
        if matches!(operations.last(), Some(RuntimeScalarStep::CallSeal(_)))
            || controls.needs_tail(operations.len())
        {
            operations
                .len()
                .checked_add(1)
                .filter(|n| *n <= usize::from(node.runtime_scalar_phases))
                .ok_or_else(|| invalid("scalar call terminal phase reservation exhausted"))?;
            operations.push(RuntimeScalarStep::Operation(Op::SetHint {
                child: condition,
                hint: HintRecipe::Primitive(PrimitiveType::Bool),
            }));
        }
        if operations.len() > usize::from(node.runtime_scalar_phases) {
            return Err(invalid("scalar assertion phase reservation exhausted"));
        }
        drop(stack);
        controls.finish(
            &mut operations,
            index,
            key,
            self.key(),
            (arguments.invocation(), body.range.start),
        )?;
        RuntimeScalarPlan::finish(
            operations,
            calls,
            fault_origins,
            index,
            key,
            self.key(),
            arguments.invocation(),
        )
    }
    #[cfg(test)]
    pub(crate) fn runtime_macro_scalar_operations(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        arguments: &RuntimeMacroArguments,
    ) -> Result<Vec<Op>, Diagnostic> {
        let plan = self.runtime_macro_scalar_plan(body, index, key, arguments)?;
        let mut operations = Vec::new();
        operations
            .try_reserve_exact(plan.steps().len())
            .map_err(|_| invalid("scalar operation compatibility allocation"))?;
        for step in plan.steps() {
            match step {
                RuntimeScalarStep::Operation(op) => operations.push(op.clone()),
                RuntimeScalarStep::Fault(error) => return Err(error.clone()),
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
