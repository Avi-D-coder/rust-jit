//! Pure entered intrinsic argument capture. The frontend selects the intrinsic by
//! definition identity first. Owned operand and message ranges retain written order;
//! capture never evaluates an operand, formats a message or prepares a thunk.
use super::{
    ast_adapter::range,
    macro_calls::SourceMacroCall,
    parse::{ParseKey, ParsedFile},
    spans::SourceRange,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    facade::macros::RuntimeMacroKind,
};
use ra_ap_syntax::{
    AstNode, Edition, TextRange, TextSize,
    ast::{self, HasArgList, HasGenericArgs},
};
#[path = "runtime_macro_newlines.rs"]
mod newline_input;
#[path = "runtime_vec_arguments.rs"]
mod vec_arguments;
pub use vec_arguments::VecForm;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-RUNTIME-MACRO-ARGUMENTS", message.into())
}
#[derive(Debug)]
pub struct RuntimeMacroArguments {
    source: ParseKey,
    invocation: SourceRange,
    kind: RuntimeMacroKind,
    operands: Vec<SourceRange>,
    vec_form: Option<VecForm>,
    declaration_arguments: Vec<SourceRange>,
    message: Option<SourceRange>,
    message_argument_count: u32,
    literal_message: Option<String>,
    literal_predicate: Option<String>,
    no_item_syntax: bool,
    binding_predicate: Option<String>,
    zero_arg_predicate_call: Option<String>,
}
impl RuntimeMacroArguments {
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.source.profile.semantics.capacity())
            .and_then(|n| {
                n.checked_add(
                    self.declaration_arguments
                        .capacity()
                        .checked_mul(std::mem::size_of::<SourceRange>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    self.operands
                        .capacity()
                        .checked_mul(std::mem::size_of::<SourceRange>())?,
                )
            })
            .and_then(|n| n.checked_add(self.literal_message.as_ref().map_or(0, String::capacity)))
            .and_then(|n| {
                n.checked_add(self.literal_predicate.as_ref().map_or(0, String::capacity))
            })
            .and_then(|n| {
                n.checked_add(
                    self.zero_arg_predicate_call
                        .as_ref()
                        .map_or(0, String::capacity),
                )
            })
            .and_then(|n| {
                n.checked_add(self.binding_predicate.as_ref().map_or(0, String::capacity))
            })
            .ok_or_else(|| fault("runtime argument metadata overflow"))
    }
    pub(crate) fn no_item_syntax(&self) -> bool {
        self.no_item_syntax
    }
    pub(crate) fn binding_predicate(&self) -> Option<&str> {
        self.binding_predicate.as_deref()
    }
    pub fn zero_arg_predicate_call(&self) -> Option<&str> {
        self.zero_arg_predicate_call.as_deref()
    }
    pub fn literal_predicate(&self) -> Option<&str> {
        self.literal_predicate.as_deref()
    }
    pub fn source(&self) -> &ParseKey {
        &self.source
    }
    pub fn invocation(&self) -> SourceRange {
        self.invocation
    }
    pub fn kind(&self) -> RuntimeMacroKind {
        self.kind
    }
    pub(crate) fn declaration_arguments(&self) -> &[SourceRange] {
        &self.declaration_arguments
    }
    pub fn vec_form(&self) -> Option<VecForm> {
        self.vec_form
    }
    pub fn operands(&self) -> &[SourceRange] {
        &self.operands
    }
    /// The entire remaining formatting token tail, including named arguments,
    /// comments and trailing comma. It is not an eager guest argument packet.
    pub fn message_argument_count(&self) -> u32 {
        self.message_argument_count
    }
    pub fn literal_message(&self) -> Option<&str> {
        self.literal_message.as_deref()
    }
    pub fn message(&self) -> Option<SourceRange> {
        self.message
    }
    pub fn require_source(&self, key: &ParseKey) -> Result<(), Diagnostic> {
        if key != &self.source {
            return Err(fault("runtime macro argument source/profile is stale"));
        }
        Ok(())
    }
}
impl StableEncode for RuntimeMacroArguments {
    const DOMAIN: &'static str = "lrr/v2/runtime-macro-arguments";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.source);
        e.u32(self.invocation.start);
        e.u32(self.invocation.end);
        e.u32(self.kind.tag());
        e.sequence(&self.operands, |e, r| {
            e.u32(r.start);
            e.u32(r.end);
        });
        e.sequence(&self.declaration_arguments, |e, r| {
            e.u32(r.start);
            e.u32(r.end);
        });
        e.bool(self.message.is_some());
        if let Some(r) = self.message {
            e.u32(r.start);
            e.u32(r.end);
        }
        // Preserve kind0–4 encodings; the new explicit tag5 carries a vec form.
        if let Some(form) = self.vec_form {
            e.u32(form.tag());
        }
    }
}
impl ParsedFile {
    pub fn runtime_macro_arguments(
        &self,
        selected: &SourceMacroCall,
        kind: RuntimeMacroKind,
    ) -> Result<RuntimeMacroArguments, Diagnostic> {
        if selected.key() != &self.key() {
            return Err(fault("selected runtime invocation source/profile is stale"));
        }
        let r = selected.range();
        let root = self.parsed.tree();
        let call = root
            .syntax()
            .covering_element(TextRange::new(
                TextSize::from(r.start),
                TextSize::from(r.end),
            ))
            .into_node()
            .and_then(|n| {
                n.ancestors().find_map(|n| {
                    let call = ast::MacroCall::cast(n)?;
                    (range(call.syntax().text_range()) == r).then_some(call)
                })
            })
            .ok_or_else(|| fault("selected runtime invocation is not an exact macro call"))?;
        let tokens = call
            .token_tree()
            .ok_or_else(|| fault("runtime macro has no delimited arguments"))?;
        let token_range = range(tokens.syntax().text_range());
        let text = tokens.syntax().text().to_string();
        if text.len() > 131_072 || text.len() < 2 {
            return Err(fault("runtime macro argument size limit"));
        }
        let delimiter = (
            text.as_bytes()[0],
            *text.as_bytes().last().expect("bounded argument text"),
        );
        if !matches!(delimiter, (b'(', b')') | (b'[', b']') | (b'{', b'}')) {
            return Err(fault("runtime macro arguments have incomplete delimiters"));
        }
        let inside = &text[1..text.len() - 1];
        if kind == RuntimeMacroKind::Vec {
            if self.generated_origin.is_some() {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "generated vec runtime arguments require token-aware list/repeat origins"
                        .into(),
                ));
            }
            return vec_arguments::capture(self, selected, inside, token_range);
        }
        let normalized = newline_input::NormalizedArguments::new(inside)?;
        let prefix = "fn __lrr_capture(){__lrr_arguments(";
        let wrapped = format!("{prefix}{});}}", normalized.as_str());
        let edition = match self.edition {
            2015 => Edition::Edition2015,
            2018 => Edition::Edition2018,
            2021 => Edition::Edition2021,
            2024 => Edition::Edition2024,
            _ => return Err(fault("runtime macro argument edition is unsupported")),
        };
        let parsed = ast::SourceFile::parse(&wrapped, edition);
        if !parsed.errors().is_empty() {
            return Err(fault(
                "runtime macro arguments are not a complete expression list",
            ));
        }
        let synthetic = parsed
            .tree()
            .syntax()
            .descendants()
            .find_map(ast::CallExpr::cast)
            .ok_or_else(|| fault("runtime argument parser lacks its wrapper"))?;
        let args = synthetic
            .arg_list()
            .ok_or_else(|| fault("runtime argument list is missing"))?;
        let required = match kind {
            RuntimeMacroKind::Panic
            | RuntimeMacroKind::Println
            | RuntimeMacroKind::Vec
            | RuntimeMacroKind::Unreachable
            | RuntimeMacroKind::Unimplemented => 0,
            RuntimeMacroKind::Assert => 1,
            RuntimeMacroKind::AssertEq | RuntimeMacroKind::AssertNe => 2,
        };
        let mut literal_message = None;
        let mut literal_predicate = None;
        let mut zero_arg_predicate_call = None;
        let mut ranges = Vec::new();
        let mut no_item_syntax = true;
        let mut binding_predicate = None;
        for argument in args.args() {
            no_item_syntax &= !argument.syntax().descendants().any(|node| {
                ast::Item::can_cast(node.kind())
                    || ast::MacroExpr::can_cast(node.kind())
                    || ast::MacroCall::can_cast(node.kind())
                    || ast::Attr::can_cast(node.kind())
            });
            if ranges.len() >= 10_000 {
                return Err(fault("runtime macro argument count limit"));
            }
            if kind == RuntimeMacroKind::Assert
                && ranges.is_empty()
                && let ast::Expr::Literal(literal) = &argument
                && matches!(
                    literal.token().kind(),
                    ra_ap_syntax::SyntaxKind::TRUE_KW | ra_ap_syntax::SyntaxKind::FALSE_KW
                )
            {
                literal_predicate = Some(literal.syntax().text().to_string());
            }
            if kind == RuntimeMacroKind::Assert
                && ranges.is_empty()
                && let ast::Expr::CallExpr(call) = &argument
                && call
                    .arg_list()
                    .is_some_and(|list| list.args().next().is_none())
                && !call
                    .syntax()
                    .children()
                    .any(|n| ast::Attr::can_cast(n.kind()))
                && let Some(ast::Expr::PathExpr(callee)) = call.expr()
                && let Some(path) = callee.path()
                && path.qualifier().is_none()
                && let Some(segment) = path.segment()
                && segment.generic_arg_list().is_none()
                && segment.coloncolon_token().is_none()
                && let Some(name) = segment.name_ref()
            {
                if name.text().len() > 4096 {
                    return Err(fault("assertion predicate call name exceeds budget"));
                }
                crate::expand::scopes::MacroName::parse(name.text(), self.edition)?;
                zero_arg_predicate_call = Some(name.text().to_string());
            }
            if kind == RuntimeMacroKind::Assert
                && ranges.is_empty()
                && let ast::Expr::PathExpr(expression) = &argument
                && !expression
                    .syntax()
                    .children()
                    .any(|n| ast::Attr::can_cast(n.kind()))
                && let Some(path) = expression.path()
                && path.qualifier().is_none()
                && let Some(segment) = path.segment()
                && segment.generic_arg_list().is_none()
                && segment.coloncolon_token().is_none()
                && let Some(name) = segment.name_ref()
            {
                if name.text().len() > 4096 {
                    return Err(fault("assertion binding name exceeds budget"));
                }
                crate::expand::scopes::MacroName::parse(name.text(), self.edition)?;
                binding_predicate = Some(name.text().to_string());
            }
            let offset = range(argument.syntax().text_range());
            let start = usize::try_from(offset.start)
                .map_err(|_| fault("runtime argument offset overflow"))?
                .checked_sub(prefix.len())
                .ok_or_else(|| fault("runtime argument precedes its input"))?;
            let end = usize::try_from(offset.end)
                .map_err(|_| fault("runtime argument offset overflow"))?
                .checked_sub(prefix.len())
                .filter(|n| *n <= normalized.as_str().len())
                .ok_or_else(|| fault("runtime argument escapes its input"))?;
            // The scratch parser follows Rust CRLF normalization. All retained
            // operand/message ranges and literal provenance remain original.
            let (start, end) = normalized.original_range(start, end)?;
            if ranges.len() == required {
                literal_message =
                    super::ast_adapter::string_value(&inside[start..end], self.edition);
            }
            let base = token_range
                .start
                .checked_add(1)
                .ok_or_else(|| fault("runtime argument base overflow"))?;
            ranges
                .try_reserve(1)
                .map_err(|_| fault("runtime argument allocation failed"))?;
            ranges.push(SourceRange {
                start: base
                    .checked_add(
                        u32::try_from(start)
                            .map_err(|_| fault("runtime argument start overflow"))?,
                    )
                    .ok_or_else(|| fault("runtime argument start overflow"))?,
                end: base
                    .checked_add(
                        u32::try_from(end).map_err(|_| fault("runtime argument end overflow"))?,
                    )
                    .ok_or_else(|| fault("runtime argument end overflow"))?,
            });
        }
        let count = match kind {
            RuntimeMacroKind::Panic
            | RuntimeMacroKind::Println
            | RuntimeMacroKind::Vec
            | RuntimeMacroKind::Unreachable
            | RuntimeMacroKind::Unimplemented => 0,
            RuntimeMacroKind::Assert => 1,
            RuntimeMacroKind::AssertEq | RuntimeMacroKind::AssertNe => 2,
        };
        if ranges.len() < count {
            return Err(fault("runtime assertion lacks its required operands"));
        }
        let message = ranges.get(count).map(|first| SourceRange {
            start: first.start,
            end: token_range.end - 1,
        });
        let message_argument_count = u32::try_from(ranges.len() - count)
            .map_err(|_| fault("runtime message argument count overflow"))?;
        let declaration_arguments = ranges.clone();
        ranges.truncate(count);
        Ok(RuntimeMacroArguments {
            source: self.key(),
            invocation: r,
            kind,
            operands: ranges,
            vec_form: None,
            declaration_arguments,
            message,
            message_argument_count,
            literal_message,
            literal_predicate,
            no_item_syntax,
            binding_predicate,
            zero_arg_predicate_call,
        })
    }
}
