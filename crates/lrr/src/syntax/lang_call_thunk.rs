//! Ordinary lowering of an expression rustc lowers to a call of a
//! language-item function. `a..=b` is `RangeInclusive::new(a, b)`: the bounds
//! evaluate once, in order, as the call's arguments, and the call continues
//! through the ordinary call boundary (selection, argument hints, contracts,
//! frame entry and return cleanup) exactly as a written
//! `::core::ops::RangeInclusive::new(a, b)` does. There is no range-specific
//! evaluator: only this call-shape draft is specific to the syntax.
use super::{draft, error, id_for, return_pc};
use crate::{
    core::diagnostic::Diagnostic,
    frontend::thunks,
    ir::{
        op::Op,
        pc::{NodeId, PcKey, Phase},
        recipe::{CallSiteId, RecipeExit, VerifiedRecipe},
    },
    syntax::{body_index::BodyIndex, outline::BodyHandle, parse::ParsedFile},
};
use ra_ap_syntax::{AstNode, SyntaxNode, ast};

impl ParsedFile {
    #[inline(never)]
    pub(super) fn initial_lang_call_thunk(
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
        let Some(range) = ast::RangeExpr::cast(syntax) else {
            return unsupported("language-item call syntax is not a range expression");
        };
        let Some((start, end)) = crate::syntax::constructors::range_inclusive_bounds(&range)
        else {
            return unsupported("range expression is not an inclusive range with both bounds");
        };
        let arguments = [start, end]
            .iter()
            .map(|bound| id_for(index, body, bound.syntax()))
            .collect::<Result<Vec<_>, _>>()?;
        let slots = arguments
            .iter()
            .map(|argument| {
                index.nodes()[argument.0 as usize]
                    .value
                    .ok_or_else(|| error("range bound value absent"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let name = crate::syntax::constructors::range_inclusive_new_path(&self.profile);
        let site = CallSiteId(id.0);
        let result = node.value.ok_or_else(|| error("range call result absent"))?;
        let advance = |ordinal: usize| -> Result<Op, Diagnostic> {
            Ok(Op::CallAdvance {
                site,
                argument: u32::try_from(ordinal).map_err(|_| error("argument ordinal overflow"))?,
            })
        };
        match Phase::from_code(key.phase) {
            Phase::Entry => draft(
                index,
                key,
                vec![Op::CallPrepare {
                    site,
                    name,
                    explicit: Vec::new(),
                    arguments: slots,
                }],
                RecipeExit::Next(index.pc(arguments[0], Phase::Entry)?),
            ),
            Phase::Operand(n) => {
                let child = node
                    .operands
                    .get(usize::from(n))
                    .ok_or_else(|| error("range call operand absent"))?;
                let ordinal = arguments
                    .iter()
                    .position(|argument| argument == child)
                    .ok_or_else(|| error("range call continuation is not a bound"))?;
                if ordinal + 1 == arguments.len() {
                    draft(
                        index,
                        key,
                        vec![
                            advance(ordinal)?,
                            Op::CallSeal {
                                site,
                                name,
                                explicit: Vec::new(),
                                arguments: slots,
                            },
                        ],
                        RecipeExit::Call {
                            site,
                            resume: return_pc(index, body, range.syntax())?,
                            result,
                        },
                    )
                } else {
                    draft(
                        index,
                        key,
                        vec![advance(ordinal)?],
                        RecipeExit::Next(index.pc(arguments[ordinal + 1], Phase::Entry)?),
                    )
                }
            }
            _ => unsupported("invalid range call phase"),
        }
    }
}
