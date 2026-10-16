//! Pure structural admission. Never fetches a successor or checks a guest type.
use super::{
    op::Op,
    pc::{NodeId, PcKey, Phase, SlotId, SlotKind},
    recipe::{ControlRecipe, Recipe, RecipeDraft, RecipeExit, VerifiedRecipe},
};
use crate::{
    core::diagnostic::Diagnostic,
    syntax::{body_index::BodyIndex, spans::BodySpan},
};
use std::collections::BTreeSet;
#[path = "pattern_decision.rs"]
mod pattern_decision;
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-IR", message.into())
}
fn pattern_descendant(body: &BodyIndex, mut child: NodeId, owner: NodeId) -> bool {
    for _ in 0..4096 {
        if child == owner {
            return true;
        }
        let Some(parent) = body
            .nodes()
            .get(child.0 as usize)
            .and_then(|node| node.parent)
        else {
            return false;
        };
        child = parent;
    }
    false
}
fn pattern_transition(
    body: &BodyIndex,
    key: &PcKey,
    operation: &super::op::PatternOperation,
) -> Result<(), Diagnostic> {
    use super::op::PatternOperation;
    use crate::syntax::body_index::{ExpressionKind, NodeRole};
    if let PatternOperation::Decision {
        owner,
        plan,
        transition,
    } = operation
    {
        return pattern_decision::transition(body, key, *owner, plan, transition);
    }
    let owner = match operation {
        PatternOperation::Begin { owner, .. }
        | PatternOperation::StageBinding { owner, .. }
        | PatternOperation::Preview { owner }
        | PatternOperation::Finish { owner, .. }
        | PatternOperation::Rollback { owner } => *owner,
        PatternOperation::Decision { .. } => unreachable!(),
    };
    let selected = body
        .nodes()
        .get(owner.0 as usize)
        .ok_or_else(|| invalid("pattern transaction owner absent"))?;
    let is_match = selected.role == NodeRole::Expression(ExpressionKind::Match);
    let conditional = selected.role == NodeRole::Expression(ExpressionKind::Other)
        && selected.clause_scope.is_some();
    if (!is_match && !conditional) || !pattern_descendant(body, NodeId(key.node), owner) {
        return Err(invalid(
            "pattern transition is outside its indexed ordinary owner",
        ));
    }
    // Both ordinary pattern owners have exactly one direct expression child.
    // Pattern-bound expressions can appear earlier in the flattened operand
    // list; they are descendants of their pattern, not the owning scrutinee.
    let mut scrutinee = None;
    for operand in &selected.operands {
        let node = body
            .nodes()
            .get(operand.0 as usize)
            .ok_or_else(|| invalid("pattern owner operand absent"))?;
        if node.parent == Some(owner) && matches!(node.role, NodeRole::Expression(_)) {
            let value = node
                .value
                .ok_or_else(|| invalid("pattern source value absent"))?;
            if scrutinee.replace(value).is_some() {
                return Err(invalid(
                    "pattern owner has ambiguous direct source operands",
                ));
            }
        }
    }
    let scrutinee = scrutinee.ok_or_else(|| invalid("pattern source operand absent"))?;
    match operation {
        PatternOperation::Begin {
            alternative,
            source,
            retry,
            ..
        } => {
            let alternative_node = body
                .nodes()
                .get(alternative.0 as usize)
                .ok_or_else(|| invalid("pattern alternative absent"))?;
            if *source != scrutinee
                || alternative_node.role != NodeRole::Pattern
                || !pattern_descendant(body, *alternative, owner)
                || (is_match && (key.node != alternative.0 || key.phase != Phase::Entry.code()?))
                || (conditional
                    && (key.node != owner.0
                        || !matches!(Phase::from_code(key.phase), Phase::Operand(_))))
            {
                return Err(invalid("pattern begin source/alternative/entry differs"));
            }
            slot(body, *source, SlotKind::Value)?;
            pc(body, retry)?;
            let target = &body.nodes()[retry.node as usize];
            if conditional {
                if *retry != body.pc(owner, Phase::Exit)? {
                    return Err(invalid("condition retry is not its indexed exit"));
                }
            } else if *retry != body.pc(owner, Phase::Exit)?
                && (retry.phase != Phase::Entry.code()?
                    || !matches!(target.role, NodeRole::Pattern | NodeRole::MatchArm)
                    || !pattern_descendant(body, target.id, owner)
                    || target.range.start <= alternative_node.range.start)
            {
                return Err(invalid(
                    "pattern retry is not a forward continuation of its owner",
                ));
            }
        }
        PatternOperation::StageBinding {
            binding, source, ..
        } => {
            if key.phase != Phase::Commit.code()?
                || (is_match && key.node != binding.0)
                || body.nodes().get(binding.0 as usize).is_none_or(|node| {
                    node.role != NodeRole::Pattern || !pattern_descendant(body, *binding, owner)
                })
                || (*source != scrutinee && !selected.runtime_temporaries.contains(&Some(*source)))
            {
                return Err(invalid(
                    "pending pattern binding source/target/phase differs",
                ));
            }
            slot(body, *source, SlotKind::Value)?;
        }
        PatternOperation::Preview { .. } => {
            if !is_match || key.phase != Phase::Commit.code()? {
                return Err(invalid(
                    "pattern guard preview is outside its selected commit phase",
                ));
            }
        }
        PatternOperation::Finish { matched, .. } => {
            if let Some(matched) = matched {
                if !conditional
                    || key.node != owner.0
                    || key.phase != Phase::Exit.code()?
                    || selected.value != Some(*matched)
                {
                    return Err(invalid("conditional pattern finish Bool differs"));
                }
                slot(body, *matched, SlotKind::Value)?;
            } else if key.phase != Phase::Commit.code()? {
                return Err(invalid(
                    "pattern commit is outside its selected commit phase",
                ));
            }
        }
        PatternOperation::Rollback { .. } => {
            if !is_match || key.phase != Phase::Exit.code()? {
                return Err(invalid(
                    "pattern rollback is outside its failed alternative exit",
                ));
            }
        }
        PatternOperation::Decision { .. } => unreachable!(),
    }
    Ok(())
}
fn definition_numeric_context(
    context: super::op::DefinitionNumericContext,
) -> Result<(), Diagnostic> {
    match context {
        super::op::DefinitionNumericContext::Parameter(ordinal) if ordinal >= 100_000 => Err(
            invalid("definition numeric parameter ordinal exceeds structural limit"),
        ),
        super::op::DefinitionNumericContext::Parameter(_)
        | super::op::DefinitionNumericContext::Result
        | super::op::DefinitionNumericContext::ResultOrDefault { .. } => Ok(()),
    }
}
fn numeric_primitive(primitive: super::op::PrimitiveType) -> Result<(), Diagnostic> {
    use super::op::PrimitiveType;
    match primitive {
        PrimitiveType::I8
        | PrimitiveType::I16
        | PrimitiveType::I32
        | PrimitiveType::I64
        | PrimitiveType::I128
        | PrimitiveType::Isize
        | PrimitiveType::U8
        | PrimitiveType::U16
        | PrimitiveType::U32
        | PrimitiveType::U64
        | PrimitiveType::U128
        | PrimitiveType::Usize
        | PrimitiveType::F32
        | PrimitiveType::F64 => Ok(()),
        PrimitiveType::Unit | PrimitiveType::Bool | PrimitiveType::Char => Err(invalid(
            "definition numeric compound RHS is not a numeric primitive",
        )),
    }
}
fn pc(body: &BodyIndex, key: &PcKey) -> Result<(), Diagnostic> {
    if &key.body != body.body() {
        return Err(invalid("recipe PC belongs to another body"));
    }
    body.pc(NodeId(key.node), Phase::from_code(key.phase))
        .map_err(|_| invalid("recipe PC node/phase absent"))?;
    Ok(())
}
fn lexical_target(
    body: &BodyIndex,
    key: &PcKey,
    wanted: crate::syntax::body_index::LabelId,
) -> Result<(), Diagnostic> {
    let mut current = body.nodes()[key.node as usize].enclosing_syntax_target;
    for _ in 0..body.labels().len() {
        let Some(id) = current else { break };
        if id == wanted {
            return Ok(());
        }
        current = body
            .labels()
            .get(id.0 as usize)
            .ok_or_else(|| invalid("control ancestry absent"))?
            .parent;
    }
    Err(invalid("control target is not a lexical ancestor"))
}
fn slot(body: &BodyIndex, id: SlotId, kind: SlotKind) -> Result<(), Diagnostic> {
    if body
        .slot_kind(id)
        .map_err(|_| invalid("recipe slot absent"))?
        != kind
    {
        return Err(invalid("recipe slot kind mismatch"));
    }
    Ok(())
}
fn span(body: &BodyIndex, origin: &BodySpan) -> Result<(), Diagnostic> {
    if &origin.body_key != body.body()
        || origin.relative_start > origin.relative_end
        || origin.relative_end > body.nodes()[0].range.end
    {
        return Err(invalid("recipe source origin outside body"));
    }
    Ok(())
}
fn charge(bytes: &mut usize, n: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(n)
        .filter(|n| *n <= 64 * 1024 * 1024)
        .ok_or_else(|| invalid("recipe metadata limit"))?;
    Ok(())
}
/// Structural permission for the scalar binding lane only. Name resolution is
/// supplied separately by the private source admission before this recipe is
/// constructed; these exact slots never authorize an aggregate or reference copy.
fn scalar_binding_copy(body: &BodyIndex, draft: &RecipeDraft) -> Option<(SlotId, SlotId, SlotId)> {
    use crate::syntax::body_index::{ExpressionKind, NodeRole};
    let pattern = body.nodes().get(draft.key.node as usize)?;
    if pattern.role != NodeRole::Pattern || draft.key.phase != Phase::Commit.code().ok()? {
        return None;
    }
    let arm = body.nodes().get(pattern.parent?.0 as usize)?;
    if arm.role != NodeRole::MatchArm {
        return None;
    }
    let mut parent = arm.parent;
    let mut owner = None;
    for _ in 0..body.nodes().len() {
        let node = body.nodes().get(parent?.0 as usize)?;
        if node.role == NodeRole::Expression(ExpressionKind::Match) {
            owner = Some(node);
            break;
        }
        if node.role != NodeRole::Syntax {
            return None;
        }
        parent = node.parent;
    }
    let owner = owner?;
    let source = body
        .nodes()
        .get(owner.operands.first()?.0 as usize)?
        .value?;
    let destination = owner.runtime_temporaries[0]?;
    let place = SlotId(destination.0.checked_add(1)?);
    match draft.operations.as_slice() {
        [
            Op::ScopeEnter { scope },
            Op::ValuePlace {
                source: actual_source,
                destination: actual_place,
            },
            Op::ReadPlace {
                place: read_place,
                destination: read_destination,
            },
            Op::Pattern {
                operation:
                    super::op::PatternOperation::StageBinding {
                        owner: transaction_owner,
                        binding,
                        source: bound_source,
                        writable: false,
                    },
            },
            ..,
        ] if *scope == arm.scope
            && *actual_source == source
            && *actual_place == place
            && *read_place == place
            && *read_destination == destination
            && *transaction_owner == owner.id
            && *binding == pattern.id
            && *bound_source == destination =>
        {
            Some((source, place, destination))
        }
        _ => None,
    }
}
fn macro_scalar_value(node: &crate::syntax::body_index::StructuralNode, value: SlotId) -> bool {
    node.role
        == crate::syntax::body_index::NodeRole::Expression(
            crate::syntax::body_index::ExpressionKind::Macro,
        )
        && (node.runtime_scalar_slots.contains(&value)
            || node.runtime_temporaries[0] == Some(value))
}
fn macro_scalar_place(
    node: &crate::syntax::body_index::StructuralNode,
    key: &PcKey,
    place: SlotId,
) -> bool {
    matches!(Phase::from_code(key.phase), Phase::Operand(_))
        && place
            .0
            .checked_sub(1)
            .is_some_and(|value| macro_scalar_value(node, SlotId(value)))
}
pub fn verify(draft: RecipeDraft, body: &BodyIndex) -> Result<VerifiedRecipe, Diagnostic> {
    verify_inner(draft, body, None, false, None)
}
/// A genuine actual MatchExpr source plan may reach exhaustion at its reserved
/// opaque operand PC. This does not authorize general opaque MatchNoArm exits.
pub(crate) fn verify_runtime_scalar_match_exhaustion(
    draft: RecipeDraft,
    body: &BodyIndex,
    proof: &crate::syntax::runtime_macro_scalars::RuntimeScalarMatchExhaustion<'_>,
) -> Result<VerifiedRecipe, Diagnostic> {
    proof.require(body, &draft.key)?;
    if !draft.operations.is_empty()
        || !matches!(draft.exits.as_slice(), [RecipeExit::MatchNoArm])
        || &draft.origin.span != proof.span()
    {
        return Err(invalid(
            "opaque match exhaustion recipe differs from source proof",
        ));
    }
    verify_inner(draft, body, None, false, Some(proof))
}
/// General macro call admission consumes a genuine privately minted source
/// witness. The ordinary public verifier never acquires this authority.
pub(crate) fn verify_runtime_scalar_call(
    draft: RecipeDraft,
    body: &BodyIndex,
    call: &crate::syntax::runtime_macro_scalars::RuntimeMacroCall,
) -> Result<VerifiedRecipe, Diagnostic> {
    require_runtime_scalar_call(&draft, body, call)?;
    verify_inner(draft, body, Some(call), false, None)
}
/// Only a freshly source-replayed scalar plan can admit an opaque argument's
/// borrow phases. Public verification retains ordinary indexed borrow rules.
pub(crate) fn verify_runtime_scalar_borrow(
    draft: RecipeDraft,
    body: &BodyIndex,
    plan: &crate::syntax::runtime_macro_scalars::RuntimeScalarPlan,
) -> Result<VerifiedRecipe, Diagnostic> {
    use crate::syntax::runtime_macro_scalars::RuntimeScalarStep;
    pc(body, &draft.key)?;
    let node = &body.nodes()[draft.key.node as usize];
    let (call, borrow) = plan
        .argument_borrow_at(&draft.key)?
        .ok_or_else(|| invalid("scalar borrow phase lacks source witness"))?;
    borrow.require(call, &draft.key.body, borrow.argument_ordinal())?;
    let ordinal = match Phase::from_code(draft.key.phase) {
        Phase::Entry => 0,
        Phase::Operand(n) => usize::from(n) + 1,
        _ => return Err(invalid("scalar borrow phase differs")),
    };
    let Some(RuntimeScalarStep::Operation(expected)) = plan.steps().get(ordinal) else {
        return Err(invalid("scalar borrow source operation absent"));
    };
    let operations = if draft.key.phase == Phase::Entry.code()?
        && let Some(scope) = node.boolean_scope.or(node.statement_scope)
        && matches!(draft.operations.first(), Some(Op::ScopeEnter { scope: actual }) if *actual == scope)
    {
        &draft.operations[1..]
    } else {
        draft.operations.as_slice()
    };
    let next = body.pc(
        node.id,
        Phase::Operand(
            u16::try_from(ordinal).map_err(|_| invalid("scalar borrow continuation overflow"))?,
        ),
    )?;
    if borrow.source().profile != *body.profile()
        || draft.origin.span != body.span(node.id)?
        || operations != std::slice::from_ref(expected)
        || !matches!(draft.exits.as_slice(), [RecipeExit::Next(target)] if target == &next)
        || ordinal
            .checked_add(1)
            .is_none_or(|next| next >= plan.steps().len())
        || !macro_scalar_value(node, borrow.argument_slot())
        || !macro_scalar_place(node, &draft.key, borrow.place())
    {
        return Err(invalid(
            "scalar borrow exact source operation/continuation differs",
        ));
    }
    let matches = match expected {
        Op::SetHint {
            child,
            hint: super::op::HintRecipe::BorrowTarget { reference },
        } => {
            borrow.hint_key() == Some(&draft.key)
                && borrow.temporary() == Some(*child)
                && *reference == borrow.argument_slot()
        }
        Op::ValuePlace {
            source,
            destination,
        } => {
            borrow.place_key() == &draft.key
                && borrow.temporary() == Some(*source)
                && *destination == borrow.place()
        }
        Op::MakePlace { destination, .. } | Op::ParameterPlace { destination, .. } => {
            borrow.place_key() == &draft.key
                && borrow.temporary().is_none()
                && *destination == borrow.place()
        }
        Op::Borrow {
            place,
            access,
            destination,
        } => {
            borrow.borrow_key() == &draft.key
                && *place == borrow.place()
                && *access == borrow.access()
                && *destination == borrow.argument_slot()
        }
        _ => false,
    };
    if !matches {
        return Err(invalid("scalar borrow source role differs"));
    }
    verify_inner(draft, body, None, true, None)
}
fn require_runtime_scalar_call(
    draft: &RecipeDraft,
    body: &BodyIndex,
    call: &crate::syntax::runtime_macro_scalars::RuntimeMacroCall,
) -> Result<(), Diagnostic> {
    use crate::syntax::{
        body_index::{ExpressionKind, NodeRole},
        runtime_macro_scalars::RuntimeMacroCallPhase,
    };
    pc(body, &draft.key)?;
    let node = &body.nodes()[draft.key.node as usize];
    if node.role != NodeRole::Expression(ExpressionKind::Macro)
        || &call.source().profile != body.profile()
        || call.span().body_key != *body.body()
        || draft.origin.span != *call.span()
        || call.span().relative_start < node.range.start
        || call.span().relative_end > node.range.end
        || call.span().relative_start >= call.span().relative_end
        || call.prepare_key().node != draft.key.node
        || draft.exits.len() != 1
        || !matches!(
            Phase::from_code(draft.key.phase),
            Phase::Entry | Phase::Operand(_)
        )
    {
        return Err(invalid("scalar call source/body/phase agreement differs"));
    }
    // Scope entry belongs to the indexed original macro boundary, not a newly
    // invented call scope. Accept only the exact normal entry prefix.
    let ops = if draft.key.phase == Phase::Entry.code()?
        && let Some(scope) = node.boolean_scope.or(node.statement_scope)
        && matches!(draft.operations.first(), Some(Op::ScopeEnter { scope: actual }) if *actual == scope)
    {
        &draft.operations[1..]
    } else {
        draft.operations.as_slice()
    };
    let fields = |site: &super::recipe::CallSiteId,
                  name: &str,
                  explicit: &[String],
                  arguments: &[SlotId]| {
        site.0 == draft.key.node
            && name == call.name()
            && explicit == call.explicit()
            && arguments == call.arguments()
    };
    let next =
        |key: &PcKey| matches!(draft.exits.as_slice(), [RecipeExit::Next(target)] if target == key);
    let seal_exit = || {
        matches!(draft.exits.as_slice(), [RecipeExit::Call { site, resume, result }]
            if site.0 == draft.key.node && *result == call.result() && resume == call.resume_key())
    };
    let count = u32::try_from(call.arguments().len())
        .map_err(|_| invalid("scalar call argument count overflow"))?;
    let matches = match ops {
        [
            Op::CallPrepare {
                site,
                name,
                explicit,
                arguments,
            },
        ] if fields(site, name, explicit, arguments) => {
            call.require(&draft.key.body, &draft.key, RuntimeMacroCallPhase::Prepare)?;
            let target = if count == 0 {
                call.seal_key()
            } else {
                call.argument_key(0)?
            };
            next(target)
        }
        [Op::CallAdvance { site, argument }]
            if site.0 == draft.key.node && argument.checked_add(1).is_some_and(|n| n < count) =>
        {
            call.require(
                &draft.key.body,
                &draft.key,
                RuntimeMacroCallPhase::Advance(*argument),
            )?;
            next(call.argument_key(*argument + 1)?)
        }
        [
            Op::CallAdvance { site, argument },
            Op::CallSeal {
                site: sealed,
                name,
                explicit,
                arguments,
            },
        ] if site.0 == draft.key.node
            && argument.checked_add(1) == Some(count)
            && fields(sealed, name, explicit, arguments) =>
        {
            call.require(
                &draft.key.body,
                &draft.key,
                RuntimeMacroCallPhase::Advance(*argument),
            )?;
            call.require(&draft.key.body, &draft.key, RuntimeMacroCallPhase::Seal)?;
            seal_exit()
        }
        [
            Op::CallSeal {
                site,
                name,
                explicit,
                arguments,
            },
        ] if count == 0 && fields(site, name, explicit, arguments) => {
            call.require(&draft.key.body, &draft.key, RuntimeMacroCallPhase::Seal)?;
            seal_exit()
        }
        _ => false,
    };
    if !matches {
        return Err(invalid(
            "scalar call operations/exit differ from exact source phase",
        ));
    }
    let mut slots = BTreeSet::new();
    for argument in call.arguments() {
        slot(body, *argument, SlotKind::Value)?;
        if !macro_scalar_value(node, *argument)
            || *argument == call.result()
            || !slots.insert(*argument)
        {
            return Err(invalid(
                "scalar call argument slots are not fresh reserved values",
            ));
        }
    }
    slot(body, call.result(), SlotKind::Value)?;
    if !macro_scalar_value(node, call.result()) {
        return Err(invalid("scalar call result is not a reserved value"));
    }
    Ok(())
}
fn verify_inner(
    mut draft: RecipeDraft,
    body: &BodyIndex,
    macro_call: Option<&crate::syntax::runtime_macro_scalars::RuntimeMacroCall>,
    macro_borrow: bool,
    scalar_match: Option<&crate::syntax::runtime_macro_scalars::RuntimeScalarMatchExhaustion<'_>>,
) -> Result<VerifiedRecipe, Diagnostic> {
    pc(body, &draft.key)?;
    span(body, &draft.origin.span)?;
    let node = &body.nodes()[draft.key.node as usize];
    // Only the source-indexed initialized simple ref statement reserves this
    // result. Its initializer is a direct expression operand, not an arbitrary
    // value elsewhere in the frame. Existing expression checks remain distinct.
    let reference_initializer = if node.role == crate::syntax::body_index::NodeRole::Statement
        && node.runtime_temporaries[0].is_some()
        && draft.key.phase == Phase::Commit.code()?
    {
        let initializer = node
            .operands
            .first()
            .and_then(|id| body.nodes().get(id.0 as usize))
            .filter(|initializer| node.operands.len() == 1 && initializer.parent == Some(node.id))
            .ok_or_else(|| invalid("reference statement requires its exact initializer operand"))?;
        Some(initializer)
    } else {
        None
    };
    if draft.origin.span.relative_start < node.range.start
        || draft.origin.span.relative_end > node.range.end
    {
        return Err(invalid("recipe origin outside selected node"));
    }
    // A source phase may only select its next PC. Section 10.4 bounds ordinary
    // operations above, while every phase still has exactly one charged exit.
    if draft.operations.len() > 8 {
        return Err(invalid("recipe permits at most eight ordinary operations"));
    }
    if draft.exits.len() != 1 {
        return Err(invalid("recipe requires exactly one terminal exit"));
    }
    if !draft.origin.expansion_ancestry.is_empty() {
        return Err(invalid(
            "expanded origin requires later validated expanded-body input",
        ));
    }
    let runtime_operations = if draft.key.phase == crate::ir::pc::Phase::Entry.code()?
        && let Some(scope) = node.boolean_scope
        && matches!(draft.operations.first(), Some(Op::ScopeEnter { scope: entered }) if *entered == scope)
    {
        &draft.operations[1..]
    } else {
        draft.operations.as_slice()
    };
    let mut bytes = 512usize;
    charge(
        &mut bytes,
        draft
            .origin
            .expansion_ancestry
            .len()
            .checked_mul(32)
            .ok_or_else(|| invalid("origin size overflow"))?,
    )?;
    let mut outputs = BTreeSet::new();
    let mut pattern_plans = BTreeSet::new();
    let mut shared_product_admission_bytes = 0usize;
    let mut string_literals = BTreeSet::new();
    let mut shared_string_literal_admission_bytes = 0usize;
    pattern_decision::exits(&draft)?;
    for op in &draft.operations {
        charge(&mut bytes, 256usize.max(std::mem::size_of::<Op>()))?;
        let annotation = match op {
            Op::SetHint { hint, .. }
            | Op::DeclareLocal { hint, .. }
            | Op::BindLocal { hint, .. }
            | Op::Literal { hint, .. } => Some(hint),
            _ => None,
        };
        if let Some(super::op::HintRecipe::DefinitionParameter(ordinal)) = annotation
            && *ordinal >= 100_000
        {
            return Err(invalid(
                "definition parameter ordinal exceeds structural limit",
            ));
        }
        if matches!(annotation, Some(super::op::HintRecipe::DefinitionResult))
            && (draft.key.node != 0
                || draft.key.phase != Phase::Entry.code()?
                || !matches!(op, Op::SetHint { child, .. } if body.nodes()[0].value == Some(*child)))
        {
            return Err(invalid(
                "definition result hint requires its exact owning root entry/result",
            ));
        }
        if let Some(super::op::HintRecipe::DefinitionNumeric(context)) = annotation {
            // Structural bound only. The reached VM must still replay the actual
            // current definition/source relation and admitted primitive contract.
            definition_numeric_context(*context)?;
        }
        if let Some(super::op::HintRecipe::BorrowTarget { reference }) = annotation {
            use crate::syntax::body_index::{ExpressionKind, NodeRole};
            let child = match op {
                Op::SetHint { child, .. } => *child,
                _ => return Err(invalid("borrow target hint is not an operand hint")),
            };
            let operand = node
                .operands
                .first()
                .and_then(|id| body.nodes().get(id.0 as usize));
            if !macro_borrow
                && (node.role != NodeRole::Expression(ExpressionKind::Borrow)
                    || draft.key.phase != Phase::Entry.code()?
                    || node.value != Some(*reference)
                    || node.operands.len() != 1
                    || operand.is_none_or(|operand| {
                        operand.value != Some(child) || operand.parent != Some(node.id)
                    })
                    || *reference == child)
            {
                return Err(invalid(
                    "borrow target hint requires its own result and operand",
                ));
            }
            slot(body, *reference, SlotKind::Value)?;
            slot(body, child, SlotKind::Value)?;
        }
        if let Some(super::op::HintRecipe::Parameter(key)) = annotation
            && (key.definition != draft.key.body.definition || key.ordinal >= 100_000)
        {
            return Err(invalid("annotation parameter identity mismatch"));
        }

        if let Some(super::op::HintRecipe::Reference { region, .. }) = annotation {
            use crate::traits::term::RegionTerm;
            if !matches!(region, RegionTerm::Static | RegionTerm::Local { .. })
                || matches!(region, RegionTerm::Local { definition, start, end }
                    if *definition != draft.key.body.definition || start >= end)
            {
                return Err(invalid("reference annotation region identity mismatch"));
            }
        }

        let output = match op {
            Op::Pattern { operation } => {
                pattern_transition(body, &draft.key, operation)?;
                if let super::op::PatternOperation::Decision { plan, .. } = operation
                    && pattern_plans.insert(std::sync::Arc::as_ptr(plan))
                {
                    let shared = plan
                        .metadata_bytes()
                        .checked_add(3 * std::mem::size_of::<usize>())
                        .ok_or_else(|| invalid("decision plan accounting overflow"))?;
                    // Admission still bounds the complete retained aggregate,
                    // even though the memo owner later charges shared payloads
                    // separately from each recipe's exclusive allocation.
                    charge(&mut bytes, shared)?;
                    shared_product_admission_bytes = shared_product_admission_bytes
                        .checked_add(shared)
                        .ok_or_else(|| invalid("decision plan accounting overflow"))?;
                }
                None
            }
            Op::ProjectTuple {
                base, destination, ..
            }
            | Op::ProjectIndex {
                base, destination, ..
            }
            | Op::ProjectField { base, destination } => {
                slot(body, *base, SlotKind::Place)?;
                slot(body, *destination, SlotKind::Place)?;
                if base == destination
                    || node.place != Some(*destination)
                    || node
                        .operands
                        .first()
                        .and_then(|id| body.nodes().get(id.0 as usize))
                        .and_then(|operand| operand.place)
                        != Some(*base)
                {
                    return Err(invalid(
                        "projection requires its base operand and its own place",
                    ));
                }
                if let Op::ProjectIndex { index, .. } = op {
                    slot(body, *index, SlotKind::Value)?;
                    if node
                        .operands
                        .get(1)
                        .and_then(|id| body.nodes().get(id.0 as usize))
                        .and_then(|operand| operand.value)
                        != Some(*index)
                    {
                        return Err(invalid(
                            "index projection requires its evaluated index operand",
                        ));
                    }
                }
                Some(*destination)
            }
            Op::DerefPlace {
                reference,
                destination,
            } => {
                slot(body, *reference, SlotKind::Place)?;
                slot(body, *destination, SlotKind::Place)?;
                let scalar_macro = macro_scalar_place(node, &draft.key, *reference)
                    && macro_scalar_place(node, &draft.key, *destination);
                if reference == destination
                    || (!scalar_macro
                        && (node.place != Some(*destination)
                            || node
                                .operands
                                .first()
                                .and_then(|operand| body.nodes().get(operand.0 as usize))
                                .and_then(|operand| operand.place)
                                != Some(*reference)))
                {
                    return Err(invalid(
                        "dereference requires distinct source and its own place slot",
                    ));
                }
                Some(*destination)
            }
            Op::ReadPlace { place, destination } => {
                slot(body, *place, SlotKind::Place)?;
                slot(body, *destination, SlotKind::Value)?;
                let scalar_macro = macro_scalar_place(node, &draft.key, *place)
                    && macro_scalar_value(node, *destination)
                    && place.0
                        == destination
                            .0
                            .checked_add(1)
                            .ok_or_else(|| invalid("macro place slot overflow"))?;
                let scalar_binding = scalar_binding_copy(body, &draft).is_some_and(
                    |(_, actual_place, actual_destination)| {
                        actual_place == *place && actual_destination == *destination
                    },
                );
                if !scalar_macro
                    && !scalar_binding
                    && !pattern_decision::read_place(body, &draft, *place, *destination)
                    && (node.place != Some(*place) || node.value != Some(*destination))
                {
                    return Err(invalid(
                        "place read requires its own place and result slots",
                    ));
                }
                Some(*destination)
            }
            Op::AssignPlace {
                place,
                source,
                destination,
            } => {
                slot(body, *place, SlotKind::Place)?;
                slot(body, *source, SlotKind::Value)?;
                slot(body, *destination, SlotKind::Value)?;
                if source == destination
                    || node.value != Some(*destination)
                    || node
                        .operands
                        .first()
                        .and_then(|id| body.nodes().get(id.0 as usize))
                        .and_then(|operand| operand.place)
                        != Some(*place)
                    || node
                        .operands
                        .get(1)
                        .and_then(|id| body.nodes().get(id.0 as usize))
                        .and_then(|operand| operand.value)
                        != Some(*source)
                {
                    return Err(invalid(
                        "place assignment requires a separate RHS and its own result",
                    ));
                }
                Some(*destination)
            }
            Op::ValuePlace {
                source,
                destination,
            } => {
                slot(body, *source, SlotKind::Value)?;
                slot(body, *destination, SlotKind::Place)?;
                if macro_borrow
                    || scalar_binding_copy(body, &draft).is_some_and(
                        |(actual_source, actual_place, _)| {
                            actual_source == *source && actual_place == *destination
                        },
                    )
                {
                    Some(*destination)
                } else {
                    let own_path = node.role
                        == crate::syntax::body_index::NodeRole::Expression(
                            crate::syntax::body_index::ExpressionKind::Path,
                        )
                        && draft.key.phase == Phase::Exit.code()?;
                    let operand = if own_path {
                        if !matches!(draft.operations.as_slice(), [Op::ValuePlace { .. }]) {
                            return Err(invalid(
                                "path result place must be its separate indexed exit",
                            ));
                        }
                        Some(node)
                    } else {
                        reference_initializer.or_else(|| {
                            node.operands
                                .first()
                                .and_then(|id| body.nodes().get(id.0 as usize))
                        })
                    }
                    .ok_or_else(|| invalid("temporary place receiver absent"))?;
                    if operand.value != Some(*source)
                        || operand.place != Some(*destination)
                        || (!own_path
                            && reference_initializer.is_none()
                            && draft.key.phase != Phase::Operand(0).code()?)
                    {
                        return Err(invalid(
                            "temporary place requires its executed base operand",
                        ));
                    }
                    Some(*destination)
                }
            }
            Op::PlaceTransfer {
                source,
                destination,
            } => {
                slot(body, *source, SlotKind::Place)?;
                slot(body, *destination, SlotKind::Place)?;
                if source == destination
                    || node.place != Some(*destination)
                    || node
                        .operands
                        .first()
                        .and_then(|id| body.nodes().get(id.0 as usize))
                        .and_then(|operand| operand.place)
                        != Some(*source)
                    || draft.key.phase != Phase::Operand(0).code()?
                {
                    return Err(invalid("place transfer requires its base and own place"));
                }
                Some(*destination)
            }
            Op::ParameterPlace {
                ordinal,
                destination,
            } => {
                slot(body, *destination, SlotKind::Place)?;
                if *ordinal >= 100_000
                    || (!macro_borrow
                        && (node.role
                            != crate::syntax::body_index::NodeRole::Expression(
                                crate::syntax::body_index::ExpressionKind::Path,
                            )
                            || node.place != Some(*destination)
                            || draft.key.phase != Phase::Entry.code()?))
                {
                    return Err(invalid(
                        "parameter place requires its original path and place slot",
                    ));
                }
                Some(*destination)
            }
            Op::MakePlace {
                binding,
                destination,
            } => {
                slot(body, *destination, SlotKind::Place)?;
                if (node.place != Some(*destination)
                    && !macro_scalar_place(node, &draft.key, *destination))
                    || body
                        .nodes()
                        .get(binding.0 as usize)
                        .is_none_or(|n| n.role != crate::syntax::body_index::NodeRole::Pattern)
                {
                    return Err(invalid(
                        "local place requires its own place slot and lexical binding",
                    ));
                }
                Some(*destination)
            }
            Op::Borrow {
                place, destination, ..
            } => {
                slot(body, *place, SlotKind::Place)?;
                slot(body, *destination, SlotKind::Value)?;
                let reference_statement = reference_initializer.is_some_and(|initializer| {
                    initializer.place == Some(*place)
                        && node.runtime_temporaries[0] == Some(*destination)
                });
                if !macro_borrow
                    && !reference_statement
                    && ((node.place != Some(*place)
                        && node
                            .operands
                            .first()
                            .and_then(|operand| body.nodes().get(operand.0 as usize))
                            .and_then(|operand| operand.place)
                            != Some(*place))
                        || node.value != Some(*destination))
                {
                    return Err(invalid("borrow requires its own place and result slots"));
                }
                Some(*destination)
            }
            Op::ConstructStart { guard } => {
                slot(body, *guard, SlotKind::Guard)?;
                if node.constructor_guard != Some(*guard)
                    || draft.key.phase != Phase::Entry.code()?
                {
                    return Err(invalid("constructor start guard/source phase mismatch"));
                }
                Some(*guard)
            }
            Op::ConstructField {
                guard,
                field,
                source,
            } => {
                slot(body, *guard, SlotKind::Guard)?;
                slot(body, *source, SlotKind::Value)?;
                let operand_matches = match Phase::from_code(draft.key.phase) {
                    Phase::Operand(ordinal) => node
                        .operands
                        .get(usize::from(ordinal))
                        .and_then(|id| body.nodes().get(id.0 as usize))
                        .is_some_and(|operand| operand.value == Some(*source)),
                    _ => false,
                };
                if node.constructor_guard != Some(*guard) || *field >= 100_000 || !operand_matches {
                    return Err(invalid("constructor field guard/source identity mismatch"));
                }
                None
            }
            Op::ConstructFinish { guard, destination } => {
                slot(body, *guard, SlotKind::Guard)?;
                slot(body, *destination, SlotKind::Value)?;
                if node.constructor_guard != Some(*guard)
                    || node.value != Some(*destination)
                    || draft.key.phase != Phase::Commit.code()?
                {
                    return Err(invalid("constructor finish guard/result identity mismatch"));
                }
                Some(*destination)
            }
            Op::SourceConstant {
                constant,
                destination,
            } => {
                use crate::syntax::body_index::{ExpressionKind, NodeRole};
                slot(body, *destination, SlotKind::Value)?;
                let operations = match draft.operations.as_slice() {
                    [Op::ScopeEnter { scope }, tail @ ..]
                        if Some(*scope) == node.boolean_scope.or(node.statement_scope) =>
                    {
                        tail
                    }
                    ops => ops,
                };
                if draft.key.phase != Phase::Entry.code()?
                    || node.role != NodeRole::Expression(ExpressionKind::Path)
                    || node.value != Some(*destination)
                    || !matches!(operations, [Op::SourceConstant { .. }])
                    || !constant.matches(body, &draft, *destination)?
                {
                    return Err(invalid(
                        "source constant instruction PC/path/result/exit/dependencies differs",
                    ));
                }
                charge(&mut bytes, constant.metadata_bytes())?;
                Some(*destination)
            }
            Op::PrimitiveFloatingConstant {
                constant,
                destination,
            } => {
                use crate::syntax::body_index::{ExpressionKind, NodeRole};
                slot(body, *destination, SlotKind::Value)?;
                let operations = match draft.operations.as_slice() {
                    [Op::ScopeEnter { scope }, operations @ ..]
                        if Some(*scope) == node.boolean_scope.or(node.statement_scope) =>
                    {
                        operations
                    }
                    operations => operations,
                };
                if draft.key.phase != Phase::Entry.code()?
                    || node.role != NodeRole::Expression(ExpressionKind::Path)
                    || node.value != Some(*destination)
                    || body.span(node.id)? != draft.origin.span
                    || !matches!(operations, [Op::PrimitiveFloatingConstant { .. }])
                    || !constant.matches(body, &draft, *destination)?
                {
                    return Err(invalid(
                        "floating constant proof PC/path/span/destination/exit/dependencies differs",
                    ));
                }
                charge(
                    &mut bytes,
                    constant
                        .metadata_bytes()
                        .ok_or_else(|| invalid("floating instruction metadata overflow"))?,
                )?;
                Some(*destination)
            }
            Op::Literal {
                literal,
                destination,
                hint,
            } => {
                slot(body, *destination, SlotKind::Value)?;
                span(body, &literal.source)?;
                if literal.spelling.is_empty() {
                    return Err(invalid("literal recipe has no source spelling"));
                }
                use super::op::{DecodedLiteral, LiteralKind, PrimitiveType};
                match (&literal.kind, &literal.decoded) {
                    (
                        kind @ (LiteralKind::Integer | LiteralKind::NegativeInteger),
                        Some(
                            DecodedLiteral::Integer {
                                negative,
                                radix,
                                suffix,
                                ..
                            }
                            | DecodedLiteral::InvalidInteger {
                                negative,
                                radix,
                                suffix,
                                ..
                            },
                        ),
                    ) => {
                        if *negative != (*kind == LiteralKind::NegativeInteger)
                            || !matches!(radix, 2 | 8 | 10 | 16)
                            || suffix.is_some_and(|ty| {
                                !matches!(
                                    ty,
                                    PrimitiveType::I8
                                        | PrimitiveType::I16
                                        | PrimitiveType::I32
                                        | PrimitiveType::I64
                                        | PrimitiveType::I128
                                        | PrimitiveType::Isize
                                        | PrimitiveType::U8
                                        | PrimitiveType::U16
                                        | PrimitiveType::U32
                                        | PrimitiveType::U64
                                        | PrimitiveType::U128
                                        | PrimitiveType::Usize
                                )
                            })
                        {
                            return Err(invalid("invalid decoded integer metadata"));
                        }
                        if matches!(
                            &literal.decoded,
                            Some(DecodedLiteral::InvalidInteger {
                                suffix: Some(_),
                                reason: super::op::IntegerLexicalError::InvalidSuffix
                                    | super::op::IntegerLexicalError::UnsupportedWidth,
                                ..
                            })
                        ) {
                            return Err(invalid("invalid integer suffix fault metadata"));
                        }
                    }
                    (LiteralKind::Integer | LiteralKind::NegativeInteger, _) => {
                        return Err(invalid(
                            "integer literal requires matched pinned decoded data",
                        ));
                    }
                    (LiteralKind::String, Some(DecodedLiteral::String(proof))) => {
                        if *hint != super::op::HintRecipe::None {
                            return Err(invalid(
                                "string literal recipe cannot supply scalar/type modifiers",
                            ));
                        }
                        proof.require_recipe(body, &draft.key, literal, *destination)?;
                        if string_literals.insert(std::sync::Arc::as_ptr(proof)) {
                            let shared = proof
                                .metadata_bytes()
                                .ok_or_else(|| invalid("string literal metadata overflow"))?;
                            charge(&mut bytes, shared)?;
                            shared_string_literal_admission_bytes =
                                shared_string_literal_admission_bytes
                                    .checked_add(shared)
                                    .ok_or_else(|| {
                                        invalid("string literal aggregate accounting overflow")
                                    })?;
                        }
                    }
                    (LiteralKind::String, _) => {
                        return Err(invalid(
                            "string literal requires actual indexed source receipt",
                        ));
                    }
                    (LiteralKind::Char, Some(DecodedLiteral::Char(_)))
                    | (LiteralKind::Byte, Some(DecodedLiteral::Byte(_))) => {}
                    (LiteralKind::Float, Some(DecodedLiteral::Float { decimal, suffix })) => {
                        if decimal.is_empty()
                            || suffix.is_some_and(|ty| {
                                !matches!(ty, PrimitiveType::F32 | PrimitiveType::F64)
                            })
                        {
                            return Err(invalid("invalid decoded float metadata"));
                        }
                        charge(&mut bytes, decimal.len())?;
                    }
                    (LiteralKind::Char | LiteralKind::Byte | LiteralKind::Float, _) => {
                        return Err(invalid("literal requires matched pinned decoded data"));
                    }
                    (_, None) => {}
                    _ => return Err(invalid("literal kind/decoded metadata mismatch")),
                }
                charge(&mut bytes, literal.spelling.len())?;
                Some(*destination)
            }
            Op::Unit { destination } => {
                slot(body, *destination, SlotKind::Value)?;
                Some(*destination)
            }
            Op::SetHint { child, hint } => {
                if let super::op::HintRecipe::Parameter(key) = hint
                    && (key.definition != draft.key.body.definition || key.ordinal >= 100_000)
                {
                    return Err(invalid("hint parameter identity mismatch"));
                }
                slot(body, *child, SlotKind::Value)?;
                None
            }
            Op::ResultTransfer {
                source,
                destination,
            } => {
                slot(body, *source, SlotKind::Value)?;
                slot(body, *destination, SlotKind::Value)?;
                if source == destination {
                    return Err(invalid("result transfer aliases its source"));
                }
                Some(*destination)
            }
            Op::ScopeEnter { scope } | Op::ScopeLeave { scope } => {
                if scope.0 as usize >= body.scopes().len() {
                    return Err(invalid("recipe scope absent"));
                }
                None
            }
            Op::Assert {
                condition,
                destination,
                message,
            } => {
                if message.capacity() > 65_536
                    || message.len() > 65_536
                    || node.role
                        != crate::syntax::body_index::NodeRole::Expression(
                            crate::syntax::body_index::ExpressionKind::Macro,
                        )
                    || condition == destination
                {
                    return Err(invalid(
                        "assertion requires distinct slots, macro origin and bounded owned message",
                    ));
                }
                slot(body, *condition, SlotKind::Value)?;
                slot(body, *destination, SlotKind::Value)?;
                charge(&mut bytes, message.capacity())?;
                Some(*destination)
            }
            Op::Panic { message } => {
                if message.len() > 65_536
                    || message.capacity() > 65_536
                    || node.role
                        != crate::syntax::body_index::NodeRole::Expression(
                            crate::syntax::body_index::ExpressionKind::Macro,
                        )
                {
                    return Err(invalid(
                        "explicit panic requires a macro origin and bounded owned message",
                    ));
                }
                charge(&mut bytes, message.capacity())?;
                None
            }
            Op::Declaration => {
                if node.role != crate::syntax::body_index::NodeRole::OpaqueItem
                    || draft.key.phase != 0
                {
                    return Err(invalid(
                        "declaration marker requires its entered opaque item",
                    ));
                }
                None
            }
            Op::CheckUnit { source } => {
                slot(body, *source, SlotKind::Value)?;
                None
            }
            Op::BranchHint {
                parent,
                condition,
                yes,
                no,
                ..
            } => {
                slot(body, *parent, SlotKind::Value)?;
                slot(body, *condition, SlotKind::Value)?;
                for child in [yes, no].into_iter().flatten() {
                    slot(body, *child, SlotKind::Value)?;
                    if child == condition || child == parent {
                        return Err(invalid("selected branch hint aliases evaluated storage"));
                    }
                }
                None
            }
            Op::DeclareLocal { binding, .. }
            | Op::LocalHint { binding, .. }
            | Op::CompoundHint { binding, .. }
            | Op::AssignLocal { binding, .. } => {
                if body
                    .nodes()
                    .get(binding.0 as usize)
                    .is_none_or(|node| node.role != crate::syntax::body_index::NodeRole::Pattern)
                {
                    return Err(invalid("local operation lacks pattern identity"));
                }
                match op {
                    Op::CompoundHint { child, kind, .. } => {
                        match kind {
                            super::op::AssignKind::Primitive { left, right } => {
                                crate::runtime::shape::PrimitiveIdentity::from_recipe(*left)?;
                                crate::runtime::shape::PrimitiveIdentity::from_recipe(*right)?;
                            }
                            super::op::AssignKind::DefinitionNumeric { context, right } => {
                                definition_numeric_context(*context)?;
                                if let Some(right) = right {
                                    numeric_primitive(*right)?;
                                }
                            }
                        }
                        slot(body, *child, SlotKind::Value)?;
                        if draft.key.phase != Phase::Entry.code()?
                            || node.runtime_temporaries[0].is_none()
                            || node.operands.len() != 2
                            || body.nodes()[node.operands[1].0 as usize].value != Some(*child)
                        {
                            return Err(invalid(
                                "compound kind requires its source-indexed entry and RHS",
                            ));
                        }
                        None
                    }
                    Op::LocalHint { child, .. } => {
                        slot(body, *child, SlotKind::Value)?;
                        None
                    }
                    Op::AssignLocal {
                        source,
                        destination,
                        ..
                    } => {
                        slot(body, *source, SlotKind::Value)?;
                        slot(body, *destination, SlotKind::Value)?;
                        if source == destination {
                            return Err(invalid("assignment aliases its result"));
                        }
                        Some(*destination)
                    }
                    _ => None,
                }
            }
            Op::BindLocal {
                binding, source, ..
            } => {
                if body
                    .nodes()
                    .get(binding.0 as usize)
                    .is_none_or(|node| node.role != crate::syntax::body_index::NodeRole::Pattern)
                {
                    return Err(invalid("local binding is not a pattern location"));
                }
                slot(body, *source, SlotKind::Value)?;
                None
            }
            Op::ReadLocal {
                binding,
                destination,
            } => {
                if body
                    .nodes()
                    .get(binding.0 as usize)
                    .is_none_or(|node| node.role != crate::syntax::body_index::NodeRole::Pattern)
                {
                    return Err(invalid("local read lacks a pattern binding location"));
                }
                slot(body, *destination, SlotKind::Value)?;
                Some(*destination)
            }
            Op::Discard { source } => {
                slot(body, *source, SlotKind::Value)?;
                None
            }
            Op::NumericBinary {
                left,
                right,
                destination,
                ..
            } => {
                slot(body, *left, SlotKind::Value)?;
                slot(body, *right, SlotKind::Value)?;
                slot(body, *destination, SlotKind::Value)?;
                if destination == left || destination == right {
                    return Err(invalid("numeric output aliases an operand"));
                }
                Some(*destination)
            }
            Op::NumericUnary {
                source,
                destination,
                ..
            }
            | Op::PrimitiveCast {
                source,
                destination,
                ..
            } => {
                slot(body, *source, SlotKind::Value)?;
                slot(body, *destination, SlotKind::Value)?;
                if source == destination {
                    return Err(invalid("numeric output aliases an operand"));
                }
                Some(*destination)
            }
            Op::NumericHint {
                parent,
                child,
                left,
                ..
            } => {
                slot(body, *parent, SlotKind::Value)?;
                slot(body, *child, SlotKind::Value)?;
                if let Some(left) = left {
                    slot(body, *left, SlotKind::Value)?;
                }
                None
            }
            Op::CallAdvance { site, argument } => {
                if site.0 != draft.key.node
                    || *argument >= 100_000
                    || (node.role
                        == crate::syntax::body_index::NodeRole::Expression(
                            crate::syntax::body_index::ExpressionKind::Macro,
                        )
                        && macro_call.is_none())
                {
                    return Err(invalid("call argument progress identity invalid"));
                }
                None
            }
            Op::CheckParameters { count } => {
                if draft.key.node != 0
                    || draft.key.phase != Phase::Entry.code()?
                    || *count > 100_000
                {
                    return Err(invalid("parameter guard requires bounded function entry"));
                }
                None
            }
            Op::ReadParameter {
                ordinal,
                destination,
            } => {
                if *ordinal >= 100_000 {
                    return Err(invalid("parameter ordinal exceeds budget"));
                }
                slot(body, *destination, SlotKind::Value)?;
                Some(*destination)
            }
            Op::CallSeal {
                site,
                name,
                explicit,
                arguments,
            }
            | Op::CallPrepare {
                site,
                name,
                explicit,
                arguments,
            } => {
                let macro_predicate = node.role
                    == crate::syntax::body_index::NodeRole::Expression(
                        crate::syntax::body_index::ExpressionKind::Macro,
                    )
                    && draft.key.phase == 0
                    && arguments.is_empty()
                    && explicit.is_empty()
                    && matches!(op, Op::CallSeal { .. })
                    && matches!(draft.exits.first(), Some(RecipeExit::Call { site: exit_site, resume, result })
                        if *exit_site == *site && Some(*result) == node.runtime_temporaries[0]
                            && resume.body == draft.key.body && resume.node == draft.key.node && resume.phase == 3)
                    && matches!(runtime_operations, [Op::SetHint { child,
                        hint: super::op::HintRecipe::Primitive(super::op::PrimitiveType::Bool) }, Op::CallSeal { .. }]
                        if Some(*child) == node.runtime_temporaries[0]);
                if site.0 != draft.key.node
                    || name.is_empty()
                    || name.len() > 4096
                    || (!macro_predicate
                        && macro_call.is_none()
                        && node.role
                            != crate::syntax::body_index::NodeRole::Expression(
                                crate::syntax::body_index::ExpressionKind::Call,
                            ))
                {
                    return Err(invalid(
                        "call seal requires its selected call node and bounded name",
                    ));
                }
                charge(&mut bytes, name.len())?;
                if explicit.len() > 100_000 {
                    return Err(invalid("explicit generic count exceeds budget"));
                }
                for ty in explicit {
                    if ty.is_empty() {
                        return Err(invalid("empty explicit generic syntax"));
                    }
                    charge(&mut bytes, ty.len())?;
                }

                if arguments.len() > 100_000 {
                    return Err(invalid("call argument count exceeds budget"));
                }
                charge(
                    &mut bytes,
                    arguments
                        .len()
                        .checked_mul(4)
                        .ok_or_else(|| invalid("call argument size overflow"))?,
                )?;
                for argument in arguments {
                    slot(body, *argument, SlotKind::Value)?;
                }

                None
            }
            Op::Unsupported(d) => {
                if d.message.is_empty() {
                    return Err(invalid("unsupported recipe lacks a diagnostic"));
                }
                charge(&mut bytes, d.message.len())?;
                None
            }
        };
        if let Some(output) = output
            && !outputs.insert(output)
        {
            return Err(invalid("recipe writes an output more than once"));
        }
    }
    let exit = draft.exits.pop().expect("one checked terminal exit");
    if let RecipeExit::PatternNext { owner } = &exit {
        pattern_decision::exit(&draft, *owner)?;
    }
    let seals: Vec<_> = draft
        .operations
        .iter()
        .filter_map(|op| match op {
            Op::CallSeal { site, .. } => Some(*site),
            _ => None,
        })
        .collect();
    if !seals.is_empty()
        && (seals.len() != 1
            || !matches!(&exit, RecipeExit::Call { site, .. } if *site == seals[0]))
    {
        return Err(invalid("call seal requires one matching Call exit"));
    }
    match &exit {
        RecipeExit::PatternNext { .. } => {}
        RecipeExit::PatternRetry { owner } => {
            use crate::syntax::body_index::{ExpressionKind, NodeRole};
            if draft.key.phase != Phase::Exit.code()?
                || node.role != NodeRole::MatchArm
                || node.guard_scope.is_none()
                || body
                    .nodes()
                    .get(owner.0 as usize)
                    .is_none_or(|owner| owner.role != NodeRole::Expression(ExpressionKind::Match))
                || !pattern_descendant(body, node.id, *owner)
            {
                return Err(invalid(
                    "pattern retry requires its owning ordinary guard-failure exit",
                ));
            }
        }
        RecipeExit::Call {
            site,
            resume,
            result,
        } => {
            if body
                .nodes()
                .get(site.0 as usize)
                .is_none_or(|node| node.value.is_none())
            {
                return Err(invalid("call site expression absent"));
            }
            pc(body, resume)?;
            slot(body, *result, SlotKind::Value)?;
        }
        RecipeExit::Effect {
            site,
            resume,
            result,
        } => {
            if body
                .nodes()
                .get(site.0 as usize)
                .is_none_or(|node| node.value.is_none())
            {
                return Err(invalid("effect site expression absent"));
            }
            pc(body, resume)?;
            slot(body, *result, SlotKind::Value)?;
        }
        RecipeExit::Next(target) => pc(body, target)?,
        RecipeExit::Backedge(target) => {
            pc(body, target)?;
            if target.phase != crate::ir::pc::Phase::Entry.code()?
                || !matches!(
                    body.nodes()[target.node as usize].role,
                    crate::syntax::body_index::NodeRole::Expression(
                        crate::syntax::body_index::ExpressionKind::While
                            | crate::syntax::body_index::ExpressionKind::Loop
                    )
                )
            {
                return Err(invalid("backedge must target an indexed loop entry"));
            }
        }
        RecipeExit::Branch { condition, yes, no } => {
            slot(body, *condition, SlotKind::Value)?;
            pc(body, yes)?;
            pc(body, no)?;
        }
        RecipeExit::Control(control) => match control {
            ControlRecipe::Return { value } => slot(body, *value, SlotKind::Value)?,
            ControlRecipe::Break {
                target,
                value,
                resume,
            } => {
                if target.0 as usize >= body.labels().len() {
                    return Err(invalid("break target absent"));
                }
                lexical_target(body, &draft.key, *target)?;
                pc(body, resume)?;
                if let Some(v) = value {
                    slot(body, *v, SlotKind::Value)?;
                }
            }
            ControlRecipe::Continue { target } => {
                if body
                    .labels()
                    .get(target.0 as usize)
                    .is_none_or(|label| !label.is_loop)
                {
                    return Err(invalid("continue target is absent or not a loop"));
                }
                lexical_target(body, &draft.key, *target)?;
            }
        },
        RecipeExit::Fault(d) => {
            if d.message.is_empty() {
                return Err(invalid("fault exit lacks a diagnostic"));
            }
            charge(&mut bytes, d.message.len())?;
        }
        RecipeExit::MatchNoArm => {
            if let Some(proof) = scalar_match {
                proof.require(body, &draft.key)?;
                if !draft.operations.is_empty() || &draft.origin.span != proof.span() {
                    return Err(invalid("opaque match exhaustion source shape differs"));
                }
            } else if !draft.operations.is_empty()
                || draft.key.phase != Phase::Exit.code()?
                || node.role
                    != crate::syntax::body_index::NodeRole::Expression(
                        crate::syntax::body_index::ExpressionKind::Match,
                    )
                || draft.origin.span != body.span(node.id)?
            {
                return Err(invalid(
                    "match exhaustion requires a zero-op exact Match exit origin",
                ));
            }
        }
    }
    for dependency in &draft.dependencies {
        charge(&mut bytes, 128)?;
        charge(&mut bytes, dependency.heap_bytes())?;
    }
    draft.dependencies.sort_unstable();
    if draft.dependencies.windows(2).any(|d| d[0].same_key(&d[1])) {
        return Err(invalid("duplicate recipe dependency key"));
    }
    Ok(VerifiedRecipe {
        recipe: Recipe {
            key: draft.key,
            origin: draft.origin,
            operations: draft.operations.into_boxed_slice(),
            exit,
            dependencies: draft.dependencies.into_boxed_slice(),
        },
        metadata_bytes: bytes,
        shared_product_admission_bytes,
        shared_string_literal_admission_bytes,
        namespace_witness: None,
    })
}
/// Exact reached declarative literal admission. The private witness is created
/// only by decoding the immutable published expansion through pinned syntax.
/// Ordinary callers retain verify's rejection of unbound expanded ancestry.
pub(crate) fn verify_declarative_literal(
    mut draft: RecipeDraft,
    body: &BodyIndex,
    output: &crate::syntax::generated_expression::GeneratedLiteral,
) -> Result<VerifiedRecipe, Diagnostic> {
    pc(body, &draft.key)?;
    let node = &body.nodes()[draft.key.node as usize];
    if draft.key.phase != Phase::Entry.code()?
        || node.role
            != crate::syntax::body_index::NodeRole::Expression(
                crate::syntax::body_index::ExpressionKind::Macro,
            )
        || draft.origin.span != body.span(node.id)?
        || output.caller() != &draft.key
        || output.literal().source != draft.origin.span
        || draft.origin.expansion_ancestry.as_slice() != [output.expansion()]
        || draft
            .dependencies
            .iter()
            .filter(|dependency| dependency.same_key(output.dependency()))
            .count()
            != 1
        || !draft.dependencies.contains(output.dependency())
    {
        return Err(invalid(
            "declarative literal witness PC/span/record/ancestry differs",
        ));
    }
    let operations = match draft.operations.as_slice() {
        [Op::ScopeEnter { scope }, rest @ ..]
            if Some(*scope) == node.boolean_scope.or(node.statement_scope) =>
        {
            rest
        }
        operations => operations,
    };
    if !matches!(operations, [Op::Literal { literal, hint: super::op::HintRecipe::None, destination }] if literal == output.literal() && Some(*destination) == node.value)
    {
        return Err(invalid(
            "declarative literal recipe differs from decoded output",
        ));
    }
    // The existing verifier accounts all operations and dependencies. Only the
    // privately bound ancestry is restored afterward, with its capacity charged.
    let ancestry = std::mem::take(&mut draft.origin.expansion_ancestry);
    let ancestry_bytes = ancestry
        .capacity()
        .checked_mul(32)
        .ok_or_else(|| invalid("declarative ancestry size overflow"))?;
    let mut verified = verify(draft, body)?;
    charge(&mut verified.metadata_bytes, ancestry_bytes)?;
    verified.recipe.origin.expansion_ancestry = ancestry;
    Ok(verified)
}
/// Closed generated break admission. The syntax witness grants no label
/// authority: the frontend certificate must bind the actual current caller,
/// local hygiene, source target/continuation, and all preparation dependencies.
pub(crate) fn verify_generated_break(
    mut draft: RecipeDraft,
    body: &BodyIndex,
    output: &crate::syntax::generated_control::GeneratedBreak,
    admission: &crate::frontend::runtime_macro_plan::GeneratedBreakAdmission,
) -> Result<VerifiedRecipe, Diagnostic> {
    admission.require(body, output)?;
    pc(body, &draft.key)?;
    let node = &body.nodes()[draft.key.node as usize];
    if draft.key.phase != Phase::Entry.code()?
        || node.role
            != crate::syntax::body_index::NodeRole::Expression(
                crate::syntax::body_index::ExpressionKind::Macro,
            )
        || output.caller() != &draft.key
        || admission.caller() != &draft.key
        || output.span() != &draft.origin.span
        || draft.origin.span != body.span(node.id)?
        || output.expansion() != admission.expansion()
        || draft.origin.expansion_ancestry.as_slice() != [output.expansion()]
        || node.value != Some(admission.destination())
        || body
            .labels()
            .get(admission.target().0 as usize)
            .is_none_or(|label| label.node != admission.target_node())
        || body
            .nodes()
            .get(admission.target_node().0 as usize)
            .is_none_or(|target| target.value != Some(admission.target_result()))
    {
        return Err(invalid(
            "generated break caller/source/target certificate differs",
        ));
    }
    for required in admission
        .required_dependencies()
        .iter()
        .chain([output.dependency()])
    {
        if draft
            .dependencies
            .iter()
            .filter(|actual| actual.same_key(required))
            .count()
            != 1
            || !draft.dependencies.contains(required)
        {
            return Err(invalid(
                "generated break required dependency absent or conflicting",
            ));
        }
    }
    if output.dependency() != admission.dependency() {
        return Err(invalid("generated break expansion dependency differs"));
    }
    let operations = match draft.operations.as_slice() {
        [Op::ScopeEnter { scope }, rest @ ..]
            if Some(*scope) == node.boolean_scope.or(node.statement_scope) =>
        {
            rest
        }
        operations => operations,
    };
    let matches = if let Some(payload) = output.payload() {
        payload.source == draft.origin.span
            && matches!(operations, [
                Op::NumericHint { parent, child, left: None, operator: None },
                Op::Literal { literal, hint: super::op::HintRecipe::None, destination },
            ] if *parent == admission.target_result()
                && *child == admission.destination()
                && *destination == admission.destination()
                && literal == payload)
    } else {
        matches!(operations, [Op::Unit { destination }] if *destination == admission.destination())
    };
    if !matches
        || !matches!(draft.exits.as_slice(), [RecipeExit::Control(ControlRecipe::Break { target, value: Some(value), resume })]
        if *target == admission.target() && *value == admission.destination() && resume == admission.resume())
    {
        return Err(invalid(
            "generated break operations/exit differ from source certificate",
        ));
    }
    // Ordinary verify still rejects expanded recipes. Remove only the privately
    // bound ancestry, then restore it with the same checked retained-byte charge.
    let ancestry = std::mem::take(&mut draft.origin.expansion_ancestry);
    let ancestry_bytes = ancestry
        .capacity()
        .checked_mul(32)
        .ok_or_else(|| invalid("generated break ancestry size overflow"))?;
    let mut verified = verify(draft, body)?;
    charge(&mut verified.metadata_bytes, ancestry_bytes)?;
    verified.recipe.origin.expansion_ancestry = ancestry;
    Ok(verified)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recipe_metadata_charge_is_checked_and_atomic() {
        let mut n = 64 * 1024 * 1024;
        assert!(charge(&mut n, 1).is_err());
        assert_eq!(n, 64 * 1024 * 1024);
        let mut n = usize::MAX;
        assert!(charge(&mut n, 1).is_err());
        assert_eq!(n, usize::MAX);
    }
    #[test]
    fn unchanged_reference_macro_borrow_phases_require_exact_source_plan_all_editions() {
        use crate::{
            core::{digest::Digest, profile::ProfileIdentity},
            facade::macros::RuntimeMacroKind,
            source::snapshot::{CfgSet, SourceBlob},
            syntax::{
                parse::ParsedFile, runtime_macro_scalars::RuntimeScalarStep, spans::SourceRange,
            },
        };
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/regions/regions-infer-call.rs"
        );
        for edition in [2015, 2018, 2021, 2024] {
            let parsed = ParsedFile::parse(
                &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
                ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            let outline = parsed
                .outline(Digest([191; 32]), vec![], &CfgSet::guest_default())
                .unwrap();
            let handle = outline
                .items
                .iter()
                .find(|i| i.def_key.declared_name == "main")
                .unwrap()
                .body
                .as_ref()
                .unwrap();
            let index = parsed.index_body(handle, Default::default()).unwrap();
            let node = index
                .nodes()
                .iter()
                .find(|n| {
                    n.role
                        == crate::syntax::body_index::NodeRole::Expression(
                            crate::syntax::body_index::ExpressionKind::Macro,
                        )
                })
                .unwrap();
            let entry = index.pc(node.id, Phase::Entry).unwrap();
            let invocation = parsed
                .macro_call_at(
                    SourceRange::checked(
                        handle.range.start + node.range.start,
                        handle.range.start + node.range.end,
                    )
                    .unwrap(),
                )
                .unwrap();
            let arguments = parsed
                .runtime_macro_arguments(&invocation, RuntimeMacroKind::AssertEq)
                .unwrap();
            let plan = parsed
                .runtime_macro_scalar_plan(handle, &index, &entry, &arguments)
                .unwrap();
            let mut count = 0;
            for (ordinal, step) in plan.steps().iter().enumerate() {
                let key = if ordinal == 0 {
                    entry.clone()
                } else {
                    index
                        .pc(node.id, Phase::Operand((ordinal - 1).try_into().unwrap()))
                        .unwrap()
                };
                if plan.argument_borrow_at(&key).unwrap().is_none() {
                    continue;
                }
                let RuntimeScalarStep::Operation(op) = step else {
                    panic!("source phase is not an operation")
                };
                let draft = RecipeDraft {
                    key: key.clone(),
                    origin: super::super::recipe::Origin {
                        span: index.span(node.id).unwrap(),
                        expansion_ancestry: vec![],
                    },
                    operations: vec![op.clone()],
                    exits: vec![RecipeExit::Next(
                        index
                            .pc(node.id, Phase::Operand(ordinal.try_into().unwrap()))
                            .unwrap(),
                    )],
                    dependencies: vec![],
                };
                verify_runtime_scalar_borrow(draft.clone(), &index, &plan).unwrap();
                assert!(verify(draft.clone(), &index).is_err());
                let mut extra = draft.clone();
                extra.operations.push(Op::Declaration);
                assert!(verify_runtime_scalar_borrow(extra, &index, &plan).is_err());
                let mut wrong_exit = draft.clone();
                wrong_exit.exits[0] = RecipeExit::Next(entry.clone());
                assert!(verify_runtime_scalar_borrow(wrong_exit, &index, &plan).is_err());
                let mut wrong = draft.clone();
                match &mut wrong.operations[0] {
                    Op::SetHint {
                        hint: super::super::op::HintRecipe::BorrowTarget { reference },
                        ..
                    } => *reference = SlotId(0),
                    Op::ValuePlace { destination, .. } | Op::Borrow { destination, .. } => {
                        *destination = SlotId(0)
                    }
                    _ => panic!("original temporary borrow phase differs"),
                }
                assert!(verify_runtime_scalar_borrow(wrong, &index, &plan).is_err());
                count += 1;
            }
            assert_eq!(count, 6);
        }
    }
}
