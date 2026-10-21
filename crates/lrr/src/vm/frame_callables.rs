//! Frame access for calls through a callee value (`TypeKey::Callable`).
//!
//! The callee operand of a value call executes as a place (`MakePlace`,
//! `ParameterPlace` or a temporary `ValuePlace`). The dispatcher inspects its
//! reached type to select the callee and retires the place when the call is
//! sealed; the place never becomes an argument.
use super::{Frame, invalid, operator_calls_frame::OperatorArgument};
use crate::{
    core::diagnostic::Diagnostic,
    ir::{op::PrimitiveType, pc::SlotId},
    runtime::{
        cell::{CellId, CellOwner, GuestStore, IntrinsicRead, StorageContract},
        place::{Place, PlaceFailure},
        reference::Access,
        shape::{TypeId, TypeKey},
        value::ValueRepr,
    },
    traits::term::TyTerm,
};

impl Frame {
    /// The value callee operand's place slot for the call at `site`.
    pub(crate) fn value_callee_place(&self, site: u32) -> Result<SlotId, Diagnostic> {
        let call = self
            .body
            .nodes()
            .get(site as usize)
            .ok_or_else(|| invalid("value call node absent"))?;
        let callee = call
            .operands
            .first()
            .and_then(|id| self.body.nodes().get(id.0 as usize))
            .ok_or_else(|| invalid("value call callee operand absent"))?;
        callee
            .place
            .ok_or_else(|| invalid("value callee place slot absent"))
    }
    /// The executed callee place and its reached descriptor.
    pub(crate) fn value_callee(
        &self,
        site: u32,
        store: &GuestStore,
    ) -> Result<(Place, TypeId), Diagnostic> {
        let slot = self.value_callee_place(site)?;
        let ty = self.place_type(slot, store)?;
        let place = self.executed_place(slot)?;
        let mut projections = Vec::new();
        projections
            .try_reserve_exact(place.projections.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        projections.extend_from_slice(&place.projections);
        Ok((
            Place {
                root: place.root,
                projections: projections.into_boxed_slice(),
            },
            ty,
        ))
    }
    /// Retire the callee place once the call is sealed.
    pub(crate) fn retire_value_callee(&mut self, site: u32) -> Result<(), Diagnostic> {
        let slot = self.value_callee_place(site)?;
        if self.place_slot(slot)?.place.is_some() {
            self.clear_place(slot)?;
        }
        Ok(())
    }
    /// The arguments of the overloaded call at `site` (`Fn::call(&callee,
    /// (a, b))`, `FnMut::call_mut(&mut callee, ..)` or `FnOnce::call_once(
    /// callee, ..)`), formed once its written arguments are evaluated: the
    /// receiver (a shared or unique reference to the callee place, or the
    /// value moved out of it for `access == None`) and the argument tuple the
    /// rust-call ABI spreads the written arguments from. `stated` are the
    /// selected impl's argument Types, typing a still unconstrained numeric
    /// literal. Each lives in a dispatcher-owned location; the returned cells
    /// are retired once the sealed packet has taken their values.
    pub(crate) fn form_overloaded_call_arguments(
        &mut self,
        site: u32,
        access: Option<Access>,
        arguments: &[SlotId],
        stated: &[Option<TypeId>],
        store: &mut GuestStore,
    ) -> Result<(Vec<OperatorArgument>, Vec<CellId>), Diagnostic> {
        fn place_fault(failure: PlaceFailure) -> Diagnostic {
            match failure {
                PlaceFailure::RuntimeFault(error) => error,
                PlaceFailure::GuestPanic(_) => invalid("overloaded callee place panicked"),
            }
        }
        self.require_store(store)?;
        let (callee, callee_type) = self.value_callee(site, store)?;
        let pointee = match self.place_slot(self.value_callee_place(site)?)?.source.clone() {
            Some(view) => view,
            None => store.descriptor_source_term(callee_type).ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "overloaded call callee requires its closed source type".into(),
                )
            })?,
        };
        let base = self.owned_location_base()?;
        let anchor = self.anchor;
        let location = |ordinal: u32| -> Result<CellOwner, Diagnostic> {
            Ok(CellOwner::FrameSlot {
                frame: anchor,
                slot: base
                    .checked_add(ordinal)
                    .ok_or_else(|| invalid("overloaded call location overflow"))?,
            })
        };
        let mut formed = Vec::with_capacity(2);
        let mut cells = Vec::with_capacity(2);
        // The receiver.
        let (receiver, descriptor, source) = match access {
            Some(access) => {
                let reference = store.borrow_ticket(&callee, access).map_err(place_fault)?;
                let descriptor = TypeId(store.peek_ticket(reference)?.type_id);
                let source = self.formed_reference_source(access == Access::Mutable, pointee)?;
                (reference, descriptor, source)
            }
            None => {
                let ticket = store.read_owning_place(&callee).map_err(place_fault)?;
                (ticket, callee_type, pointee)
            }
        };
        let cell = store.new_cell(StorageContract::Exact(descriptor), false, location(10)?)?;
        store.initialize(cell, receiver)?;
        cells.push(cell);
        formed.push(OperatorArgument {
            cell,
            source,
            descriptor,
            slot: None,
        });
        // The argument tuple: each written argument moves (or copies) into
        // its field. An unconstrained numeric literal takes rustc's fallback.
        let mut tickets = Vec::with_capacity(arguments.len());
        let mut members = Vec::with_capacity(arguments.len());
        let mut sources = Vec::with_capacity(arguments.len());
        for (ordinal, slot) in arguments.iter().enumerate() {
            self.materialize_argument_as(*slot, stated.get(ordinal).copied().flatten(), store)?;
            let cell = self.cell(*slot)?;
            let value = store.admit_move(cell)?;
            let ty = store.effective_type(value)?.ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "call argument requires a reached concrete type".into(),
                )
            })?;
            let view = self.argument_source_view(*slot, ty, store)?.ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "overloaded call argument requires its closed source type".into(),
                )
            })?;
            tickets.push(if store.admit_owning_read(cell)? == IntrinsicRead::CopyAggregate {
                store.copy_aggregate_value(cell)?
            } else {
                store.read_owning(cell)?
            });
            members.push(ty);
            sources.push(view);
        }
        let (tuple, descriptor, source) = if members.is_empty() {
            (
                store.ticket(ValueRepr::unit())?,
                TypeId(1),
                TyTerm::Primitive(PrimitiveType::Unit),
            )
        } else {
            let descriptor = store.intern_type(TypeKey::Tuple(members.clone().into_boxed_slice()))?;
            let guard = store.begin_construction(&members)?;
            for (field, ticket) in tickets.into_iter().enumerate() {
                store.construction_push(
                    guard,
                    u32::try_from(field).map_err(|_| invalid("argument ordinal overflow"))?,
                    ticket,
                )?;
            }
            (
                store.commit_construction(guard, descriptor)?,
                descriptor,
                TyTerm::Tuple(sources),
            )
        };
        let cell = store.new_cell(StorageContract::Exact(descriptor), false, location(11)?)?;
        store.initialize(cell, tuple)?;
        cells.push(cell);
        formed.push(OperatorArgument {
            cell,
            source,
            descriptor,
            slot: None,
        });
        Ok((formed, cells))
    }
}
