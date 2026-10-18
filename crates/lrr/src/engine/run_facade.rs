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
        if self.pending.contains_key(&identity) {
            return QueryPoll::Ready(Ok(false));
        }
        // A relative callee a block-level import binds is selected by its
        // entered lexical Value goal, as the written library path is below.
        if !library_candidate(name) {
            return match self.lexical_facade_call(db, site, name) {
                QueryPoll::Ready(Ok(Some((function, definition)))) => {
                    self.frame
                        .facade_calls
                        .insert(identity, (function, definition));
                    QueryPoll::Ready(Ok(true))
                }
                QueryPoll::Ready(Ok(None)) => QueryPoll::Ready(Ok(false)),
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            };
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
        let consumed =
            action.consume(&self.frame.actions, self.frame.pc(), site, &resume, result)?;
        let packet = self
            .frame
            .call_arguments
            .take()
            .ok_or_else(|| call_fault("facade Call exit has no argument packet"))?;
        let identity = self.frame.call_identity(site.0);
        self.frame.facade_calls.remove(&identity);
        let tickets = packet.take_tickets(&mut self.store)?;
        let mut moved_in = Vec::new();
        // A type-argument intrinsic reads the enclosing function's type
        // parameters (`size_of::<T>` is `size_of<T>() { lrr_size_of() }`).
        let type_arguments = self
            .frame
            .enclosing_type_arguments(function.type_arguments())?;
        let outcome = execute(
            function,
            &tickets,
            &type_arguments,
            &mut self.store,
            &mut self.output,
            &mut self.format_sinks,
            &mut moved_in,
        );
        let keep = match &outcome {
            Ok(FacadeOutcome::Value(ticket)) => Some(*ticket),
            _ => None,
        };
        for ticket in tickets {
            if Some(ticket) != keep && !moved_in.contains(&ticket) {
                self.store.discard(ticket)?;
            }
        }
        match outcome? {
            FacadeOutcome::Value(ticket) => {
                // An owning or reference result keeps its closed source view
                // (the descriptor records it exactly), as a constructed
                // nominal value does; scalars need none.
                let value = self.store.peek_ticket(ticket)?;
                let view = match value.kind {
                    8 | 9 => self
                        .store
                        .effective_type(value)?
                        .and_then(|ty| self.frame.constructed_source(ty, &self.store)),
                    _ => None,
                };
                self.frame.resume_call(
                    consumed.resume,
                    consumed.result,
                    ticket,
                    &mut self.store,
                )?;
                match view {
                    Some(view) => self.frame.set_source_view(consumed.result, Some(view)),
                    None => Ok(()),
                }
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

/// The host libm/IEEE operation a `core::intrinsics::lrr_f{32,64}_math` code
/// names, for both float widths (the bodies of `f32`/`f64` `sqrt`, `sin`,
/// `floor`, ...). The codes are the constants of `facade/core/float_impls.rs`.
macro_rules! host_float_math {
    ($op:expr, $a:expr, $b:expr, $c:expr) => {
        match $op {
            0 => Some($a.sqrt()),
            1 => Some($a.floor()),
            2 => Some($a.ceil()),
            3 => Some($a.round()),
            4 => Some($a.trunc()),
            5 => Some($a.round_ties_even()),
            6 => Some($a.exp()),
            7 => Some($a.exp2()),
            8 => Some($a.ln()),
            9 => Some($a.log10()),
            10 => Some($a.log2()),
            11 => Some($a.sin()),
            12 => Some($a.cos()),
            13 => Some($a.tan()),
            14 => Some($a.asin()),
            15 => Some($a.acos()),
            16 => Some($a.atan()),
            17 => Some($a.sinh()),
            18 => Some($a.cosh()),
            19 => Some($a.tanh()),
            20 => Some($a.asinh()),
            21 => Some($a.acosh()),
            22 => Some($a.atanh()),
            23 => Some($a.cbrt()),
            24 => Some($a.exp_m1()),
            25 => Some($a.ln_1p()),
            26 => Some($a.powf($b)),
            27 => Some($a.atan2($b)),
            28 => Some($a.hypot($b)),
            29 => Some($a.mul_add($b, $c)),
            30 => Some($a.min($b)),
            31 => Some($a.max($b)),
            32 => Some($a.copysign($b)),
            33 => Some($a.abs()),
            _ => None,
        }
    };
}

/// `moved_in` collects argument tickets the intrinsic moved into the store
/// itself (a value pushed into a vector); the caller discards the rest.
fn execute(
    function: LibraryFunction,
    tickets: &[TicketId],
    type_arguments: &[crate::runtime::shape::TypeId],
    store: &mut GuestStore,
    output: &mut GuestOutput,
    sinks: &mut super::format_calls::FormatSinks,
    moved_in: &mut Vec<TicketId>,
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
            Ok(FacadeOutcome::Panic(display_through_reference(
                store, value,
            )?))
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
                None => {
                    format!("assertion `left {op} right` failed\n  left: {left}\n right: {right}")
                }
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
            let equal =
                store.str_reference_contents(*left)? == store.str_reference_contents(*right)?;
            Ok(FacadeOutcome::Value(
                store.ticket(ValueRepr::boolean(equal))?,
            ))
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
        // The text a formatting request's output buffer holds (`to_string`).
        LibraryFunction::FmtTake => {
            let sink = store.peek_ticket(argument()?)?.payload;
            let text = sinks.take(sink)?;
            Ok(FacadeOutcome::Value(
                store.immutable_str_ticket(text.as_bytes())?,
            ))
        }
        // One byte of a str referent.
        LibraryFunction::StrByte => {
            let [text, index] = tickets else {
                return Err(facade_fault("str byte arguments differ"));
            };
            let index = usize::try_from(store.peek_ticket(*index)?.payload)
                .map_err(|_| facade_fault("str byte index overflow"))?;
            let byte = *store
                .str_reference_contents(*text)?
                .as_bytes()
                .get(index)
                .ok_or_else(|| facade_fault("str byte index is out of range"))?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unsigned(
                crate::runtime::shape::TypeId(9),
                u64::from(byte),
            )?)?))
        }
        LibraryFunction::VecLen => {
            let vector = store.peek_ticket(argument()?)?;
            let len = store.vec_len(vector)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unsigned(
                crate::runtime::shape::TypeId(13),
                len,
            )?)?))
        }
        LibraryFunction::VecPush | LibraryFunction::VecInsert => {
            let (vector, index, element) = match tickets {
                [vector, element] if function == LibraryFunction::VecPush => {
                    (*vector, None, *element)
                }
                [vector, index, element] if function == LibraryFunction::VecInsert => {
                    (*vector, Some(store.peek_ticket(*index)?.payload), *element)
                }
                _ => return Err(facade_fault("vector insertion arguments differ")),
            };
            let reference = store.peek_ticket(vector)?;
            store.vec_insert(reference, index, element)?;
            moved_in.push(element);
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unit())?))
        }
        LibraryFunction::VecRemove => {
            let [vector, index] = tickets else {
                return Err(facade_fault("vector removal arguments differ"));
            };
            let reference = store.peek_ticket(*vector)?;
            let index = store.peek_ticket(*index)?.payload;
            Ok(FacadeOutcome::Value(store.vec_remove(reference, index)?))
        }
        LibraryFunction::VecAsSlice | LibraryFunction::VecAsMutSlice => {
            let reference = store.peek_ticket(argument()?)?;
            Ok(FacadeOutcome::Value(store.vec_slice(
                reference,
                function == LibraryFunction::VecAsMutSlice,
            )?))
        }
        LibraryFunction::SliceSwap => {
            let [slice, a, b] = tickets else {
                return Err(facade_fault("slice swap arguments differ"));
            };
            let slice = store.peek_ticket(*slice)?;
            let (a, b) = (
                store.peek_ticket(*a)?.payload,
                store.peek_ticket(*b)?.payload,
            );
            store.slice_swap(slice, a, b)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unit())?))
        }
        LibraryFunction::VecAppendArray => {
            let [vector, array] = tickets else {
                return Err(facade_fault("vector append arguments differ"));
            };
            let reference = store.peek_ticket(*vector)?;
            store.vec_append_array(reference, *array)?;
            moved_in.push(*array);
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unit())?))
        }
        LibraryFunction::CharFromU32 => {
            let code = u32::try_from(store.peek_ticket(argument()?)?.payload)
                .ok()
                .and_then(char::from_u32)
                .ok_or_else(|| facade_fault("value is not a Unicode scalar value"))?;
            Ok(FacadeOutcome::Value(
                store.ticket(ValueRepr::character(code))?,
            ))
        }
        LibraryFunction::StrSlice => {
            let [text, start, end] = tickets else {
                return Err(facade_fault("str slice arguments differ"));
            };
            let (start, end) = (
                store.peek_ticket(*start)?.payload,
                store.peek_ticket(*end)?.payload,
            );
            Ok(match store.str_slice_ticket(*text, start, end)? {
                Ok(slice) => FacadeOutcome::Value(slice),
                Err(message) => FacadeOutcome::Panic(message),
            })
        }
        LibraryFunction::BytesAsStr => {
            let reference = store.peek_ticket(argument()?)?;
            Ok(FacadeOutcome::Value(store.bytes_as_str(reference)?))
        }
        LibraryFunction::MemReplace | LibraryFunction::CellReplace => {
            let [place, incoming] = tickets else {
                return Err(facade_fault("replacement arguments differ"));
            };
            let reference = store.peek_ticket(*place)?;
            moved_in.push(*incoming);
            Ok(FacadeOutcome::Value(
                if function == LibraryFunction::MemReplace {
                    store.mem_replace(reference, *incoming)?
                } else {
                    store.cell_replace(reference, *incoming)?
                },
            ))
        }
        LibraryFunction::MemSwap => {
            let [a, b] = tickets else {
                return Err(facade_fault("swap arguments differ"));
            };
            let (a, b) = (store.peek_ticket(*a)?, store.peek_ticket(*b)?);
            store.mem_swap(a, b)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unit())?))
        }
        LibraryFunction::AtomicOp => {
            let [reference, incoming, operation, ordering] = tickets else {
                return Err(facade_fault("atomic arguments differ"));
            };
            let reference = store.peek_ticket(*reference)?;
            let incoming = store.peek_ticket(*incoming)?;
            let operation = store.peek_ticket(*operation)?.payload;
            let ordering = store.peek_ticket(*ordering)?.payload;
            Ok(
                match store.atomic_op(reference, incoming, operation, ordering)? {
                    Ok(value) => FacadeOutcome::Value(value),
                    Err(message) => FacadeOutcome::Panic(message.into()),
                },
            )
        }
        LibraryFunction::CellTake => {
            let reference = store.peek_ticket(argument()?)?;
            Ok(FacadeOutcome::Value(store.cell_take(reference)?))
        }
        LibraryFunction::SharedToMut => {
            let reference = store.peek_ticket(argument()?)?;
            Ok(FacadeOutcome::Value(store.shared_to_mut(reference)?))
        }
        LibraryFunction::BoxLeak => {
            let boxed = argument()?;
            moved_in.push(boxed);
            Ok(FacadeOutcome::Value(store.leak_box(boxed)?))
        }
        LibraryFunction::Forget => {
            let value = argument()?;
            moved_in.push(value);
            Ok(FacadeOutcome::Value(store.forget_value(value)?))
        }
        // The IEEE bit pattern of a float and back (`to_bits`/`from_bits`).
        LibraryFunction::F64Bits | LibraryFunction::F32Bits => {
            let value = store.peek_ticket(argument()?)?;
            let (kind, result) = if function == LibraryFunction::F64Bits {
                (7, crate::runtime::shape::TypeId(12))
            } else {
                (6, crate::runtime::shape::TypeId(11))
            };
            if value.kind != kind {
                return Err(facade_fault(
                    "float bits argument is not the expected float",
                ));
            }
            Ok(FacadeOutcome::Value(
                store.ticket(ValueRepr::unsigned(result, value.payload)?)?,
            ))
        }
        LibraryFunction::F64FromBits | LibraryFunction::F32FromBits => {
            let value = store.peek_ticket(argument()?)?;
            if value.kind != 5 {
                return Err(facade_fault(
                    "float from_bits argument is not an unsigned integer",
                ));
            }
            Ok(FacadeOutcome::Value(store.ticket(
                if function == LibraryFunction::F64FromBits {
                    ValueRepr::f64(f64::from_bits(value.payload))
                } else {
                    ValueRepr::f32(f32::from_bits(
                        u32::try_from(value.payload)
                            .map_err(|_| facade_fault("f32 bit pattern exceeds 32 bits"))?,
                    ))
                },
            )?))
        }
        LibraryFunction::F64Math | LibraryFunction::F32Math => {
            let [op, a, b, c] = tickets else {
                return Err(facade_fault("float operation arguments differ"));
            };
            let op = u32::try_from(store.peek_ticket(*op)?.payload)
                .map_err(|_| facade_fault("float operation code overflow"))?;
            let (a, b, c) = (
                store.peek_ticket(*a)?,
                store.peek_ticket(*b)?,
                store.peek_ticket(*c)?,
            );
            let value = if function == LibraryFunction::F64Math {
                if [a.kind, b.kind, c.kind] != [7, 7, 7] {
                    return Err(facade_fault("f64 operation operands are not f64"));
                }
                let (a, b, c) = (
                    f64::from_bits(a.payload),
                    f64::from_bits(b.payload),
                    f64::from_bits(c.payload),
                );
                host_float_math!(op, a, b, c).map(ValueRepr::f64)
            } else {
                if [a.kind, b.kind, c.kind] != [6, 6, 6] {
                    return Err(facade_fault("f32 operation operands are not f32"));
                }
                let (a, b, c) = (
                    f32::from_bits(a.payload as u32),
                    f32::from_bits(b.payload as u32),
                    f32::from_bits(c.payload as u32),
                );
                host_float_math!(op, a, b, c).map(ValueRepr::f32)
            };
            let value = value.ok_or_else(|| facade_fault("unknown float operation code"))?;
            Ok(FacadeOutcome::Value(store.ticket(value)?))
        }
        LibraryFunction::RefEq => {
            let [a, b] = tickets else {
                return Err(facade_fault("reference comparison arguments differ"));
            };
            let (a, b) = (store.peek_ticket(*a)?, store.peek_ticket(*b)?);
            Ok(FacadeOutcome::Value(
                store.ticket(ValueRepr::boolean(store.ref_eq(a, b)?))?,
            ))
        }
        LibraryFunction::VecAsStr => {
            let reference = store.peek_ticket(argument()?)?;
            Ok(FacadeOutcome::Value(store.vec_str(reference)?))
        }
        // Raw pointers (E-R5-008): the address table and place operations
        // live in `runtime/cell_pointers.rs`.
        LibraryFunction::PtrNull | LibraryFunction::PtrNullMut => {
            let pointer = store.raw_pointer_type(
                function == LibraryFunction::PtrNullMut,
                crate::runtime::shape::TypeId(1),
            )?;
            let value = store.null_pointer(pointer)?;
            Ok(FacadeOutcome::Value(store.ticket(value)?))
        }
        LibraryFunction::PtrAsConst | LibraryFunction::PtrAsMut => {
            let pointer = store.peek_ticket(argument()?)?;
            let value =
                store.pointer_with_mutability(pointer, function == LibraryFunction::PtrAsMut)?;
            Ok(FacadeOutcome::Value(store.ticket(value)?))
        }
        LibraryFunction::PtrOffset => {
            let [pointer, count] = tickets else {
                return Err(facade_fault("pointer offset arguments differ"));
            };
            let (pointer, count) = (store.peek_ticket(*pointer)?, store.peek_ticket(*count)?);
            let moved = store.pointer_offset(pointer, count.payload as i64)?;
            Ok(FacadeOutcome::Value(store.ticket(moved)?))
        }
        LibraryFunction::PtrOffsetFrom => {
            let [pointer, origin] = tickets else {
                return Err(facade_fault("pointer distance arguments differ"));
            };
            let (pointer, origin) = (store.peek_ticket(*pointer)?, store.peek_ticket(*origin)?);
            let distance = store.pointer_offset_from(pointer, origin)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::signed(
                crate::runtime::shape::TypeId(8),
                distance,
            )?)?))
        }
        LibraryFunction::PtrRead => {
            let pointer = store.peek_ticket(argument()?)?;
            Ok(FacadeOutcome::Value(store.pointer_read(pointer)?))
        }
        LibraryFunction::PtrWrite => {
            let [pointer, value] = tickets else {
                return Err(facade_fault("pointer write arguments differ"));
            };
            let pointer = store.peek_ticket(*pointer)?;
            moved_in.push(*value);
            store.pointer_write(pointer, *value)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unit())?))
        }
        LibraryFunction::PtrRetype => Ok(FacadeOutcome::Value(argument()?)),
        LibraryFunction::SliceAsPtr | LibraryFunction::SliceAsMutPtr => {
            let reference = store.peek_ticket(argument()?)?;
            let pointer =
                store.elements_pointer(reference, function == LibraryFunction::SliceAsMutPtr)?;
            Ok(FacadeOutcome::Value(store.ticket(pointer)?))
        }
        // `core::mem` layout queries (E-R5-009): one layout algorithm in
        // `runtime/layout.rs`, fed by `runtime/cell_layout.rs`.
        LibraryFunction::SizeOf | LibraryFunction::AlignOf => {
            let [ty] = type_arguments else {
                return Err(facade_fault("layout intrinsic lacks its type argument"));
            };
            let layout = store.layout_of(*ty)?;
            let answer = if function == LibraryFunction::SizeOf {
                layout.size
            } else {
                layout.align
            };
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unsigned(
                crate::runtime::shape::TypeId(13),
                answer,
            )?)?))
        }
        LibraryFunction::NeedsDrop => {
            let [ty] = type_arguments else {
                return Err(facade_fault("needs_drop lacks its type argument"));
            };
            let answer = store.needs_drop_type(*ty)?;
            Ok(FacadeOutcome::Value(
                store.ticket(ValueRepr::boolean(answer))?,
            ))
        }
        LibraryFunction::VariantCount => {
            let [ty] = type_arguments else {
                return Err(facade_fault("variant_count lacks its type argument"));
            };
            let answer = store.variant_count_of(*ty)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unsigned(
                crate::runtime::shape::TypeId(13),
                answer,
            )?)?))
        }
        LibraryFunction::SizeOfVal | LibraryFunction::AlignOfVal => {
            let value = store.peek_ticket(argument()?)?;
            let (size, align) = store.referent_layout(value)?;
            let answer = if function == LibraryFunction::SizeOfVal {
                size
            } else {
                align
            };
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::unsigned(
                crate::runtime::shape::TypeId(13),
                answer,
            )?)?))
        }
        LibraryFunction::DiscriminantValue => {
            let value = store.peek_ticket(argument()?)?;
            let answer = store.referent_discriminant(value)?;
            Ok(FacadeOutcome::Value(store.ticket(ValueRepr::signed(
                crate::runtime::shape::TypeId(8),
                answer,
            )?)?))
        }
        LibraryFunction::Zeroed => {
            let [ty] = type_arguments else {
                return Err(facade_fault("zeroed lacks its type argument"));
            };
            Ok(FacadeOutcome::Value(store.zeroed_value(*ty)?))
        }
        LibraryFunction::Transmute => {
            let [source, destination] = type_arguments else {
                return Err(facade_fault("transmute lacks its type arguments"));
            };
            let ticket = argument()?;
            let value = store.peek_ticket(ticket)?;
            moved_in.push(ticket);
            Ok(FacadeOutcome::Value(store.transmute_value(
                value,
                *source,
                *destination,
            )?))
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
        17 if value.type_id == 17 => (store.wide_integer_bits(value)? as i128).to_string(),
        17 => store.wide_integer_bits(value)?.to_string(),
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
