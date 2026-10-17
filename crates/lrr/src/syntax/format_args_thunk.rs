//! Ordinary lowering of `builtin # format_args(template, args..)`, the
//! pinned parser's FORMAT_ARGS_EXPR (rustc's FormatArgs AST node). Argument
//! expressions evaluate once, in source order, through their own indexed
//! nodes; the parsed template is retained in the single format operation.
use super::{draft, error, id_for, return_pc};
use crate::syntax::{outline::BodyHandle, parse::ParsedFile};
use crate::{
    core::diagnostic::Diagnostic,
    frontend::thunks,
    ir::{
        format::FormatTemplate,
        op::Op,
        pc::{NodeId, PcKey, Phase},
        recipe::{RecipeExit, VerifiedRecipe},
    },
    syntax::body_index::BodyIndex,
};
use ra_ap_syntax::{
    AstNode, SyntaxNode,
    ast::{self, HasName},
};

impl ParsedFile {
    #[inline(never)]
    pub(super) fn initial_format_args_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        syntax: SyntaxNode,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let id = NodeId(key.node);
        let node = &index.nodes()[key.node as usize];
        let fallback = index.pc(id, Phase::Exit)?;
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        let expression = ast::FormatArgsExpr::cast(syntax).expect("checked format_args");
        let template = match expression.template() {
            Some(ast::Expr::Literal(literal)) => match literal.kind() {
                ast::LiteralKind::String(text) => match text.value() {
                    Ok(value) => value.into_owned(),
                    Err(_) => return unsupported("format_args template escape is invalid"),
                },
                _ => return unsupported("format_args template is not a string literal"),
            },
            _ => return unsupported("format_args template requires eager macro expansion"),
        };
        let mut arguments = Vec::new();
        // rustc borrows each argument (`Argument::new_display(&x)`): a place
        // argument is evaluated as its place and autoref'd (a builtin scalar
        // is read), exactly like a comparison's place operand; it is never
        // moved by formatting.
        let mut reads = Vec::new();
        let mut positional = 0usize;
        let mut named = Vec::new();
        for argument in expression.args() {
            let Some(value) = argument.expr() else {
                return unsupported("format_args argument expression absent");
            };
            match argument.name() {
                Some(name) => named.push(name.text().trim_start_matches("r#").to_owned()),
                None if named.is_empty() => positional += 1,
                None => return unsupported("positional format argument follows a named one"),
            }
            reads.push(self.comparison_operand_read(index, body, &value)?);
            arguments.push(id_for(index, body, value.syntax())?);
        }
        let template = match FormatTemplate::parse(&template, positional, &named) {
            Ok(template) => std::sync::Arc::new(template),
            Err(diagnostic) if diagnostic.code == "LRR-E-UNSUPPORTED" => {
                return unsupported(&diagnostic.message);
            }
            Err(diagnostic) => return Err(diagnostic),
        };
        let slots = arguments
            .iter()
            .map(|argument| {
                index.nodes()[argument.0 as usize]
                    .value
                    .ok_or_else(|| error("format argument value absent"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let destination = node
            .value
            .ok_or_else(|| error("format_args result absent"))?;
        // The autoref operation completing argument `ordinal`, if it is a
        // place argument.
        let autoref = |ordinal: usize| -> Result<Vec<Op>, Diagnostic> {
            let Some(read) = reads[ordinal] else {
                return Ok(Vec::new());
            };
            let node = &index.nodes()[arguments[ordinal].0 as usize];
            Ok(vec![Op::ComparisonOperand {
                place: node
                    .place
                    .ok_or_else(|| error("format argument place absent"))?,
                read,
                destination: node
                    .value
                    .ok_or_else(|| error("format argument value absent"))?,
            }])
        };
        let finish = |mut ops: Vec<Op>| {
            ops.push(Op::FormatArguments {
                template: template.clone(),
                arguments: slots.clone(),
                destination,
            });
            draft(
                index,
                key,
                ops,
                RecipeExit::Next(return_pc(index, body, expression.syntax())?),
            )
        };
        match Phase::from_code(key.phase) {
            Phase::Entry if arguments.is_empty() => finish(Vec::new()),
            Phase::Entry => draft(
                index,
                key,
                vec![],
                RecipeExit::Next(index.pc(arguments[0], Phase::Entry)?),
            ),
            Phase::Operand(n) => {
                let child = node
                    .operands
                    .get(usize::from(n))
                    .ok_or_else(|| error("format_args operand absent"))?;
                let ordinal = arguments
                    .iter()
                    .position(|argument| argument == child)
                    .ok_or_else(|| error("format_args continuation is not an argument"))?;
                if ordinal + 1 == arguments.len() {
                    finish(autoref(ordinal)?)
                } else {
                    draft(
                        index,
                        key,
                        autoref(ordinal)?,
                        RecipeExit::Next(index.pc(arguments[ordinal + 1], Phase::Entry)?),
                    )
                }
            }
            _ => unsupported("invalid format_args phase"),
        }
    }
}
