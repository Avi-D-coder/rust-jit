//! Trait-method formatting, dispatcher side. A `FormatArguments` operation
//! that demands a placeholder's trait `fmt` (`vm/frame_format_calls.rs`) is
//! entered here as an ordinary source call of the facade's
//! `core::fmt::lrr_format_<trait>::<T>(&value, sink, spec, fill, width,
//! precision)`, named in the facade `core` unit. That function builds the
//! `Formatter` over a fresh output buffer and calls `<T as Trait>::fmt`
//! through the ordinary trait-call route; this module only allocates the
//! buffer, enters the call, and hands its text back to the operation.
use super::*;
use crate::{ir::format::FormatTrait, runtime::cell::TicketId, source::manifest::Namespace};

/// Output buffers of in-progress formatting calls, by run-local handle.
#[derive(Default)]
pub(crate) struct FormatSinks {
    buffers: Vec<Option<String>>,
}
impl FormatSinks {
    pub(crate) fn open(&mut self) -> usize {
        match self.buffers.iter().position(Option::is_none) {
            Some(free) => {
                self.buffers[free] = Some(String::new());
                free
            }
            None => {
                self.buffers.push(Some(String::new()));
                self.buffers.len() - 1
            }
        }
    }
    fn buffer(&mut self, sink: u64) -> Result<&mut String, Diagnostic> {
        usize::try_from(sink)
            .ok()
            .and_then(|sink| self.buffers.get_mut(sink))
            .and_then(Option::as_mut)
            .ok_or_else(|| call_fault("formatter output buffer is not open"))
    }
    pub(crate) fn append(&mut self, sink: u64, text: &str) -> Result<(), Diagnostic> {
        self.buffer(sink)?.push_str(text);
        Ok(())
    }
    pub(crate) fn take(&mut self, sink: u64) -> Result<String, Diagnostic> {
        let text = std::mem::take(self.buffer(sink)?);
        self.buffers[sink as usize] = None;
        Ok(text)
    }
}

/// The facade entry formatting a value through `format`'s trait.
fn entry_name(format: FormatTrait) -> &'static str {
    match format {
        FormatTrait::Display => "lrr_format_display",
        FormatTrait::Debug | FormatTrait::LowerHexDebug | FormatTrait::UpperHexDebug => {
            "lrr_format_debug"
        }
        FormatTrait::LowerHex => "lrr_format_lower_hex",
        FormatTrait::UpperHex => "lrr_format_upper_hex",
        FormatTrait::Octal => "lrr_format_octal",
        FormatTrait::Binary => "lrr_format_binary",
        FormatTrait::LowerExp => "lrr_format_lower_exp",
        FormatTrait::UpperExp => "lrr_format_upper_exp",
        FormatTrait::Pointer => "lrr_format_pointer",
    }
}

impl RunMachine {
    /// The facade entry for `format`, selected once per run.
    fn format_entry(
        &mut self,
        db: &mut Database,
        format: FormatTrait,
    ) -> QueryPoll<Arc<SourceCallable>> {
        let name = entry_name(format);
        if let Some(callee) = self.format_entries.get(name) {
            return QueryPoll::Ready(callee.revalidate(db).map(|()| Arc::clone(callee)));
        }
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("formatting requires a source catalogue")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(self.members.as_deref()),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let core = crate::facade::unit(self.frame.body.profile(), "core");
        let path = format!(
            "{}::fmt::{name}",
            crate::frontend::callables::dollar_crate_segment(core)
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
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-NAME-MISSING",
                format!(
                    "formatting a value through its trait requires the library facade (`core::fmt::{name}`)"
                ),
            )));
        }
        let callee = match resolver.select(db, found) {
            QueryPoll::Ready(Ok(CallSelection::Source(callee))) => Arc::new(*callee),
            QueryPoll::Ready(Ok(_)) => {
                return QueryPoll::Ready(Err(call_fault(
                    "formatting entry is not an ordinary source function",
                )));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.format_entries.insert(name, Arc::clone(&callee));
        QueryPoll::Ready(Ok(callee))
    }
    pub(super) fn enter_format_call(&mut self, db: &mut Database) -> QueryPoll<()> {
        macro_rules! poll {
            ($e:expr) => {
                match $e {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        let Some(demand) = self.frame.format_demand() else {
            return QueryPoll::Ready(Err(call_fault("no formatting call is demanded")));
        };
        if self.prepared_call.is_some() || self.frame.call_arguments.is_some() {
            return QueryPoll::Ready(Err(call_fault(
                "a formatting call cannot interleave a sealed call",
            )));
        }
        let callee = poll!(self.format_entry(db, demand.format));
        let arguments: Vec<_> = demand
            .arguments
            .iter()
            .map(|argument| ArgumentType {
                source: self.frame.bound_view(&argument.source, &self.store),
                descriptor: argument.descriptor,
            })
            .collect();
        let references = vec![None; arguments.len()];
        let call = poll!(self.prepare_selected_source_call(
            db,
            callee,
            &[],
            None,
            &arguments,
            &references,
        ));
        let cells: Vec<_> = demand.arguments.iter().map(|argument| argument.cell).collect();
        QueryPoll::Ready((|| {
            call.callee.revalidate(db)?;
            let sink = self.format_sinks.open();
            self.frame.set_format_sink(sink, &mut self.store)?;
            let args = crate::vm::call::ArgumentPacket::seal(
                &mut self.store,
                &cells,
                &call.contracts.parameter_descriptors,
            )?;
            let callee = self.frame.enter_format_call(
                call.body.clone(),
                call.body.pc(NodeId(0), Phase::Entry)?,
                &mut self.store,
            )?;
            self.enter_callee(call, callee, &args, CallContinuation::Format { sink })
        })())
    }
    /// The entry's `fmt::Result` is admitted as its declared result; it
    /// resumes no destination of the operation.
    pub(super) fn admit_format_return(
        &self,
        db: &mut Database,
        suspended: &SuspendedCall,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        if !suspended.caller.format_call_entered() {
            return Err(call_fault("formatting result requires an entered call"));
        }
        let value = self.store.peek_ticket(ticket)?;
        let actual = self.store.effective_type(value)?.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "formatting result requires its reached concrete type".into(),
            )
        })?;
        self.check_source_return(db, &suspended.call, actual)
    }
    /// The entry finished its return cleanup: restore the formatting frame
    /// and give the operation the placeholder's text.
    pub(super) fn resume_format_call(
        &mut self,
        suspended: SuspendedCall,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let CallContinuation::Format { sink } = suspended.continuation else {
            return Err(call_fault("return continuation is not a formatting call"));
        };
        self.restore_caller(suspended, ticket)?;
        // `Result<(), fmt::Error>`: `Ok` is variant 0.
        let ok = self.store.aggregate_variant(ticket)? == Some(0);
        self.frame.drop_format_result(ticket, &mut self.store)?;
        let text = self.format_sinks.take(sink as u64)?;
        if !ok {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "a formatting trait implementation returned an error; rustc's \
                 resulting panic is not reported yet"
                    .into(),
            ));
        }
        self.frame.finish_format_call(text, &mut self.store)
    }
    /// The entry unwound: its buffer closes and the formatting operation
    /// becomes the panicking one.
    pub(super) fn unwind_format_call(&mut self, sink: usize) -> Result<(), Diagnostic> {
        self.format_sinks.take(sink as u64)?;
        self.frame.unwind_format_call(&mut self.store)
    }
}
