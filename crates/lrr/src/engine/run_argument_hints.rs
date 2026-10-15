//! Source hints are demanded at the next argument boundary, never from a later
//! argument whose evaluation might be prevented by an earlier panic.
use super::*;

pub(super) fn apply_context(
    hint: &mut ArgumentHint,
    callee: &SourceCallable,
    explicit: &[(u32, ArgumentType)],
    result: Option<&ArgumentType>,
) {
    if let crate::traits::term::TyTerm::Param(key) = &hint.source {
        if let Some((_, actual)) = explicit.iter().find(|(ordinal, _)| *ordinal == key.ordinal) {
            hint.descriptor = Some(actual.descriptor);
            hint.source = actual.source.clone();
        } else if callee.signature().result_symbolic.as_ref() == Some(&hint.source)
            && let Some(actual) = result
        {
            if hint.descriptor.is_none() {
                hint.descriptor = Some(actual.descriptor);
            }
            hint.source = actual.source.clone();
        }
    }
}

/// Account the opaque source proof retained beside an argument hint as well as
/// its source term. Descriptor publication never supplies source identity.
pub(super) fn retained_hint_bytes(hint: &ArgumentHint) -> Result<usize, Diagnostic> {
    let mut bytes = hint
        .source
        .heap_bytes()
        .ok_or_else(crate::runtime::arena::resource)?;
    if let Some(proof) = &hint.closed_reference {
        bytes = bytes
            .checked_add(proof.allocated_bytes()?)
            .ok_or_else(crate::runtime::arena::resource)?;
    }
    Ok(bytes)
}

/// A closed declaration supplies pointee/access constraints. Only the actual
/// caller's written borrow supplies a Local source range; forwarded references
/// keep the source attached by their actual evaluation.
pub(super) fn runtime_hint(
    db: &mut Database,
    frame: &Frame,
    store: &mut GuestStore,
    callee: &SourceCallable,
    hint: &ArgumentHint,
    ordinal: usize,
    slot: SlotId,
) -> Result<
    (
        Option<crate::runtime::shape::TypeId>,
        Option<crate::traits::term::TyTerm>,
    ),
    Diagnostic,
> {
    use crate::{
        syntax::body_index::{ExpressionKind, NodeRole},
        traits::term::{RegionTerm, TyTerm},
    };
    let Some(proof) = &hint.closed_reference else {
        return Ok((
            hint.descriptor,
            hint.descriptor.map(|_| hint.source.clone()),
        ));
    };
    proof.validate_hint(hint)?;
    proof.require_current(db, callee)?;
    if proof.ordinal() as usize != ordinal {
        return Err(call_fault("reference hint ordinal differs"));
    }
    let range = if let Some(call) = &frame.runtime_call {
        if call.arguments().get(ordinal) != Some(&slot) {
            return Err(call_fault("reference hint slot differs"));
        }
        match call.argument_borrow(
            u32::try_from(ordinal)
                .map_err(|_| call_fault("reference argument ordinal overflow"))?,
        )? {
            Some(borrow) => {
                borrow.require(call, frame.body.body(), proof.ordinal())?;
                if borrow.source().profile != *frame.body.profile()
                    || (borrow.access() == crate::ir::op::BorrowAccess::Mutable) != proof.mutable()
                {
                    return Err(call_fault("reference argument source/access differs"));
                }
                borrow.temporary().map(|_| {
                    (
                        borrow.borrow_span().relative_start,
                        borrow.borrow_span().relative_end,
                    )
                })
            }
            None => None,
        }
    } else {
        let call = frame
            .body
            .nodes()
            .get(frame.pc().node as usize)
            .ok_or_else(|| call_fault("reference caller absent"))?;
        if call.role != NodeRole::Expression(ExpressionKind::Call) {
            return Err(call_fault(
                "reference hint requires its indexed ordinary call",
            ));
        }
        // Structural call operands also contain the callee expression. The
        // actual ArgList-produced value slot selects the written argument;
        // ordinal indexing of this broader operand list is not argument order.
        let mut arguments = call
            .operands
            .iter()
            .filter_map(|node| frame.body.nodes().get(node.0 as usize))
            .filter(|node| node.value == Some(slot));
        let argument = arguments.next();
        if arguments.next().is_some() {
            return Err(call_fault(
                "reference argument slot has ambiguous indexed origins",
            ));
        }
        argument
            .filter(|node| {
                node.role==NodeRole::Expression(ExpressionKind::Borrow)
                // Literal/call nodes establish a fresh scalar temporary. Other
                // conflates groups, projections and reborrows; it requires an
                // exact source producer instead of descriptor/name inference.
                && node.operands.first()
                    .and_then(|operand|frame.body.nodes().get(operand.0 as usize))
                    .is_some_and(|operand|matches!(operand.role,
                        NodeRole::Expression(ExpressionKind::Literal | ExpressionKind::Call)))
            })
            .map(|node| (node.range.start, node.range.end))
    };
    let source = range.map(|(start, end)| {
        TyTerm::Ref(
            RegionTerm::Local {
                definition: frame.body.body().definition,
                start,
                end,
            },
            proof.mutable(),
            Box::new(TyTerm::Primitive(proof.pointee())),
        )
    });
    // Materialize only the checked descriptor, after source admission.
    let descriptor = proof.materialize_hint(store)?;
    Ok((Some(descriptor), source))
}

impl RunMachine {
    pub(super) fn prepare_later_argument_hint(
        &mut self,
        db: &mut Database,
        site: u32,
        completed: u32,
        target: &PcKey,
    ) -> QueryPoll<()> {
        match self.later_argument_hint(db, site, completed, target) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }

    fn later_argument_hint(
        &mut self,
        db: &mut Database,
        site: u32,
        completed: u32,
        target: &PcKey,
    ) -> Result<QueryPoll<()>, Diagnostic> {
        self.frame.preflight_call_advance(site, completed)?;
        let next = completed
            .checked_add(1)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| call_fault("argument hint ordinal overflow"))?;
        let selected = self
            .pending
            .get(&self.frame.call_identity(site))
            .ok_or_else(|| call_fault("argument hint lacks selected call identity"))?;
        let slot = *selected
            .argument_slots
            .get(next)
            .ok_or_else(|| call_fault("next argument is outside the written call"))?;
        let continuation_matches = if let Some(call) = &self.frame.runtime_call {
            self.frame.pc().node == site
                && call.arguments().get(next) == Some(&slot)
                && call.argument_key(
                    u32::try_from(next).map_err(|_| call_fault("argument ordinal overflow"))?,
                )? == target
        } else {
            self.frame.pc().node == site
                && matches!(Phase::from_code(self.frame.pc().phase), Phase::Operand(_))
                && target.body == self.frame.pc().body
                && target.phase == Phase::Entry.code()?
                && self
                    .frame
                    .body
                    .nodes()
                    .get(target.node as usize)
                    .is_some_and(|node| node.value == Some(slot))
        };
        if !continuation_matches {
            return Err(call_fault(
                "argument hint continuation differs from its written slot",
            ));
        }
        self.frame.preflight_hint(slot)?;
        if next >= selected.callee.signature().parameters.len() {
            // Excess written arguments still execute before the final arity
            // guard. They receive no fabricated parameter context.
            return Ok(QueryPoll::Ready(Ok(())));
        }
        if next < selected.hints.len() {
            // A query pause can revisit this boundary, but never repeats the
            // completed argument's opcode or consumes its owner again.
            return Ok(QueryPoll::Ready(Ok(())));
        }
        if next != selected.hints.len() || selected.hints.len() == selected.hints.capacity() {
            return Err(call_fault("argument hint prefix changed during suspension"));
        }
        let graph = self
            .graph
            .as_ref()
            .ok_or_else(|| call_fault("argument hint source catalogue absent"))?;
        let sources: Vec<_> = self.sources.iter().collect();
        let mut prepared = match prepare_argument_hint(db, &selected.callee, graph, &sources, next)
        {
            QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(hints))) => hints,
            QueryPoll::Ready(Ok(_)) => {
                return Err(call_fault(
                    "argument source hints require further type integration",
                ));
            }
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
        };
        if prepared.len() != 1 {
            return Err(call_fault("selected argument has no unique source hint"));
        }
        let mut hint = prepared
            .pop()
            .ok_or_else(|| call_fault("argument hint disappeared"))?;
        apply_context(
            &mut hint,
            &selected.callee,
            &selected.explicit,
            selected.result_hint.as_ref(),
        );
        let heap = retained_hint_bytes(&hint)?;
        let retained = selected
            .bytes
            .checked_add(heap)
            .ok_or_else(crate::runtime::arena::resource)?;
        let pending = self
            .pending_bytes
            .checked_add(heap)
            .ok_or_else(crate::runtime::arena::resource)?;
        if pending
            .checked_add(self.call_bytes)
            .is_none_or(|n| n > self.call_limits.max_metadata_bytes)
        {
            return Err(crate::runtime::arena::resource());
        }
        let (descriptor, source) = runtime_hint(
            db,
            &self.frame,
            &mut self.store,
            &selected.callee,
            &hint,
            next,
            slot,
        )?;
        self.frame.preflight_source_view(slot, source.as_ref())?;
        // All checked admission precedes mutation. Inline prefix capacity was
        // reserved and charged at identity selection; this adds only its heap.
        self.frame.set_source_view(slot, source)?;
        self.frame.set_hint(slot, descriptor)?;
        let identity = self.frame.call_identity(site);
        let selected = self
            .pending
            .get_mut(&identity)
            .ok_or_else(|| call_fault("selected call disappeared during argument admission"))?;
        selected.hints.push(hint);
        selected.bytes = retained;
        self.pending_bytes = pending;
        Ok(QueryPoll::Ready(Ok(())))
    }
}

#[cfg(test)]
#[path = "run_reference_local_source_tests.rs"]
mod reference_local_source_tests;
