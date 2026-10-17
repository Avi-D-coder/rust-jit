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

/// A declared parameter's written Type at its argument boundary.
pub(super) enum WrittenArgumentHint {
    None,
    /// Every parameter it names is fixed: its exact descriptor.
    Exact(crate::runtime::shape::TypeId),
    /// Known structure with open parts (`Option<T>` before `T` is fixed,
    /// `(T, u8)`, `Box<dyn Fn()>`): a partial context for the argument's own
    /// construction.
    Partial(
        std::sync::Arc<crate::ir::written_type::WrittenType>,
        crate::vm::frame::PartialArguments,
    ),
}

/// The written parameter Type of `hint` under the callee type arguments
/// already fixed at this call. Advisory context only: the evaluated contract
/// still checks the reached argument against the declared Type.
pub(super) fn written_hint(
    store: &mut GuestStore,
    hint: &ArgumentHint,
    definition: crate::core::digest::Digest,
    bound: &BTreeMap<crate::traits::term::ParamKey, crate::runtime::shape::TypeId>,
) -> Result<WrittenArgumentHint, Diagnostic> {
    // A bare parameter (`x: T`) the call has already fixed.
    if let crate::traits::term::TyTerm::Param(key) = &hint.source
        && key.definition == definition
        && hint.closed_reference.is_none()
    {
        return Ok(match bound.get(key) {
            Some(ty) => WrittenArgumentHint::Exact(*ty),
            None => WrittenArgumentHint::None,
        });
    }
    let Some(written) = &hint.written else {
        return Ok(WrittenArgumentHint::None);
    };
    let lookup = |ordinal: u32| {
        bound
            .get(&crate::traits::term::ParamKey {
                definition,
                ordinal,
            })
            .copied()
    };
    if let Some(ty) = written.materialize_with(store, &lookup)? {
        return Ok(WrittenArgumentHint::Exact(ty));
    }
    let mut fixed = Vec::new();
    for (key, ty) in bound.range(
        crate::traits::term::ParamKey {
            definition,
            ordinal: 0,
        }..=crate::traits::term::ParamKey {
            definition,
            ordinal: u32::MAX,
        },
    ) {
        let index = key.ordinal as usize;
        if fixed.len() <= index {
            fixed.resize(index + 1, None);
        }
        fixed[index] = Some(*ty);
    }
    Ok(WrittenArgumentHint::Partial(
        std::sync::Arc::clone(written),
        crate::vm::frame::PartialArguments::Fixed(fixed.into()),
    ))
}

/// The callee type arguments fixed before argument `upto` is evaluated:
/// explicit type arguments, then the reached descriptors of the already
/// evaluated arguments at their declared parameter positions. A
/// disagreement is left to the evaluated contract, which reports it.
pub(super) fn bound_call_parameters(
    frame: &Frame,
    store: &GuestStore,
    definition: crate::core::digest::Digest,
    explicit: &[(u32, ArgumentType)],
    earlier: &[ArgumentHint],
    slots: &[SlotId],
    expected: &BTreeMap<crate::traits::term::ParamKey, crate::runtime::shape::TypeId>,
) -> Result<BTreeMap<crate::traits::term::ParamKey, crate::runtime::shape::TypeId>, Diagnostic> {
    let mut bound = BTreeMap::new();
    for (ordinal, actual) in explicit {
        bound.insert(
            crate::traits::term::ParamKey {
                definition,
                ordinal: *ordinal,
            },
            actual.descriptor,
        );
    }
    for (hint, slot) in earlier.iter().zip(slots) {
        let crate::runtime::cell::CellState::Live(value) = store.state(frame.cell(*slot)?)? else {
            continue;
        };
        if value.type_id == 0 {
            continue;
        }
        let mut reached = bound.clone();
        if crate::frontend::call_contracts::bind_runtime_parameters(
            &hint.source,
            crate::runtime::shape::TypeId(value.type_id),
            store,
            definition,
            &mut reached,
        )
        .is_ok()
        {
            bound = reached;
        }
    }
    // The result context's expectation fills only what the call itself
    // has not fixed.
    for (key, ty) in expected {
        bound.entry(*key).or_insert(*ty);
    }
    Ok(bound)
}

/// Account the opaque source proof retained beside an argument hint as well as
/// its source term. Descriptor publication never supplies source identity.
pub(super) fn retained_hint_bytes(hint: &ArgumentHint) -> Result<usize, Diagnostic> {
    let mut bytes = hint
        .source
        .heap_bytes()
        .ok_or_else(crate::runtime::arena::resource)?;
    if let Some(written) = &hint.written {
        bytes = bytes
            .checked_add(written.heap_bytes())
            .and_then(|n| n.checked_add(std::mem::size_of::<crate::ir::written_type::WrittenType>()))
            .ok_or_else(crate::runtime::arena::resource)?;
    }
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
        // A closed tuple/array parameter gives its written argument the
        // structural descriptor its fields construct against.
        let descriptor = match hint.descriptor {
            Some(descriptor) => Some(descriptor),
            None if matches!(
                hint.source,
                TyTerm::Tuple(_) | TyTerm::Array(..)
            ) =>
            {
                store.intern_structural_term(&hint.source)?
            }
            // A closed `&[T]`/`&mut [T]` parameter: like a closed reference
            // proof, descriptor only (the coercion site's expectation, which
            // selects unsizing and implicit reborrow); the caller slot keeps
            // the region its actual evaluation attaches.
            None if matches!(&hint.source, TyTerm::Ref(_, _, target)
                if matches!(target.as_ref(), TyTerm::Slice(_)))
                && crate::frontend::call_contracts::closed_term(&hint.source) =>
            {
                let descriptor = store.intern_structural_term(&hint.source)?;
                return Ok((descriptor, None));
            }
            None => None,
        };
        return Ok((descriptor, descriptor.map(|_| hint.source.clone())));
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
        let method = call.role == NodeRole::Expression(ExpressionKind::MethodCall);
        if call.role != NodeRole::Expression(ExpressionKind::Call) && !method {
            return Err(call_fault(
                "reference hint requires its indexed ordinary call",
            ));
        }
        // A method receiver's autoref borrows its place: a fresh temporary
        // (literal/call receiver) gets that receiver's local region, a named
        // place keeps the source its borrow attaches.
        if method && ordinal == 0 {
            let receiver = call
                .operands
                .first()
                .and_then(|node| frame.body.nodes().get(node.0 as usize))
                .ok_or_else(|| call_fault("method receiver node absent"))?;
            let temporary = matches!(
                receiver.role,
                NodeRole::Expression(
                    ExpressionKind::Literal | ExpressionKind::Call | ExpressionKind::MethodCall
                )
            );
            let source = temporary.then(|| {
                TyTerm::Ref(
                    RegionTerm::Local {
                        definition: frame.body.body().definition,
                        start: receiver.range.start,
                        end: receiver.range.end,
                    },
                    proof.mutable(),
                    Box::new(proof.pointee_source().clone()),
                )
            });
            let descriptor = proof.materialize_hint(store)?;
            return Ok((Some(descriptor), source));
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
            Box::new(proof.pointee_source().clone()),
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
                && (matches!(Phase::from_code(self.frame.pc().phase), Phase::Operand(_))
                    // A method call's receiver argument completes at its Exit.
                    || (Phase::from_code(self.frame.pc().phase) == Phase::Exit
                        && self.frame.body.nodes().get(site as usize).is_some_and(|node| {
                            node.role
                                == crate::syntax::body_index::NodeRole::Expression(
                                    crate::syntax::body_index::ExpressionKind::MethodCall,
                                )
                        })))
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
            QueryPoll::Ready(Ok(other)) => {
                return Err(call_fault(&format!(
                    "argument source hints require further type integration: {}",
                    other.describe()
                )));
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
        // An associated-type projection (`T::V`) is normalized once the
        // generics it names are bound by the already evaluated arguments.
        let definition = selected.callee.signature().definition;
        let needs_bound = hint.written.as_ref().is_some_and(|written| written.has_parameters())
            || matches!(&hint.source, crate::traits::term::TyTerm::Param(key) if key.definition == definition)
            || crate::frontend::associated_types::contains_projection(&hint.source);
        let bound = if needs_bound {
            bound_call_parameters(
                &self.frame,
                &self.store,
                definition,
                &selected.explicit,
                &selected.hints,
                &selected.argument_slots,
                &selected.expected,
            )?
        } else {
            BTreeMap::new()
        };
        if crate::frontend::associated_types::contains_projection(&hint.source)
            && let Some(members) = self.members.clone()
        {
            let runtime = bound.clone();
            let mut bound = hint.source.clone();
            substitute_runtime_parameters(&mut bound, &runtime, &self.store);
            match crate::frontend::associated_types::normalize(db, graph, &sources, &members, &bound)
            {
                QueryPoll::Ready(Ok(Some(normalized))) => {
                    hint.descriptor = self.store.intern_structural_term(&normalized)?;
                    hint.source = normalized;
                }
                QueryPoll::Ready(Ok(None)) => {}
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
            }
        }
        let heap = retained_hint_bytes(&hint)?;
        let retained = selected
            .bytes
            .checked_add(heap)
            .ok_or_else(crate::runtime::arena::resource)?;
        let pending = self
            .pending_bytes
            .checked_add(heap)
            .ok_or_else(crate::runtime::arena::resource)?;
        let (descriptor, source) = runtime_hint(
            db,
            &self.frame,
            &mut self.store,
            &selected.callee,
            &hint,
            next,
            slot,
        )?;
        let written = match descriptor {
            Some(_) => WrittenArgumentHint::None,
            None => written_hint(&mut self.store, &hint, definition, &bound)?,
        };
        let descriptor = match &written {
            WrittenArgumentHint::Exact(ty) => Some(*ty),
            _ => descriptor,
        };
        self.frame.preflight_source_view(slot, source.as_ref())?;
        // All checked admission precedes mutation. Inline prefix capacity was
        // reserved and charged at identity selection; this adds only its heap.
        self.frame.set_source_view(slot, source)?;
        self.frame.set_hint(slot, descriptor)?;
        if let WrittenArgumentHint::Partial(written, arguments) = written {
            self.frame.set_partial_hint_with(slot, written, Some(arguments))?;
        }
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

/// Replace each bound generic parameter with its reached runtime type.
fn substitute_runtime_parameters(
    term: &mut crate::traits::term::TyTerm,
    runtime: &BTreeMap<crate::traits::term::ParamKey, crate::runtime::shape::TypeId>,
    store: &GuestStore,
) {
    use crate::traits::term::{GenericArg, TyTerm};
    match term {
        TyTerm::Param(key) => {
            if let Some(actual) = runtime
                .get(key)
                .and_then(|ty| store.descriptor_source_term(*ty))
            {
                *term = actual;
            }
        }
        TyTerm::Projection(key) => {
            substitute_runtime_parameters(&mut key.trait_ref.self_ty, runtime, store)
        }
        TyTerm::Ref(_, _, inner) | TyTerm::Slice(inner) | TyTerm::Array(inner, _) => {
            substitute_runtime_parameters(inner, runtime, store)
        }
        TyTerm::Tuple(items) => {
            for item in items {
                substitute_runtime_parameters(item, runtime, store);
            }
        }
        TyTerm::Nominal(_, args) => {
            for arg in args {
                if let GenericArg::Type(ty) = arg {
                    substitute_runtime_parameters(ty, runtime, store);
                }
            }
        }
        _ => {}
    }
}
