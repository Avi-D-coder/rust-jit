//! Exact original-source vec expression forms. No operand evaluation or runtime body inspection.
use super::{RuntimeMacroArguments, fault};
use crate::{
    facade::macros::RuntimeMacroKind,
    syntax::{
        ast_adapter::range, macro_calls::SourceMacroCall, parse::ParsedFile, spans::SourceRange,
    },
};
use ra_ap_syntax::{AstNode, Edition, ast};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VecForm {
    Empty,
    List,
    Repeat,
}
impl VecForm {
    pub(super) fn tag(self) -> u32 {
        match self {
            Self::Empty => 0,
            Self::List => 1,
            Self::Repeat => 2,
        }
    }
}
pub(super) fn capture(
    source: &ParsedFile,
    selected: &SourceMacroCall,
    inside: &str,
    token_range: SourceRange,
) -> Result<RuntimeMacroArguments, crate::core::diagnostic::Diagnostic> {
    let normalized = super::newline_input::NormalizedArguments::new(inside)?;
    let prefix = "fn __lrr_capture(){let _=[";
    let mut wrapped = String::new();
    let bytes = prefix
        .len()
        .checked_add(normalized.as_str().len())
        .and_then(|n| n.checked_add(3))
        .ok_or_else(|| fault("vec argument wrapper overflow"))?;
    wrapped
        .try_reserve_exact(bytes)
        .map_err(|_| fault("vec argument wrapper allocation"))?;
    wrapped.push_str(prefix);
    wrapped.push_str(normalized.as_str());
    wrapped.push_str("];}");
    let edition = match source.edition {
        2015 => Edition::Edition2015,
        2018 => Edition::Edition2018,
        2021 => Edition::Edition2021,
        2024 => Edition::Edition2024,
        _ => return Err(fault("vec argument edition unsupported")),
    };
    let parsed = ast::SourceFile::parse(&wrapped, edition);
    if !parsed.errors().is_empty() {
        return Err(fault(
            "vec arguments are not a complete empty/list/repeat form",
        ));
    }
    let array = parsed
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::ArrayExpr::cast)
        .ok_or_else(|| fault("vec argument wrapper array absent"))?;
    let mut operands = Vec::new();
    let form = match array.kind() {
        ast::ArrayExprKind::Repeat {
            initializer,
            repeat,
        } => {
            for operand in [initializer, repeat] {
                operands
                    .try_reserve(1)
                    .map_err(|_| fault("vec argument allocation"))?;
                operands.push(operand.ok_or_else(|| fault("vec repeat lacks initializer/count"))?);
            }
            VecForm::Repeat
        }
        ast::ArrayExprKind::ElementList(expressions) => {
            for expression in expressions {
                if operands.len() >= 10_000 {
                    return Err(fault("vec argument count limit"));
                }
                operands
                    .try_reserve(1)
                    .map_err(|_| fault("vec argument allocation"))?;
                operands.push(expression);
            }
            if operands.is_empty() {
                VecForm::Empty
            } else {
                VecForm::List
            }
        }
    };
    let base = token_range
        .start
        .checked_add(1)
        .ok_or_else(|| fault("vec argument base overflow"))?;
    let mut ranges = Vec::new();
    ranges
        .try_reserve_exact(operands.len())
        .map_err(|_| fault("vec source range allocation"))?;
    let mut no_item_syntax = true;
    let mut nodes = 0usize;
    for operand in operands {
        for node in operand.syntax().descendants() {
            nodes = nodes
                .checked_add(1)
                .filter(|n| *n <= 100_000)
                .ok_or_else(|| fault("vec argument structural work limit"))?;
            no_item_syntax &= !ast::Item::can_cast(node.kind())
                && !ast::MacroExpr::can_cast(node.kind())
                && !ast::MacroCall::can_cast(node.kind())
                && !ast::Attr::can_cast(node.kind());
        }
        let offset = range(operand.syntax().text_range());
        let start = usize::try_from(offset.start)
            .map_err(|_| fault("vec start offset overflow"))?
            .checked_sub(prefix.len())
            .ok_or_else(|| fault("vec operand precedes original input"))?;
        let end = usize::try_from(offset.end)
            .map_err(|_| fault("vec end offset overflow"))?
            .checked_sub(prefix.len())
            .filter(|n| *n <= normalized.as_str().len())
            .ok_or_else(|| fault("vec operand escapes original input"))?;
        let (start, end) = normalized.original_range(start, end)?;
        ranges.push(SourceRange {
            start: base
                .checked_add(u32::try_from(start).map_err(|_| fault("vec source offset overflow"))?)
                .ok_or_else(|| fault("vec source offset overflow"))?,
            end: base
                .checked_add(u32::try_from(end).map_err(|_| fault("vec source offset overflow"))?)
                .ok_or_else(|| fault("vec source offset overflow"))?,
        });
    }
    let mut declaration_arguments = Vec::new();
    declaration_arguments
        .try_reserve_exact(ranges.len())
        .map_err(|_| fault("vec declaration argument allocation"))?;
    declaration_arguments.extend_from_slice(&ranges);
    Ok(RuntimeMacroArguments {
        source: source.key(),
        invocation: selected.range(),
        kind: RuntimeMacroKind::Vec,
        operands: ranges,
        vec_form: Some(form),
        declaration_arguments,
        message: None,
        message_argument_count: 0,
        literal_message: None,
        literal_predicate: None,
        no_item_syntax,
        binding_predicate: None,
        zero_arg_predicate_call: None,
    })
}

#[cfg(test)]
#[path = "runtime_vec_newline_tests.rs"]
mod newline_tests;
