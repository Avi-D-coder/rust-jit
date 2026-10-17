//! Drop glue's `Drop::drop` witness. When a cleanup owner (scope exit, return
//! or panic cleanup, `break`/`continue`, a discarded temporary, an assigned
//! local's or place's old value, or a field of any of those) reaches a value
//! whose nominal type has a user `Drop` impl, the dispatcher selects
//! `<T as Drop>::drop` through the run's impl member index and the ordinary
//! call resolver, and enters it as an ordinary source call: shared contract
//! preparation, frame entry and return cleanup. Its receiver is a `&mut` to the
//! value being dropped, which the dropping frame lends to its own location for
//! the call's duration. The call resumes no recipe call site: its unit result
//! is discarded and the suspended cleanup activation continues with the value's
//! fields (rustc's drop glue order).
use super::*;
use crate::runtime::{cell::TicketId, shape::TypeKey};

/// The dropping frame's own terminal continuation, suspended with it while
/// its witness runs (a witness may be entered by return or panic cleanup).
pub(super) struct DropWitnessResume {
    pub(super) returning: Option<returning::ReturnContinuation>,
    pub(super) unwinding: Option<unwinding::PanicContinuation>,
}

impl RunMachine {
    pub(super) fn enter_drop_witness(&mut self, db: &mut Database) -> QueryPoll<()> {
        macro_rules! ready {
            ($e:expr) => {
                match $e {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
        }
        let Some(ty) = self.frame.drop_witness_demand() else {
            return QueryPoll::Ready(Err(call_fault("no Drop witness is demanded")));
        };
        if self.prepared_call.is_some() || self.frame.call_arguments.is_some() {
            return QueryPoll::Ready(Err(call_fault(
                "a Drop witness cannot interleave a sealed call",
            )));
        }
        let identity = match ready!(self.store.type_descriptor(ty)).key.as_ref() {
            TypeKey::Nominal { identity, .. } => *identity,
            _ => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "cleanup requires a Drop witness for a non-nominal value".into(),
                )));
            }
        };
        let callee = match self.drop_witnesses.get(&identity) {
            Some(callee) => {
                ready!(callee.revalidate(db));
                callee.clone()
            }
            None => match self.select_drop_witness(db, identity) {
                QueryPoll::Ready(Ok(callee)) => {
                    self.drop_witnesses.insert(identity, callee.clone());
                    callee
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            },
        };
        let (argument, source, descriptor) = ready!(self.frame.lend_drop_witness(&mut self.store));
        let evidence = ready!(ClosedReferenceArgument::capture_drop_receiver(
            &self.store,
            argument,
            &source
        ));
        let arguments = [ArgumentType { source, descriptor }];
        let call = match self.prepare_selected_source_call(
            db,
            Arc::from(callee),
            &[],
            None,
            &arguments,
            &[Some(evidence)],
        ) {
            QueryPoll::Ready(Ok(call)) => call,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready((|| {
            if let Some(proof) = &call.contracts.closed_references {
                proof.require_argument_cells(&self.store, &[argument], &arguments)?;
            }
            call.callee.revalidate(db)?;
            let args = crate::vm::call::ArgumentPacket::seal(
                &mut self.store,
                &[argument],
                &call.contracts.parameter_descriptors,
            )?;
            let callee = self.frame.enter_drop_witness(
                call.body.clone(),
                call.body.pc(NodeId(0), Phase::Entry)?,
                &mut self.store,
            )?;
            let resume = DropWitnessResume {
                returning: self.returning.take(),
                unwinding: self.unwinding.take(),
            };
            self.enter_callee(call, callee, &args, CallContinuation::DropWitness(resume))
        })())
    }
    /// `<T as Drop>::drop` through the member index's `Drop` impl of nominal
    /// `identity` and the ordinary call resolver (selected once per run).
    fn select_drop_witness(
        &self,
        db: &mut Database,
        identity: crate::core::digest::Digest,
    ) -> QueryPoll<Box<SourceCallable>> {
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Err(call_fault("Drop witness requires the impl index")));
        };
        let function = match members.drop_functions(identity).as_slice() {
            [function] => *function,
            [] => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "cleanup requires its source-selected Drop witness".into(),
                )));
            }
            _ => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-NAME-AMBIGUOUS",
                    "several Drop impls select one nominal type".into(),
                )));
            }
        };
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("Drop witness requires a source catalogue")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(Some(&members)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let callee = match resolver.select_member_function(db, function) {
            QueryPoll::Ready(Ok(callee)) => callee,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        if !matches!(callee.signature().parameters.as_slice(), [receiver] if receiver.receiver) {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-CALL-SIGNATURE",
                "selected `Drop::drop` is not a `&mut self` method".into(),
            )));
        }
        QueryPoll::Ready(Ok(callee))
    }
    /// `Drop::drop` returns `()`, checked like any selected source return.
    pub(super) fn admit_drop_witness_return(
        &self,
        db: &mut Database,
        suspended: &SuspendedCall,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let value = self.store.peek_ticket(ticket)?;
        if value != ValueRepr::unit() {
            return Err(Diagnostic::new(
                "LRR-E-CALL-RETURN-TYPE",
                "`Drop::drop` returned a non-unit value".into(),
            ));
        }
        let actual = self.store.effective_type(value)?.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "Drop witness result requires its unit descriptor".into(),
            )
        })?;
        self.check_source_return(db, &suspended.call, actual)
    }
    /// The witness frame finished its return cleanup: retire it, restore the
    /// dropping frame with its own continuation and continue its drop glue.
    pub(super) fn resume_drop_witness(
        &mut self,
        suspended: SuspendedCall,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let CallContinuation::DropWitness(resume) = suspended.continuation else {
            return Err(call_fault("return continuation is not a Drop witness"));
        };
        let bytes = self
            .call_bytes
            .checked_sub(suspended.call.retained_bytes)
            .ok_or_else(|| call_fault("call metadata retirement underflow"))?;
        self.store.discard(ticket)?;
        self.collected.accumulate(self.frame.counters())?;
        self.retire_pending_frame()?;
        self.call_bytes = bytes;
        self.frame = suspended.caller;
        self.body_lease = suspended.body_lease;
        self.pending = suspended.pending;
        self.returning = resume.returning;
        self.unwinding = resume.unwinding;
        self.frame.finish_drop_witness(&mut self.store)
    }
}
