//! Cold admission of the exact retained ordinary pattern decision receipt.
//! Source/type/name preparation is deliberately absent from this verifier.
use crate::ir::{
    op::{Op, PatternDecision, PatternOperation},
    pc::{NodeId, PcKey, Phase, SlotId},
    recipe::{RecipeDraft, RecipeExit},
    validate::{invalid, pattern_descendant},
};
use crate::{
    core::diagnostic::Diagnostic,
    syntax::{
        body_index::BodyIndex,
        pattern_product::{DecisionKind, PatternProductPlan},
    },
};
use std::sync::Arc;
#[cfg(test)]
#[path = "pattern_decision_tests.rs"]
mod tests;

pub(super) fn transition(
    body: &BodyIndex,
    key: &PcKey,
    owner: NodeId,
    plan: &PatternProductPlan,
    action: &PatternDecision,
) -> Result<(), Diagnostic> {
    plan.require_index(body)?;
    if owner != plan.owner() || !pattern_descendant(body, NodeId(key.node), owner) {
        return Err(invalid("decision owner differs from prepared source"));
    }
    let selected = &body.nodes()[key.node as usize];
    let root = &body.nodes()[plan.root().0 as usize];
    let phase = Phase::from_code(key.phase);
    let row = plan.row(selected.id);
    let needs_source = row.is_some_and(|row| {
        matches!(
            row.kind,
            DecisionKind::Scalar(_) | DecisionKind::Binding(_) | DecisionKind::Tuple(_)
        )
    });
    let admitted = match action {
        PatternDecision::Begin {source,matched,unmatched,unmatched_inactive} => {
            let entry = if let Some(arm) = plan.arm() {
                key == &body.pc(arm,Phase::Entry)?
            } else {
                selected.id == owner && matches!(phase,Phase::Operand(_))
                    && selected.operands.iter().position(|operand| body.nodes()[operand.0 as usize].value == Some(plan.source()))
                        .is_some_and(|ordinal| Phase::Operand(u16::try_from(ordinal).unwrap_or(u16::MAX)).code().ok() == Some(key.phase))
            };
            entry && *source == plan.source() && matched == plan.matched() && unmatched == plan.unmatched() && unmatched_inactive == plan.unmatched_inactive()
        }
        PatternDecision::EnterAttempt {scope} => plan.arm().is_some() && selected.id == root.id && phase == Phase::Entry && *scope == root.scope,
        PatternDecision::Observe => row.is_some() && matches!(phase,Phase::Entry|Phase::Operand(1)|Phase::Operand(2)|Phase::Commit|Phase::Exit),
        PatternDecision::StartSource => needs_source && phase == Phase::Entry,
        PatternDecision::SourceStep => needs_source && phase == Phase::Operand(0),
        PatternDecision::CheckTuple {place,arity} => row.is_some_and(|row| matches!(&row.kind,DecisionKind::Tuple(children) if children.len() == *arity as usize))
            && phase == Phase::Operand(1) && Some(*place) == source_place(body,owner,plan,selected.id),
        PatternDecision::Enter => row.is_some_and(|row| !matches!(row.kind,DecisionKind::Scalar(_))) && if needs_source {phase == Phase::Operand(1)} else {phase == Phase::Entry},
        PatternDecision::Success => row.is_some() && phase == Phase::Commit,
        PatternDecision::Test {result,first,upper_pending} => {
            let test = if plan.arm().is_some() {body.nodes()[owner.0 as usize].runtime_temporaries[1]} else {body.nodes()[owner.0 as usize].value};
            let bounds = row.and_then(|row| row.scalar_upper_pending());
            Some(*result) == test && bounds.is_some_and(|both| if *first {
                phase == Phase::Operand(1) && *upper_pending == both
            } else {phase == Phase::Operand(2) && both && !*upper_pending})
        }
        PatternDecision::Fail {node:Some(node)} => *node == selected.id && row.is_some_and(|row| matches!(row.kind,DecisionKind::Scalar(_))) && phase == Phase::Exit,
        PatternDecision::Fail {node:None} => plan.arm() == Some(selected.id) && selected.guard_scope.is_some() && phase == Phase::Exit,
        PatternDecision::Cleanup => selected.id == root.id && phase == Phase::PatternCleanup,
    };
    if !admitted {
        return Err(invalid(
            "decision action differs from exact prepared node/phase/source",
        ));
    }
    Ok(())
}
pub(super) fn source_place(
    body: &BodyIndex,
    owner: NodeId,
    plan: &PatternProductPlan,
    node: NodeId,
) -> Option<SlotId> {
    let depth = plan.row(node)?.source_path.len();
    let node = if depth % 2 == 0 { node } else { owner };
    body.nodes().get(node.0 as usize)?.runtime_temporaries[0]?
        .0
        .checked_add(1)
        .map(SlotId)
}
/// Ordinary ReadPlace is admitted only for the retained node's exact source
/// cursor place and own value scratch. Runtime still demands source identity,
/// live place authority and intrinsic non-consuming Copy membership.
pub(super) fn read_place(
    body: &BodyIndex,
    draft: &RecipeDraft,
    place: SlotId,
    destination: SlotId,
) -> bool {
    if Phase::from_code(draft.key.phase) != Phase::Operand(1) {
        return false;
    }
    draft.operations.iter().any(|op| match op {
        Op::Pattern {
            operation:
                PatternOperation::Decision {
                    owner,
                    plan,
                    transition: PatternDecision::Observe,
                },
        } => {
            plan.row(NodeId(draft.key.node)).is_some_and(|row| {
                matches!(row.kind, DecisionKind::Scalar(_) | DecisionKind::Binding(_))
            }) && Some(place) == source_place(body, *owner, plan, NodeId(draft.key.node))
                && body.nodes()[draft.key.node as usize].runtime_temporaries[0] == Some(destination)
        }
        _ => false,
    })
}
pub(super) fn exit(draft: &RecipeDraft, owner: NodeId) -> Result<(), Diagnostic> {
    let mut terminal = None;
    for (ordinal, op) in draft.operations.iter().enumerate() {
        if let Op::Pattern {
            operation:
                PatternOperation::Decision {
                    owner: actual,
                    plan,
                    transition,
                },
        } = op
        {
            if *actual != owner {
                return Err(invalid("pattern exit owner differs from transition"));
            }
            if matches!(
                transition,
                PatternDecision::Begin { .. }
                    | PatternDecision::Enter
                    | PatternDecision::Success
                    | PatternDecision::Fail { .. }
                    | PatternDecision::Cleanup
                    | PatternDecision::StartSource
                    | PatternDecision::SourceStep
                    | PatternDecision::Test { .. }
            ) {
                if terminal.replace((ordinal, Arc::as_ptr(plan))).is_some() {
                    return Err(invalid(
                        "pattern recipe has multiple continuation producers",
                    ));
                }
            }
        }
    }
    if terminal.is_none_or(|(ordinal, _)| ordinal + 1 != draft.operations.len()) {
        return Err(invalid(
            "pattern exit requires its last completed prepared transition",
        ));
    }
    Ok(())
}
/// A decision producer cannot be paired with a caller-selected static exit.
pub(super) fn exits(draft: &RecipeDraft) -> Result<(), Diagnostic> {
    for op in &draft.operations {
        if let Op::Pattern {
            operation:
                PatternOperation::Decision {
                    owner, transition, ..
                },
        } = op
            && matches!(
                transition,
                PatternDecision::Begin { .. }
                    | PatternDecision::Enter
                    | PatternDecision::Success
                    | PatternDecision::Fail { .. }
                    | PatternDecision::Cleanup
                    | PatternDecision::StartSource
                    | PatternDecision::SourceStep
                    | PatternDecision::Test { .. }
            )
            && !matches!(draft.exits.as_slice(),[RecipeExit::PatternNext {owner:actual}] if actual == owner)
        {
            return Err(invalid(
                "prepared pattern transition requires its checked dynamic exit",
            ));
        }
        if let Op::Pattern {
            operation:
                PatternOperation::Decision {
                    plan,
                    transition: PatternDecision::Fail { node: None },
                    ..
                },
        } = op
        {
            let scope = plan.index().nodes()[plan
                .arm()
                .ok_or_else(|| invalid("guard failure has no prepared arm"))?
                .0 as usize]
                .scope;
            if draft.operations.iter().filter(|operation| matches!(operation,Op::ScopeLeave {scope:actual} if *actual == scope)).count() != 1 {
                return Err(invalid("guard retry requires exactly one ordinary arm scope leave"));
            }
        }
    }
    Ok(())
}
