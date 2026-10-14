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
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-IR", message.into())
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
pub fn verify(mut draft: RecipeDraft, body: &BodyIndex) -> Result<VerifiedRecipe, Diagnostic> {
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
    for op in &draft.operations {
        charge(&mut bytes, 256usize.max(std::mem::size_of::<Op>()))?;
        let annotation = match op {
            Op::SetHint { hint, .. }
            | Op::DeclareLocal { hint, .. }
            | Op::BindLocal { hint, .. }
            | Op::Literal { hint, .. } => Some(hint),
            _ => None,
        };
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
            if node.role != NodeRole::Expression(ExpressionKind::Borrow)
                || draft.key.phase != Phase::Entry.code()?
                || node.value != Some(*reference)
                || node.operands.len() != 1
                || operand.is_none_or(|operand| {
                    operand.value != Some(child) || operand.parent != Some(node.id)
                })
                || *reference == child
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
                if !scalar_macro && (node.place != Some(*place) || node.value != Some(*destination))
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
                if !reference_statement
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
                ..
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
                        let super::op::AssignKind::Primitive { left, right } = kind;
                        crate::runtime::shape::PrimitiveIdentity::from_recipe(*left)?;
                        crate::runtime::shape::PrimitiveIdentity::from_recipe(*right)?;
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
                if site.0 != draft.key.node || *argument >= 100_000 {
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
            if !draft.operations.is_empty()
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
}
