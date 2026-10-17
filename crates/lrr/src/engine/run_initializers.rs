//! Reached `const` and `static` initializers run through the ordinary
//! interpreter (E-R5-007). A `ValuePath` naming such an item, whose value the
//! pure evaluator does not produce, demands the item's initializer; the
//! dispatcher enters its body as a zero-parameter source call
//! (`CallResolver::initializer`) and the returned value becomes:
//! - a static's run-long cell, installed once and read or borrowed in place;
//! - a constant's value: kept in a run-long cell and copied on each use when
//!   its value is copied intrinsically (scalars, shared references); otherwise
//!   handed to the one use that demanded it, so each use is a fresh value.
use super::*;
use crate::{core::digest::Digest, runtime::cell::TicketId};

/// The initializer a `ValuePath` preparation demands before its PC runs.
pub(crate) struct InitializerDemand {
    pub(crate) identity: Digest,
    pub(crate) is_static: bool,
    pub(crate) pc: PcKey,
}

impl RunMachine {
    pub(super) fn demand_initializer(&mut self, identity: Digest, is_static: bool) {
        self.initializer_demand = Some(InitializerDemand {
            identity,
            is_static,
            pc: self.frame.pc().clone(),
        });
    }
    pub(super) fn enter_initializer(&mut self, db: &mut Database) -> QueryPoll<()> {
        macro_rules! poll {
            ($e:expr) => {
                match $e {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        let Some(demand) = self.initializer_demand.take() else {
            return QueryPoll::Ready(Err(call_fault("no initializer is demanded")));
        };
        if demand.pc != *self.frame.pc()
            || self.prepared_call.is_some()
            || self.frame.call_arguments.is_some()
        {
            return QueryPoll::Ready(Err(call_fault(
                "an initializer cannot interleave another call",
            )));
        }
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("initializers require a source catalogue")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(self.members.as_deref()),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(callee) = poll!(resolver.initializer(db, demand.identity)) else {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "constant or static initializer outside the pure constant domain needs a module-level item (a body-local one has no executable initializer route yet)".into(),
            )));
        };
        let reference_result = callee
            .signature()
            .result
            .first()
            .is_some_and(|token| token.text == "&");
        let writable = callee
            .signature()
            .header
            .tokens
            .windows(2)
            .any(|pair| pair[0].text == "static" && pair[1].text == "mut");
        let call = poll!(self.prepare_selected_source_call(
            db,
            Arc::new(callee),
            &[],
            None,
            &[],
            &[],
        ));
        QueryPoll::Ready((|| {
            call.callee.revalidate(db)?;
            let args = crate::vm::call::ArgumentPacket::seal(&mut self.store, &[], &[])?;
            let entry = call.body.pc(NodeId(0), Phase::Entry)?;
            let mut frame = crate::vm::frame::Frame::new(call.body.clone(), entry, &mut self.store)?;
            frame.fuel = self.frame.fuel.share();
            frame.actions = self.frame.actions.share();
            self.enter_callee(
                call,
                frame,
                &args,
                CallContinuation::Initializer {
                    identity: demand.identity,
                    is_static: demand.is_static,
                    writable,
                },
            )?;
            // The entered initializer frame runs in its root scope; a
            // reference result is its root value's coercion context.
            self.frame.enter_initializer_root()?;
            if reference_result {
                crate::vm::step::initializer_root_hint(&mut self.frame, &mut self.store)?;
            }
            Ok(())
        })())
    }
    /// The initializer's value is admitted as its declared result.
    pub(super) fn admit_initializer_return(
        &self,
        db: &mut Database,
        suspended: &SuspendedCall,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let value = self.store.peek_ticket(ticket)?;
        let actual = self.store.effective_type(value)?.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "initializer result requires its reached concrete type".into(),
            )
        })?;
        self.check_source_return(db, &suspended.call, actual)
    }
    /// Install the returned value; the caller's `ValuePath` is prepared again
    /// at the same PC and now finds it.
    pub(super) fn resume_initializer(
        &mut self,
        suspended: SuspendedCall,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let CallContinuation::Initializer {
            identity,
            is_static,
            writable,
        } = suspended.continuation
        else {
            return Err(call_fault("return continuation is not an initializer"));
        };
        let (_, source) = self.restore_caller(suspended, ticket)?;
        let value = self.store.peek_ticket(ticket)?;
        let ty = self.store.effective_type(value)?.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "initializer result requires its reached concrete type".into(),
            )
        })?;
        // A scalar or shared reference is copied intrinsically: one run-long
        // value serves every use, as rustc's evaluated constant does.
        let copied = !is_static
            && matches!(
                self.store.type_descriptor(ty)?.key.as_ref(),
                crate::runtime::shape::TypeKey::Primitive(_)
                    | crate::runtime::shape::TypeKey::Ref { mutable: false, .. }
            );
        if is_static || copied {
            let cell = self.run_long_cell(ty, writable)?;
            self.store.initialize(cell, ticket)?;
            if is_static {
                self.static_cells.insert(identity, cell);
                if let Some(view) = source {
                    self.static_views.insert(identity, view);
                }
            } else {
                self.const_cells.insert(identity, (cell, source));
            }
        } else {
            self.initialized_values
                .insert(self.frame.pc().clone(), (ticket, source));
        }
        Ok(())
    }
    /// The next slot of the run's static anchor.
    pub(super) fn next_static_slot(&mut self) -> Result<u32, Diagnostic> {
        let slot = self.static_slots;
        self.static_slots = slot
            .checked_add(1)
            .ok_or_else(|| Diagnostic::new("LRR-E-INTERNAL-IR", "static slot overflow".into()))?;
        Ok(slot)
    }
    /// A location owned by the run for its whole duration.
    fn run_long_cell(
        &mut self,
        ty: crate::runtime::shape::TypeId,
        writable: bool,
    ) -> Result<crate::runtime::cell::CellId, Diagnostic> {
        let anchor = match self.static_anchor {
            Some(anchor) => anchor,
            None => {
                let anchor = self.store.new_frame_anchor()?;
                self.static_anchor = Some(anchor);
                anchor
            }
        };
        let slot = self.next_static_slot()?;
        self.store.new_cell(
            crate::runtime::cell::StorageContract::Exact(ty),
            writable,
            crate::runtime::cell::CellOwner::FrameSlot {
                frame: anchor,
                slot,
            },
        )
    }
}
