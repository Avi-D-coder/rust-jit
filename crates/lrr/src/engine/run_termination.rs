//! The program entry's `Termination`. rustc's `lang_start` calls `main` and
//! reports what it returned; a `main` returning `Result<(), E>` prints
//! `Error: {e:?}` to stderr and exits with status 1 for `Err`. Once `main`'s
//! frame has finished its return cleanup with a value the scalar summary
//! cannot state, the dispatcher enters the facade's ordinary source function
//! `std::process::__lrr_report::<T>(value) -> i32` (the value moves into its
//! argument; `T: Termination` selects the impl through the ordinary trait
//! route) and the status it returns ends the run. Nothing here evaluates
//! Rust: it forms one call, exactly as a formatting call is formed.
use super::*;
use crate::{
    runtime::{cell::TicketId, shape::TypeKey},
    source::manifest::Namespace,
};

impl RunMachine {
    /// The entry function with a written result Type (`-> Result<(), E>`)
    /// gets the definition environment every called function gets from its
    /// call contract: the declared result Type is the expected type of the
    /// returned and `return`ed values (`Ok(())`, `?`'s `from_residual`).
    /// Prepared once, before the entry frame starts.
    pub(super) fn prepare_root_context(&mut self, db: &mut Database) -> QueryPoll<()> {
        if self.root_context_prepared {
            return QueryPoll::Ready(Ok(()));
        }
        let Some(root) = self.root_source.clone() else {
            return QueryPoll::Ready(Ok(()));
        };
        if !self.calls.is_empty()
            || self.frame.counters().operations_started != 0
            || root.signature().result.is_empty()
        {
            self.root_context_prepared = true;
            return QueryPoll::Ready(Ok(()));
        }
        let mut call = match self.prepare_selected_source_call(db, root, &[], None, &[], &[]) {
            QueryPoll::Ready(Ok(call)) => call,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.root_context_prepared = true;
        if let Some(context) = call.context.take() {
            return QueryPoll::Ready(self.frame.install_context(context));
        }
        QueryPoll::Ready(Ok(()))
    }
    /// Whether `main`'s returned value is not a primitive scalar: a nominal,
    /// tuple or other aggregate that `Termination` reports.
    pub(super) fn root_value_is_aggregate(&self, value: ValueRepr) -> bool {
        self.store
            .type_descriptor(crate::runtime::shape::TypeId(value.type_id))
            .is_ok_and(|descriptor| !matches!(descriptor.key.as_ref(), TypeKey::Primitive(_)))
    }
    /// The facade entry, selected through the ordinary Value namespace.
    fn termination_entry(&mut self, db: &mut Database) -> QueryPoll<Arc<SourceCallable>> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("termination requires a source catalogue")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(self.members.as_deref()),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let std = crate::facade::unit(self.frame.body.profile(), "std");
        let path = format!(
            "{}::process::__lrr_report",
            crate::frontend::callables::dollar_crate_segment(std)
        );
        let found = match resolver.lookup_written(
            db,
            self.frame.pc().body.definition,
            &path,
            Namespace::Value,
        ) {
            QueryPoll::Ready(Ok(found)) => found,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        if !matches!(found, crate::frontend::names::NameLookup::Defined(_)) {
            return QueryPoll::Ready(Err(call_fault(
                "reporting `main`'s result requires the library facade (`std::process::__lrr_report`)",
            )));
        }
        match resolver.select(db, found) {
            QueryPoll::Ready(Ok(CallSelection::Source(callee))) => {
                QueryPoll::Ready(Ok(Arc::new(*callee)))
            }
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Err(call_fault(
                "termination entry is not an ordinary source function",
            ))),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    pub(super) fn enter_termination(&mut self, db: &mut Database) -> QueryPoll<()> {
        macro_rules! poll {
            ($e:expr) => {
                match $e {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        let Some((ticket, refusal)) = self
            .termination
            .as_ref()
            .map(|(ticket, refusal)| (*ticket, refusal.clone()))
        else {
            return QueryPoll::Ready(Err(call_fault("no termination report is demanded")));
        };
        if self.prepared_call.is_some() || self.frame.call_arguments.is_some() {
            return QueryPoll::Ready(Err(call_fault(
                "a termination report cannot interleave a sealed call",
            )));
        }
        // Without the facade entry (or a closed source Type for the value) the
        // scalar summary's refusal stands.
        let callee = match self.termination_entry(db) {
            QueryPoll::Ready(Ok(callee)) => callee,
            QueryPoll::Ready(Err(_)) => return QueryPoll::Ready(Err(refusal)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let value = match self.store.peek_ticket(ticket) {
            Ok(value) => value,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let descriptor = crate::runtime::shape::TypeId(value.type_id);
        let Some(source) = self.store.descriptor_source_term(descriptor) else {
            return QueryPoll::Ready(Err(refusal));
        };
        let arguments = [ArgumentType { source, descriptor }];
        let call = poll!(self.prepare_selected_source_call(
            db,
            callee,
            &[],
            None,
            &arguments,
            &[None],
        ));
        QueryPoll::Ready((|| {
            call.callee.revalidate(db)?;
            let cell = self.frame.lend_termination_value(ticket, &mut self.store)?;
            let args = crate::vm::call::ArgumentPacket::seal(
                &mut self.store,
                &[cell],
                &call.contracts.parameter_descriptors,
            )?;
            let callee = self.frame.enter_termination(
                call.body.clone(),
                call.body.pc(NodeId(0), Phase::Entry)?,
                &mut self.store,
            )?;
            self.termination = None;
            self.enter_callee(call, callee, &args, CallContinuation::Termination)
        })())
    }
    /// The report finished its return cleanup: its `i32` status ends the run.
    pub(super) fn finish_termination(
        &mut self,
        suspended: SuspendedCall,
        ticket: TicketId,
    ) -> Result<RunReport, Diagnostic> {
        let status = self.store.peek_ticket(ticket)?;
        let i32_type = crate::runtime::shape::PrimitiveIdentity::from_recipe(
            crate::ir::op::PrimitiveType::I32,
        )?
        .id();
        if status.kind != 4 || status.type_id != i32_type.0 {
            return Err(Diagnostic::new(
                "LRR-E-CALL-RETURN-TYPE",
                "`__lrr_report` returned a value that is not its `i32` status".into(),
            ));
        }
        let exit_code = status.payload as i64 as i32;
        self.restore_caller(suspended, ticket)?;
        self.store.discard(ticket)?;
        Ok(RunReport {
            live_edition: self.frame.body.profile().edition,
            semantics_profile: crate::core::profile::SEMANTICS_PROFILE,
            schema: "lrr/v2/development-run",
            status: "completed",
            value: ValueSummary::Unit,
            opcode_counters: self.all_counters()?,
            installed_recipes: self.installed.len(),
            installed_lrir_bytes: self.installed.metadata_bytes(),
            output: std::mem::take(&mut self.output),
            exit_code,
        })
    }
}
