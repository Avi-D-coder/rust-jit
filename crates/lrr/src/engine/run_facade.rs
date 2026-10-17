//! Closed library facade calls (Section 17). A call whose ordinary value
//! lookup selected a facade function evaluates its written arguments like any
//! call, seals them, and completes in place: the intrinsic consumes the
//! argument tickets and either writes the call result and resumes the caller
//! or raises a guest panic at the call site.
use super::*;
use crate::{
    facade::functions::LibraryFunction,
    runtime::{
        cell::{TicketId, str_storage::ReferenceView},
        value::ValueRepr,
    },
};

pub(super) enum FacadeOutcome {
    Panic(String),
    Value(TicketId),
}

fn facade_fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}

/// Only paths that can name a library crate item are offered to the facade
/// selector; ordinary relative calls keep their single source selection.
fn library_candidate(name: &str) -> bool {
    name.starts_with("$crate@")
        || name.starts_with("::")
        || ["std::", "core::", "alloc::"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

impl RunMachine {
    /// Select (once per activation and site) whether this call names a closed
    /// facade function through the ordinary value namespace.
    pub(super) fn select_facade_call(
        &mut self,
        db: &mut Database,
        site: u32,
        name: &str,
    ) -> QueryPoll<bool> {
        let identity = self.frame.call_identity(site);
        if self.frame.facade_calls.contains_key(&identity) {
            return QueryPoll::Ready(Ok(true));
        }
        if !library_candidate(name) || self.pending.contains_key(&identity) {
            return QueryPoll::Ready(Ok(false));
        }
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Ok(false));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(self.members.as_deref()),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match resolver.source_function(db, self.frame.pc().body.definition, name) {
            QueryPoll::Ready(Ok(CallSelection::Facade(function, definition))) => {
                self.frame
                    .facade_calls
                    .insert(identity, (function, definition));
                QueryPoll::Ready(Ok(true))
            }
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Ok(false)),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// Facade arguments carry no source parameter hints.
    pub(super) fn start_facade_call(
        &mut self,
        site: u32,
        arguments: &[SlotId],
    ) -> Result<(), Diagnostic> {
        let identity = self.frame.call_identity(site);
        let (_, definition) = *self
            .frame
            .facade_calls
            .get(&identity)
            .ok_or_else(|| facade_fault("facade call start lacks its selection"))?;
        self.frame.call_start = Some(crate::vm::call::CallStartFacts {
            ordinary: crate::frontend::callables::OrdinaryCallWitness::facade(
                self.frame.body.body().clone(),
                NodeId(site),
                definition,
            ),
            site,
            hints: arguments.iter().map(|slot| (*slot, None, None)).collect(),
            partial: Vec::new(),
        });
        Ok(())
    }
    pub(super) fn seal_facade_call(
        &mut self,
        site: u32,
        arguments: &[SlotId],
    ) -> Result<(), Diagnostic> {
        let identity = self.frame.call_identity(site);
        let (function, _) = *self
            .frame
            .facade_calls
            .get(&identity)
            .ok_or_else(|| facade_fault("facade call seal lacks its selection"))?;
        if arguments.len() != function.arity() {
            return Err(Diagnostic::new(
                "LRR-E-CALL-ARITY",
                "evaluated written argument count differs from the facade signature".into(),
            ));
        }
        let mut cells = Vec::with_capacity(arguments.len());
        let mut types = Vec::with_capacity(arguments.len());
        for slot in arguments {
            let cell = self.frame.cell(*slot)?;
            let value = self.store.admit_move(cell)?;
            let descriptor = self.store.effective_type(value)?.ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "call argument requires a reached concrete type".into(),
                )
            })?;
            cells.push(cell);
            types.push(descriptor);
        }
        self.frame.call_types = types;
        self.frame.call_cells = cells;
        self.frame.call_ready = true;
        self.frame.facade_sealed = Some((function, site));
        Ok(())
    }
    /// The Call exit of a sealed facade call: complete it in place.
    pub(super) fn complete_facade_call(
        &mut self,
        action: crate::vm::action::CallAction,
        site: crate::ir::recipe::CallSiteId,
        resume: PcKey,
        result: SlotId,
    ) -> Result<(), Diagnostic> {
        let (function, sealed) = self
            .frame
            .facade_sealed
            .take()
            .ok_or_else(|| facade_fault("facade Call exit has no sealed facade call"))?;
        if sealed != site.0 || site.0 != self.frame.pc().node {
            return Err(call_fault("facade Call site changed after sealing"));
        }
        let consumed = action.consume(
            &self.frame.actions,
            self.frame.pc(),
            site,
            &resume,
            result,
        )?;
        let packet = self
            .frame
            .call_arguments
            .take()
            .ok_or_else(|| call_fault("facade Call exit has no argument packet"))?;
        let identity = self.frame.call_identity(site.0);
        self.frame.facade_calls.remove(&identity);
        let tickets = packet.take_tickets(&mut self.store)?;
        let outcome = execute(
            function,
            &tickets,
            &mut self.store,
            &mut self.output,
            &mut self.format_sinks,
        );
        let keep = match &outcome {
            Ok(FacadeOutcome::Value(ticket)) => Some(*ticket),
            _ => None,
        };
        for ticket in tickets {
            if Some(ticket) != keep {
                self.store.discard(ticket)?;
            }
        }
        match outcome? {
            FacadeOutcome::Value(ticket) => {
                self.frame
                    .resume_call(consumed.resume, consumed.result, ticket, &mut self.store)
            }
            FacadeOutcome::Panic(message) => {
                self.frame.panic_in_suspended_call()?;
                self.begin_panic_cleanup(crate::vm::step::GuestPanic {
                    kind: crate::vm::numeric::PanicKind::Explicit,
                    message: Some(message),
                    pc: self.frame.pc().clone(),
                    origin: consumed.origin,
                })
            }
        }
    }
}

fn execute(
    function: LibraryFunction,
    tickets: &[TicketId],
    store: &mut GuestStore,
    output: &mut GuestOutput,
    sinks: &mut super::format_calls::FormatSinks,
) -> Result<FacadeOutcome, Diagnostic> {
    let argument = || {
        tickets
            .first()
            .copied()
            .ok_or_else(|| facade_fault("facade argument absent"))
    };
    match function {
        LibraryFunction::Panic | LibraryFunction::PanicStr2015 => Ok(FacadeOutcome::Panic(
            store.str_reference_contents(argument()?)?.to_owned(),
        )),
        LibraryFunction::BeginPanic => {
            let value = store.peek_ticket(argument()?)?;
            // std's default hook prints &str/String payloads, otherwise
            // `Box<dyn Any>`.
            let message = match value.kind {
                9 => match store.reference_view(value)? {
                    ReferenceView::Str(text) => text.to_owned(),
                    ReferenceView::Value(_) => "Box<dyn Any>".to_owned(),
                },
                _ => "Box<dyn Any>".to_owned(),
            };
            Ok(FacadeOutcome::Panic(message))
        }
        LibraryFunction::PanicDisplay => {
            let value = store.peek_ticket(argument()?)?;
            Ok(FacadeOutcome::Panic(display_through_reference(store, value)?))
        }
        LibraryFunction::UnreachableDisplay => {
            let value = store.peek_ticket(argument()?)?;
            Ok(FacadeOutcome::Panic(format!(
                "internal error: entered unreachable code: {}",
                display_through_reference(store, value)?
            )))
        }
        LibraryFunction::MustUse => Ok(FacadeOutcome::Value(argument()?)),
        // fmt::Arguments is the rendered str buffer of Op::FormatArguments.
        LibraryFunction::PanicFmt => Ok(FacadeOutcome::Panic(
            store.str_reference_contents(argument()?)?.to_owned(),
        )),
        LibraryFunction::Print | LibraryFunction::Eprint => {
            let text = store.str_reference_contents(argument()?)?;
            if function == LibraryFunction::Print {
                output.stdout.push_str(text);
            } else {
                output.stderr.push_str(text);
            }
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unit())?))
        }
        LibraryFunction::Format => Err(facade_fault(
            "alloc::fmt::format awaits the String library facade",
        )),
        LibraryFunction::AssertFailed => {
            // core::panicking::assert_failed_inner's message.
            let [kind, left, right, arguments] = tickets else {
                return Err(facade_fault("assert_failed arguments differ"));
            };
            let op = match store.aggregate_variant(*kind)? {
                Some(0) => "==",
                Some(1) => "!=",
                Some(2) => "matches",
                _ => return Err(facade_fault("assertion kind value differs")),
            };
            let debug = |store: &GuestStore, ticket: TicketId| -> Result<String, Diagnostic> {
                let value = store.peek_ticket(ticket)?;
                let template = crate::ir::format::FormatTemplate::parse("{:?}", 1, &[])?;
                crate::ir::format::render(
                    &template,
                    &[crate::vm::frame::format_value(store, value)?],
                )
            };
            let left = debug(store, *left)?;
            let right = debug(store, *right)?;
            // Option<fmt::Arguments>: None, or Some(rendered arguments).
            let message = match store.aggregate_variant(*arguments)? {
                Some(0) => None,
                Some(1) => {
                    let cell = store.aggregate_field(*arguments, 0)?;
                    let value = store.read_scalar(cell)?;
                    match store.reference_view(value)? {
                        ReferenceView::Str(text) => Some(text.to_owned()),
                        ReferenceView::Value(_) => {
                            return Err(facade_fault("assertion message differs"));
                        }
                    }
                }
                _ => return Err(facade_fault("assertion message option differs")),
            };
            Ok(FacadeOutcome::Panic(match message {
                Some(message) => format!(
                    "assertion `left {op} right` failed: {message}\n  left: {left}\n right: {right}"
                ),
                None => format!(
                    "assertion `left {op} right` failed\n  left: {left}\n right: {right}"
                ),
            }))
        }
        // `<[T]>::len` / `str::len`: the referent's element or byte count.
        LibraryFunction::SliceLen | LibraryFunction::StrLen => {
            let value = store.peek_ticket(argument()?)?;
            let len = store.referent_len(value)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unsigned(
                crate::runtime::shape::TypeId(13),
                len,
            )?)?))
        }
        // `<str as PartialEq>::eq`: byte equality of the two referents.
        LibraryFunction::StrEq => {
            let [left, right] = tickets else {
                return Err(facade_fault("str equality arguments differ"));
            };
            let equal = store.str_reference_contents(*left)? == store.str_reference_contents(*right)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::boolean(equal))?))
        }
        // `Formatter::write_str`: append to the request's buffer.
        LibraryFunction::FmtWrite => {
            let [sink, text] = tickets else {
                return Err(facade_fault("formatter write arguments differ"));
            };
            let sink = store.peek_ticket(*sink)?.payload;
            sinks.append(sink, store.str_reference_contents(*text)?)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unit())?))
        }
        // A primitive `fmt` (or `Formatter::pad`): the native rendering under
        // the formatter's options.
        LibraryFunction::FmtValue => {
            let [value, format, sink, spec, fill, width, precision] = tickets else {
                return Err(facade_fault("formatter value arguments differ"));
            };
            let scalar = |ticket: &TicketId| -> Result<u64, Diagnostic> {
                Ok(store.peek_ticket(*ticket)?.payload)
            };
            let (format, sink, spec) = (scalar(format)?, scalar(sink)?, scalar(spec)?);
            let fill = char::from_u32(
                u32::try_from(scalar(fill)?).map_err(|_| facade_fault("fill is not a char"))?,
            )
            .ok_or_else(|| facade_fault("fill is not a char"))?;
            let placeholder = formatter_placeholder(format, spec, fill)?;
            let count = |set: u64, ticket: &TicketId| -> Result<Option<usize>, Diagnostic> {
                Ok(if spec & set != 0 {
                    Some(
                        usize::try_from(scalar(ticket)?)
                            .map_err(|_| facade_fault("format count overflow"))?,
                    )
                } else {
                    None
                })
            };
            let rendered = crate::ir::format::render_placeholder(
                &placeholder,
                count(16, width)?,
                count(32, precision)?,
                &crate::vm::frame::format_value(store, store.peek_ticket(*value)?)?,
            )?;
            sinks.append(sink, &rendered)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unit())?))
        }
        LibraryFunction::FmtSink => {
            let sink = sinks.open();
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unsigned(
                crate::runtime::shape::TypeId(13),
                sink as u64,
            )?)?))
        }
        // core::fmt::builders::PadAdapter: every line the nested output
        // starts is indented by four spaces.
        LibraryFunction::FmtPadAdapter => {
            let [parent, nested] = tickets else {
                return Err(facade_fault("pad adapter arguments differ"));
            };
            let parent = store.peek_ticket(*parent)?.payload;
            let text = sinks.take(store.peek_ticket(*nested)?.payload)?;
            let mut padded = String::with_capacity(text.len());
            let mut on_newline = true;
            for line in text.split_inclusive('\n') {
                if on_newline {
                    padded.push_str("    ");
                }
                on_newline = line.ends_with('\n');
                padded.push_str(line);
            }
            sinks.append(parent, &padded)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unit())?))
        }
        LibraryFunction::Unsupported => Err(Diagnostic::new(
            "LRR-E-FACADE-MISSING",
            format!(
                "library item `{}` is declared by the LRR facade but not executable yet",
                store.str_reference_contents(argument()?)?
            ),
        )),
    }
}

/// `<&T as Display>` for the closed scalar/str family.
fn display_through_reference(store: &GuestStore, value: ValueRepr) -> Result<String, Diagnostic> {
    if value.kind != 9 {
        return Err(facade_fault("Display argument is not a shared reference"));
    }
    match store.reference_view(value)? {
        ReferenceView::Str(text) => Ok(text.to_owned()),
        ReferenceView::Value(inner) if inner.kind == 9 => display_through_reference(store, inner),
        ReferenceView::Value(inner) => display_scalar(store, inner),
    }
}

pub(super) fn display_scalar(store: &GuestStore, value: ValueRepr) -> Result<String, Diagnostic> {
    let _ = store;
    Ok(match value.kind {
        2 => (value.payload == 1).to_string(),
        3 => char::from_u32(u32::try_from(value.payload).map_err(|_| facade_fault("char"))?)
            .ok_or_else(|| facade_fault("invalid char payload"))?
            .to_string(),
        4 => (value.payload as i64).to_string(),
        5 => value.payload.to_string(),
        6 => f32::from_bits(value.payload as u32).to_string(),
        7 => f64::from_bits(value.payload).to_string(),
        _ => {
            return Err(facade_fault(
                "Display of this value awaits trait-method formatting",
            ));
        }
    })
}

/// The placeholder a facade `Formatter`'s options describe (see
/// `core::fmt::Formatter` in the facade): `format` is the trait code of the
/// primitive `fmt` impl (0 Display, 1 Debug, 4 LowerHex, 5 UpperHex,
/// 6 Octal, 7 Binary, 8 LowerExp, 9 UpperExp).
fn formatter_placeholder(
    format: u64,
    spec: u64,
    fill: char,
) -> Result<crate::ir::format::Placeholder, Diagnostic> {
    use crate::ir::format::{Align, Count, FormatTrait, Placeholder};
    let format = match format {
        0 => FormatTrait::Display,
        1 if spec & 64 != 0 => FormatTrait::LowerHexDebug,
        1 if spec & 128 != 0 => FormatTrait::UpperHexDebug,
        1 => FormatTrait::Debug,
        4 => FormatTrait::LowerHex,
        5 => FormatTrait::UpperHex,
        6 => FormatTrait::Octal,
        7 => FormatTrait::Binary,
        8 => FormatTrait::LowerExp,
        9 => FormatTrait::UpperExp,
        _ => return Err(facade_fault("formatter trait code differs")),
    };
    Ok(Placeholder {
        argument: 0,
        fill,
        align: match (spec >> 8) & 3 {
            1 => Some(Align::Left),
            2 => Some(Align::Right),
            3 => Some(Align::Center),
            _ => None,
        },
        plus: spec & 1 != 0,
        minus: spec & 2 != 0,
        alternate: spec & 4 != 0,
        zero: spec & 8 != 0,
        // Resolved by the caller; the counts are never re-read here.
        width: (spec & 16 != 0).then_some(Count::Is(0)),
        precision: (spec & 32 != 0).then_some(Count::Is(0)),
        format,
    })
}
