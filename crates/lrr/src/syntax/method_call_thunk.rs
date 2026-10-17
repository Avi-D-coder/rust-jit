//! Ordinary lowering of `receiver.method::<..>(args..)`. The receiver
//! evaluates once as a place (a temporary place for a value receiver); the
//! dispatcher selects the method from the receiver's reached type through the
//! shared impl member index and records the receiver adjustment (autoderef,
//! autoref or move); `Op::MethodReceiver` then forms the receiver argument and
//! the call continues through the ordinary call boundary.
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
    AstNode, SyntaxNode,
    ast::{self, HasArgList, HasGenericArgs},
};

/// The dispatcher's method-call name spelling: `.name`.
pub(crate) fn method_call_name(name: &str) -> String {
    format!(".{name}")
}

impl ParsedFile {
    #[inline(never)]
    pub(super) fn initial_method_call_thunk(
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
        let call = ast::MethodCallExpr::cast(syntax).expect("checked method call");
        let Some(receiver) = call.receiver() else {
            return unsupported("method receiver absent");
        };
        let Some(name) = call.name_ref() else {
            return unsupported("method name absent");
        };
        let Some(list) = call.arg_list() else {
            return unsupported("method argument list absent");
        };
        let receiver_id = id_for(index, body, receiver.syntax())?;
        let receiver_node = &index.nodes()[receiver_id.0 as usize];
        let receiver_place = receiver_node
            .place
            .ok_or_else(|| error("method receiver place absent"))?;
        let receiver_argument = node.runtime_temporaries[0]
            .ok_or_else(|| error("method receiver argument slot absent"))?;
        // The receiver argument's reserved place pair.
        let receiver_scratch = crate::ir::pc::SlotId(
            receiver_argument
                .0
                .checked_add(1)
                .ok_or_else(|| error("method receiver scratch place overflow"))?,
        );
        let arguments = list
            .args()
            .map(|argument| id_for(index, body, argument.syntax()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut slots = vec![receiver_argument];
        for argument in &arguments {
            slots.push(
                index.nodes()[argument.0 as usize]
                    .value
                    .ok_or_else(|| error("method argument value absent"))?,
            );
        }
        let explicit = call
            .generic_arg_list()
            .map(|list| {
                list.generic_args()
                    .map(|arg| arg.syntax().text().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let site = crate::ir::recipe::CallSiteId(id.0);
        let name = method_call_name(&name.text());
        let result = node.value.ok_or_else(|| error("method call result absent"))?;
        let seal = |mut ops: Vec<Op>| {
            ops.push(Op::CallSeal {
                site,
                name: name.clone(),
                explicit: explicit.clone(),
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
        let advance = |argument: usize| -> Result<Op, Diagnostic> {
            Ok(Op::CallAdvance {
                site,
                argument: u32::try_from(argument).map_err(|_| error("argument ordinal overflow"))?,
            })
        };
        match Phase::from_code(key.phase) {
            Phase::Entry => draft(
                index,
                key,
                vec![],
                RecipeExit::Next(index.pc(receiver_id, Phase::Entry)?),
            ),
            Phase::Operand(n) => {
                let child = node
                    .operands
                    .get(usize::from(n))
                    .ok_or_else(|| error("method call operand absent"))?;
                if *child == receiver_id {
                    let ops = if intrinsic_place_receiver(&receiver) {
                        Vec::new()
                    } else {
                        // A value receiver gets a temporary place, as `&value`.
                        vec![Op::ValuePlace {
                            source: receiver_node
                                .value
                                .ok_or_else(|| error("method receiver value absent"))?,
                            destination: receiver_place,
                        }]
                    };
                    return draft(index, key, ops, RecipeExit::Next(index.pc(id, Phase::Commit)?));
                }
                let ordinal = arguments
                    .iter()
                    .position(|argument| argument == child)
                    .ok_or_else(|| error("method call continuation is not an argument"))?;
                let ops = vec![advance(ordinal + 1)?];
                if ordinal + 1 == arguments.len() {
                    seal(ops)
                } else {
                    draft(
                        index,
                        key,
                        ops,
                        RecipeExit::Next(index.pc(arguments[ordinal + 1], Phase::Entry)?),
                    )
                }
            }
            // The receiver place is published: the dispatcher selects the
            // method from its reached type and records the adjustment.
            Phase::Commit => draft(
                index,
                key,
                vec![
                    Op::CallPrepare {
                        site,
                        name: name.clone(),
                        explicit: explicit.clone(),
                        arguments: slots.clone(),
                    },
                    Op::MethodReceiver {
                        site,
                        place: receiver_place,
                        scratch: receiver_scratch,
                        destination: receiver_argument,
                    },
                ],
                RecipeExit::Next(index.pc(id, Phase::Exit)?),
            ),
            Phase::Exit => {
                let ops = vec![advance(0)?];
                if arguments.is_empty() {
                    seal(ops)
                } else {
                    draft(
                        index,
                        key,
                        ops,
                        RecipeExit::Next(index.pc(arguments[0], Phase::Entry)?),
                    )
                }
            }
        }
    }
}
