//! Trait-method formatting, frame side. `format_args!` renders the closed
//! primitive/str family natively; a placeholder whose argument is any other
//! value formats through its trait's `fmt` impl, as rustc's
//! `Argument::new_display(&x)` does. The `FormatArguments` operation then
//! suspends (a format activation, resumable like an operator activation) and
//! demands that call per placeholder, in template order. The dispatcher
//! enters the facade's `core::fmt::lrr_format_<trait>::<T>` with `&argument`,
//! a fresh output buffer and the placeholder's options; that ordinary source
//! function builds the `Formatter` and selects `<T as Trait>::fmt` through
//! the ordinary trait-call route (`engine/run_format_calls.rs`). Each
//! returned buffer becomes its placeholder's text; when none remains the
//! template renders and the operation completes.
use super::*;
use crate::{
    ir::{
        format::{Align, Count, FormatTemplate, FormatTrait, FormatValue, Piece, Placeholder},
        op::Op,
    },
    runtime::{place::Place, reference::Access, value::ValueRepr},
    traits::term::{RegionTerm, TyTerm},
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Stage {
    /// Between trait calls (or before the first).
    Ready,
    Demanded,
    Entered,
}
/// One argument of the demanded formatting entry, in parameter order.
#[derive(Debug, Clone)]
pub(crate) struct FormatArgument {
    pub(crate) cell: CellId,
    pub(crate) source: TyTerm,
    pub(crate) descriptor: TypeId,
}
/// A suspended `FormatArguments` operation.
pub(super) struct FormatActivation {
    pc: crate::core::digest::Digest,
    operation_index: u32,
    expected: Op,
    /// Rendered text of each placeholder piece formatted through its trait.
    rendered: BTreeMap<usize, String>,
    stage: Stage,
    /// The placeholder piece and trait of the demanded or entered call.
    piece: usize,
    format: FormatTrait,
    /// The demanded entry's arguments: `&value`, then the placeholder's
    /// options (sink placeholder, spec, fill, width, precision).
    arguments: Vec<FormatArgument>,
    /// Dispatcher-owned locations, retired after each call.
    owned: Vec<CellId>,
}
/// The dispatcher's view of a demanded formatting call.
pub(crate) struct FormatDemand {
    pub(crate) format: FormatTrait,
    pub(crate) arguments: Vec<FormatArgument>,
}

/// Native rendering of `cell`, or `None` when it formats through its trait.
fn native_value(store: &GuestStore, cell: CellId) -> Result<Option<FormatValue>, Diagnostic> {
    match string_literal_frame::format_cell(store, cell) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.code == string_literal_frame::FORMAT_TRAIT_CODE => Ok(None),
        Err(error) => Err(error),
    }
}
/// The placeholder's options as the facade `Formatter` holds them: `spec`
/// bits 1 `+`, 2 `-`, 4 `#`, 8 `0`, 16 width set, 32 precision set, 64 `x?`,
/// 128 `X?`, bits 8..10 the alignment.
fn spec_of(
    p: &Placeholder,
    values: &[Option<FormatValue>],
) -> Result<(u64, char, usize, usize), Diagnostic> {
    let count = |count: Option<Count>| -> Result<Option<usize>, Diagnostic> {
        Ok(match count {
            None => None,
            Some(Count::Is(n)) => Some(n),
            Some(Count::Argument(i)) => match values.get(i) {
                Some(Some(FormatValue::Unsigned(n, _))) => Some(
                    usize::try_from(*n).map_err(|_| invalid("format count overflow"))?,
                ),
                _ => {
                    return Err(Diagnostic::new(
                        "LRR-E-TYPE-CONTEXT",
                        "format width/precision argument is not usize".into(),
                    ));
                }
            },
        })
    };
    let width = count(p.width)?;
    let precision = count(p.precision)?;
    let mut spec = 0u64;
    for (set, bit) in [
        (p.plus, 1),
        (p.minus, 2),
        (p.alternate, 4),
        (p.zero, 8),
        (width.is_some(), 16),
        (precision.is_some(), 32),
        (p.format == FormatTrait::LowerHexDebug, 64),
        (p.format == FormatTrait::UpperHexDebug, 128),
    ] {
        if set {
            spec |= bit;
        }
    }
    spec |= match p.align {
        None => 0,
        Some(Align::Left) => 1,
        Some(Align::Right) => 2,
        Some(Align::Center) => 3,
    } << 8;
    Ok((spec, p.fill, width.unwrap_or(0), precision.unwrap_or(0)))
}

impl Frame {
    pub(crate) fn has_format_activation(&self) -> bool {
        self.format_activation.is_some()
    }
    /// Execute or resume `FormatArguments` at operation `index`: `true` when
    /// it completed, `false` when it demands a trait call first.
    pub(crate) fn format_arguments_step(
        &mut self,
        index: usize,
        op: &Op,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        let Op::FormatArguments {
            template,
            arguments,
            destination,
        } = op
        else {
            return Err(invalid("format step requires a format operation"));
        };
        if let Some(activation) = &self.format_activation {
            if activation.pc != self.pc.fingerprint()
                || activation.operation_index as usize != index
                || activation.expected != *op
            {
                return Err(invalid("format call resumed at a different operation"));
            }
            if activation.stage != Stage::Ready {
                return Err(invalid("format operation resumed before its call returned"));
            }
        }
        let mut values = Vec::with_capacity(arguments.len());
        for slot in arguments {
            values.push(native_value(store, self.cell(*slot)?)?);
        }
        for (piece, entry) in template.pieces.iter().enumerate() {
            let Piece::Placeholder(placeholder) = entry else {
                continue;
            };
            let pending = values
                .get(placeholder.argument)
                .ok_or_else(|| invalid("format argument absent"))?
                .is_none()
                && !self
                    .format_activation
                    .as_ref()
                    .is_some_and(|activation| activation.rendered.contains_key(&piece));
            if pending {
                let slot = arguments[placeholder.argument];
                self.demand_format_call(index, op, piece, placeholder, slot, &values, store)?;
                return Ok(false);
            }
        }
        let rendered = self
            .format_activation
            .take()
            .map(|activation| activation.rendered)
            .unwrap_or_default();
        let text = render_template(template, &values, &rendered)?;
        // The autoref'd place arguments were borrowed for this operation
        // only; they are no operator's operands.
        self.autoref_operands.retain(|slot| !arguments.contains(slot));
        let ticket = store.immutable_str_ticket(text.as_bytes())?;
        let ty = TypeId(store.peek_ticket(ticket)?.type_id);
        if let Err(error) = self.preflight_write_type(*destination, ty, store) {
            store.discard(ticket)?;
            return Err(error);
        }
        self.write(*destination, ticket, store)
            .map(|()| true)
    }
    #[allow(clippy::too_many_arguments)]
    fn demand_format_call(
        &mut self,
        index: usize,
        op: &Op,
        piece: usize,
        placeholder: &Placeholder,
        slot: SlotId,
        values: &[Option<FormatValue>],
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        if self.operator_activation.is_some()
            || self.copy_activation.is_some()
            || self.cleanup_activation.is_some()
        {
            return Err(invalid("format call overlaps another activation"));
        }
        let (spec, fill, width, precision) = spec_of(placeholder, values)?;
        if self.format_activation.is_none() {
            self.format_activation = Some(FormatActivation {
                pc: self.pc.fingerprint(),
                operation_index: u32::try_from(index)
                    .map_err(|_| invalid("format operation index overflow"))?,
                expected: op.clone(),
                rendered: BTreeMap::new(),
                stage: Stage::Ready,
                piece,
                format: placeholder.format,
                arguments: Vec::new(),
                owned: Vec::new(),
            });
        }
        let mut owned = Vec::new();
        let mut arguments = Vec::with_capacity(6);
        // `&argument`: an autoref'd place argument already holds it; a
        // value argument's temporary is borrowed in place.
        let cell = self.cell(slot)?;
        let value = store.admit_move(cell)?;
        let ty = store.effective_type(value)?.ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "format argument requires its reached concrete type".into(),
            )
        })?;
        let view = |frame: &Self, store: &GuestStore| -> Result<TyTerm, Diagnostic> {
            match frame.source_view(slot)? {
                Some(view) => Ok(view),
                None => store.descriptor_source_term(ty).ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        "format argument requires its closed source type".into(),
                    )
                }),
            }
        };
        if self.autoref_operands.contains(&slot) {
            // The packet moves its argument: the shared reference is copied
            // into an owned location, so later placeholders still read it.
            let source = view(self, store)?;
            let copy = store.copy_from(cell)?;
            let owner = self.format_owned_cell(ty, 0, store)?;
            store.initialize(owner, copy)?;
            owned.push(owner);
            arguments.push(FormatArgument {
                cell: owner,
                source,
                descriptor: ty,
            });
        } else {
            let pointee = view(self, store)?;
            let reference = store
                .borrow_ticket(&Place::cell(cell), Access::Shared)
                .map_err(|error| match error {
                    crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                    crate::runtime::place::PlaceFailure::GuestPanic(_) => {
                        invalid("format argument borrow reported a bounds panic")
                    }
                })?;
            let descriptor = TypeId(store.peek_ticket(reference)?.type_id);
            let owner = self.format_owned_cell(descriptor, 0, store)?;
            store.initialize(owner, reference)?;
            owned.push(owner);
            let range = self
                .body
                .nodes()
                .get(self.pc.node as usize)
                .ok_or_else(|| invalid("format node absent"))?
                .range;
            arguments.push(FormatArgument {
                cell: owner,
                source: TyTerm::Ref(
                    RegionTerm::Local {
                        definition: self.body.body().definition,
                        start: range.start,
                        end: range.end,
                    },
                    false,
                    Box::new(pointee),
                ),
                descriptor,
            });
        }
        // The options, as owned primitive locations. The sink location stays
        // uninitialized until the dispatcher enters the call
        // (`set_format_sink`).
        let usize_ty = TypeId(13);
        for (ordinal, value) in [
            (1, None),
            (2, Some(ValueRepr::unsigned(TypeId(12), spec)?)),
            (3, Some(ValueRepr::character(fill))),
            (4, Some(ValueRepr::unsigned(usize_ty, width as u64)?)),
            (5, Some(ValueRepr::unsigned(usize_ty, precision as u64)?)),
        ] {
            let descriptor = value.map_or(usize_ty, |value| TypeId(value.type_id));
            let owner = self.format_owned_cell(descriptor, ordinal, store)?;
            if let Some(value) = value {
                let ticket = store.ticket(value)?;
                store.initialize(owner, ticket)?;
            }
            owned.push(owner);
            arguments.push(FormatArgument {
                cell: owner,
                source: store.descriptor_source_term(descriptor).ok_or_else(|| {
                    invalid("format option descriptor has no source type")
                })?,
                descriptor,
            });
        }
        let activation = self
            .format_activation
            .as_mut()
            .ok_or_else(|| invalid("format activation absent"))?;
        activation.piece = piece;
        activation.format = placeholder.format;
        activation.arguments = arguments;
        activation.owned = owned;
        activation.stage = Stage::Demanded;
        Ok(())
    }
    fn format_owned_cell(
        &self,
        descriptor: TypeId,
        ordinal: u32,
        store: &mut GuestStore,
    ) -> Result<CellId, Diagnostic> {
        store.new_cell(
            StorageContract::Exact(descriptor),
            false,
            CellOwner::FrameSlot {
                frame: self.anchor,
                slot: self
                    .owned_location_base()?
                    .checked_add(8 + ordinal)
                    .ok_or_else(|| invalid("format location overflow"))?,
            },
        )
    }
    /// The demanded call, until the dispatcher enters it.
    pub(crate) fn format_demand(&self) -> Option<FormatDemand> {
        self.format_activation
            .as_ref()
            .filter(|activation| activation.stage == Stage::Demanded)
            .map(|activation| FormatDemand {
                format: activation.format,
                arguments: activation.arguments.clone(),
            })
    }
    /// The suspended format operation a resumed recipe continues at.
    pub(crate) fn format_operation_index(&self) -> Option<usize> {
        self.format_activation
            .as_ref()
            .map(|activation| activation.operation_index as usize)
    }
    pub(crate) fn format_demanded(&self) -> bool {
        self.format_activation
            .as_ref()
            .is_some_and(|activation| activation.stage == Stage::Demanded)
    }
    /// Store the dispatcher's output buffer in the demanded call's sink
    /// argument before the packet seals it.
    pub(crate) fn set_format_sink(
        &mut self,
        sink: usize,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let activation = self
            .format_activation
            .as_ref()
            .filter(|activation| activation.stage == Stage::Demanded)
            .ok_or_else(|| invalid("format sink requires a demanded call"))?;
        let cell = activation
            .arguments
            .get(1)
            .ok_or_else(|| invalid("format sink argument absent"))?
            .cell;
        let ticket = store.ticket(ValueRepr::unsigned(TypeId(13), sink as u64)?)?;
        store.initialize(cell, ticket)
    }
    /// Enter the demanded entry: the callee frame shares this run's step
    /// and action observation, like any callee entered from this frame.
    pub(crate) fn enter_format_call(
        &mut self,
        body: Arc<BodyIndex>,
        entry: PcKey,
        store: &mut GuestStore,
    ) -> Result<Self, Diagnostic> {
        self.require_store(store)?;
        let activation = self
            .format_activation
            .as_mut()
            .filter(|activation| activation.stage == Stage::Demanded)
            .ok_or_else(|| invalid("format call entry requires a demanded call"))?;
        activation.stage = Stage::Entered;
        let mut frame = Self::new(body, entry, store)?;
        frame.fuel = self.fuel.share();
        frame.actions = self.actions.share();
        Ok(frame)
    }
    pub(crate) fn format_call_entered(&self) -> bool {
        self.format_activation
            .as_ref()
            .is_some_and(|activation| activation.stage == Stage::Entered)
    }
    /// The entry returned `Ok`: its buffer is the placeholder's text.
    /// The entry's `fmt::Result` is a temporary of this operation: it is
    /// dropped through the ordinary cleanup machine from an owned location.
    /// `fmt::Result` has no `Drop::drop` witness to demand.
    pub(crate) fn drop_format_result(
        &mut self,
        ticket: TicketId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let value = store.peek_ticket(ticket)?;
        let ty = store
            .effective_type(value)?
            .ok_or_else(|| invalid("format result lacks its concrete type"))?;
        let cell = self.format_owned_cell(ty, 7, store)?;
        store.initialize(cell, ticket)?;
        let mut machine = crate::vm::cleanup::CleanupMachine::begin(cell, store, &mut self.fuel)?;
        loop {
            match machine.advance(store, &mut self.fuel)? {
                crate::vm::cleanup::CleanupProgress::Advanced => {}
                crate::vm::cleanup::CleanupProgress::Complete => break,
                crate::vm::cleanup::CleanupProgress::NeedWitness { .. } => {
                    return Err(invalid("fmt::Result demanded a Drop witness"));
                }
            }
        }
        let completed = self
            .counters
            .cleanup_elements_completed
            .checked_add(machine.completed_owners())
            .ok_or_else(crate::runtime::arena::resource)?;
        self.counters.cleanup_elements_completed = completed;
        Ok(())
    }
    pub(crate) fn finish_format_call(
        &mut self,
        text: String,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let activation = self
            .format_activation
            .as_mut()
            .filter(|activation| activation.stage == Stage::Entered)
            .ok_or_else(|| invalid("format result requires an entered call"))?;
        activation.rendered.insert(activation.piece, text);
        activation.stage = Stage::Ready;
        activation.arguments.clear();
        for cell in std::mem::take(&mut activation.owned) {
            store.retire_cell(cell)?;
        }
        Ok(())
    }
    /// The entry unwound (or returned `Err`): this operation becomes the
    /// panicking operation.
    pub(crate) fn unwind_format_call(&mut self, store: &mut GuestStore) -> Result<(), Diagnostic> {
        let activation = self
            .format_activation
            .take()
            .filter(|activation| activation.stage == Stage::Entered)
            .ok_or_else(|| invalid("format unwind requires an entered call"))?;
        for cell in activation.owned {
            store.retire_cell(cell)?;
        }
        self.status = FrameStatus::Panicked;
        Ok(())
    }
}

/// Render `template` with the native argument values, substituting each
/// trait-formatted placeholder's returned text.
fn render_template(
    template: &FormatTemplate,
    values: &[Option<FormatValue>],
    rendered: &BTreeMap<usize, String>,
) -> Result<String, Diagnostic> {
    let mut out = String::new();
    for (piece, entry) in template.pieces.iter().enumerate() {
        match entry {
            Piece::Literal(text) => out.push_str(text),
            Piece::Placeholder(placeholder) => {
                if let Some(text) = rendered.get(&piece) {
                    out.push_str(text);
                    continue;
                }
                let (_, _, width, precision) = spec_of(placeholder, values)?;
                let value = values
                    .get(placeholder.argument)
                    .and_then(Option::as_ref)
                    .ok_or_else(|| invalid("format argument has no rendering"))?;
                out.push_str(&crate::ir::format::render_placeholder(
                    placeholder,
                    placeholder.width.map(|_| width),
                    placeholder.precision.map(|_| precision),
                    value,
                )?);
            }
        }
    }
    Ok(out)
}
