//! Ordinary lowering of a call whose callee is a value: a local binding, a
//! parameter or any non-path expression (`f(x)`, `(make())(x)`). rustc
//! desugars it to an `Fn*` call on the callee; here the callee evaluates once
//! as a place (a temporary place for a value callee), the dispatcher selects
//! the callee from that place's reached callable type (`TypeKey::Callable`:
//! a function item's source function or a closure's body), and the written
//! arguments then cross the ordinary call boundary.
use super::{draft, error, id_for, intrinsic_place_receiver, return_pc};
use crate::{
    core::diagnostic::Diagnostic,
    frontend::thunks,
    ir::{
        op::Op,
        pc::{NodeId, PcKey, Phase},
        recipe::{RecipeExit, VerifiedRecipe},
    },
    syntax::{body_index::BodyIndex, outline::BodyHandle, parse::ParsedFile},
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasArgList},
};

use crate::ir::op::VALUE_CALL_NAME;

impl ParsedFile {
    #[inline(never)]
    pub(super) fn value_call_thunk(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        key: &PcKey,
        call: &ast::CallExpr,
        callee: &ast::Expr,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        let id = NodeId(key.node);
        let node = &index.nodes()[key.node as usize];
        let fallback = index.pc(id, Phase::Exit)?;
        let unsupported = |message: &str| {
            thunks::unsupported(index, key.clone(), message.into(), fallback.clone())
        };
        let Some(list) = call.arg_list() else {
            return unsupported("call argument list absent");
        };
        let callee_id = id_for(index, body, callee.syntax())?;
        let callee_node = &index.nodes()[callee_id.0 as usize];
        let callee_place = callee_node
            .place
            .ok_or_else(|| error("value callee place absent"))?;
        let arguments = list
            .args()
            .map(|argument| id_for(index, body, argument.syntax()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut slots = Vec::with_capacity(arguments.len());
        for argument in &arguments {
            slots.push(
                index.nodes()[argument.0 as usize]
                    .value
                    .ok_or_else(|| error("value call argument value absent"))?,
            );
        }
        let site = crate::ir::recipe::CallSiteId(id.0);
        let result = node
            .value
            .ok_or_else(|| error("value call result absent"))?;
        let seal = |mut ops: Vec<Op>| {
            ops.push(Op::CallSeal {
                site,
                name: VALUE_CALL_NAME.into(),
                explicit: Vec::new(),
                arguments: slots.clone(),
            });
            draft(
                index,
                key,
                ops,
                RecipeExit::Call {
                    site,
                    resume: return_pc(index, body, call.syntax())?,
                    result,
                },
            )
        };
        match Phase::from_code(key.phase) {
            Phase::Entry => draft(
                index,
                key,
                Vec::new(),
                RecipeExit::Next(index.pc(callee_id, Phase::Entry)?),
            ),
            Phase::Operand(n) => {
                let child = *node
                    .operands
                    .get(usize::from(n))
                    .ok_or_else(|| error("value call operand absent"))?;
                if child == callee_id {
                    // The callee place is published before the dispatcher
                    // selects the call from its type (at Commit).
                    let mut ops = if intrinsic_place_receiver(callee) {
                        Vec::new()
                    } else {
                        // A value callee gets a temporary place, as `&value`.
                        vec![Op::ValuePlace {
                            source: callee_node
                                .value
                                .ok_or_else(|| error("value callee value absent"))?,
                            destination: callee_place,
                        }]
                    };
                    ops.push(Op::CalleeDeref {
                        place: callee_place,
                    });
                    return draft(
                        index,
                        key,
                        ops,
                        RecipeExit::Next(index.pc(id, Phase::Commit)?),
                    );
                }
                let ordinal = arguments
                    .iter()
                    .position(|argument| *argument == child)
                    .ok_or_else(|| error("value call continuation is not an argument"))?;
                let advance = Op::CallAdvance {
                    site,
                    argument: u32::try_from(ordinal)
                        .map_err(|_| error("argument ordinal overflow"))?,
                };
                if ordinal + 1 == arguments.len() {
                    seal(vec![advance])
                } else {
                    draft(
                        index,
                        key,
                        vec![advance],
                        RecipeExit::Next(index.pc(arguments[ordinal + 1], Phase::Entry)?),
                    )
                }
            }
            Phase::Commit => {
                if arguments.is_empty() {
                    return seal(Vec::new());
                }
                draft(
                    index,
                    key,
                    vec![Op::CallPrepare {
                        site,
                        name: VALUE_CALL_NAME.into(),
                        explicit: Vec::new(),
                        arguments: slots.clone(),
                    }],
                    RecipeExit::Next(index.pc(arguments[0], Phase::Entry)?),
                )
            }
            _ => unsupported("invalid value call phase"),
        }
    }
}
