//! Canonical resumable structural Copy state. No native temporaries or queries.
use super::*;
use crate::{
    ir::op::Op,
    runtime::{
        cell::{
            IntrinsicRead,
            aggregate_copy::{AggregateCopy, CopyProgress, CopyWork},
        },
        place::PlaceFailure,
    },
};

pub(super) struct OpActivation {
    pc: crate::core::digest::Digest,
    operation_index: u32,
    expected: Op,
    copy: AggregateCopy,
    source: CellId,
    destination: SlotId,
    source_view: Option<crate::traits::term::TyTerm>,
    source_reservation: usize,
}
fn place_error(error: PlaceFailure) -> Diagnostic {
    match error {
        PlaceFailure::RuntimeFault(error) => error,
        PlaceFailure::GuestPanic(_) => invalid("prevalidated aggregate Copy place changed bounds"),
    }
}
impl Frame {
    pub(crate) fn resume_operation_index(&self) -> usize {
        self.constant_activation
            .as_ref()
            .map(|_| self.source_constant_operation_index().unwrap_or(0))
            .or_else(|| {
                self.copy_activation
                    .as_ref()
                    .map(|activation| activation.operation_index as usize)
            })
            .or_else(|| {
                self.cleanup_activation
                    .as_ref()
                    .and_then(|activation| activation.operation_index())
            })
            .unwrap_or(0)
    }
    pub(crate) fn has_copy_activation(&self) -> bool {
        self.copy_activation.is_some()
    }
    pub(crate) fn check_copy_activation(&self, index: usize, op: &Op) -> Result<(), Diagnostic> {
        let activation = self
            .copy_activation
            .as_ref()
            .ok_or_else(|| invalid("copy activation absent"))?;
        if self.cleanup_activation.is_some()
            || activation.pc != self.pc.fingerprint()
            || activation.operation_index as usize != index
            || activation.expected != *op
        {
            return Err(invalid(
                "aggregate Copy resumed at a different bound operation",
            ));
        }
        Ok(())
    }
    fn copy_metadata_limit(&self, source_reservation: usize) -> Result<usize, Diagnostic> {
        let base = Self::metadata_bytes(&self.body, self.parameter_metadata_count())
            .and_then(|n| n.checked_add(self.constructor_mapping_bytes))
            .and_then(|n| n.checked_add(self.projection_bytes))
            .and_then(|bytes| bytes.checked_add(self.source_extra_bytes))
            .and_then(|n| n.checked_add(self.auxiliary_metadata_bytes()?))
            .and_then(|n| n.checked_add(source_reservation))
            .ok_or_else(crate::runtime::arena::resource)?;
        (32_usize * 1024 * 1024)
            .checked_sub(base)
            .ok_or_else(crate::runtime::arena::resource)
    }
    /// Called once after ordinary preflight and the opcode entry charge.
    pub(crate) fn begin_aggregate_copy(
        &mut self,
        index: usize,
        op: &Op,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        if self.copy_activation.is_some() {
            return Err(invalid("copy activation already active"));
        }
        let (source, destination, mode) = match op {
            Op::ReadLocal {
                binding,
                destination,
            } => {
                let cell = self.binding(*binding)?;
                (cell, *destination, store.admit_owning_read(cell)?)
            }
            Op::ReadPlace { place, destination } => {
                let (cell, _, mode) = store
                    .admit_owning_place_read(self.executed_place(*place)?)
                    .map_err(place_error)?;
                (cell, *destination, mode)
            }
            _ => return Ok(false),
        };
        if mode != IntrinsicRead::CopyAggregate {
            return Ok(false);
        }
        let index = u32::try_from(index).map_err(|_| invalid("copy operation index overflow"))?;
        let source_view = match op {
            Op::ReadLocal { binding, .. } => {
                self.clone_source(self.binding_sources[binding.0 as usize].as_ref())?
            }
            Op::ReadPlace { place, .. } => {
                self.clone_source(self.place_slot(*place)?.source.as_ref())?
            }
            _ => return Err(invalid("aggregate Copy holds a non-read operation")),
        };
        self.preflight_source_view(destination, source_view.as_ref())?;
        let source_reservation = Self::source_heap(source_view.as_ref())?
            .checked_add(
                Self::source_excess(source_view.as_ref())?
                    .saturating_sub(Self::source_excess(self.source_view_ref(destination)?)?),
            )
            .ok_or_else(crate::runtime::arena::resource)?;
        let limit = self.copy_metadata_limit(source_reservation)?;
        store.reserve_owning_read_result(self.slot(destination)?.cell.is_none(), false)?;
        self.prepare_write(destination, store)?;
        let copy = AggregateCopy::begin(store, source, limit)?;
        self.copy_activation = Some(OpActivation {
            pc: self.pc.fingerprint(),
            operation_index: index,
            expected: op.clone(),
            copy,
            source,
            destination,
            source_view,
            source_reservation,
        });
        Ok(true)
    }
    /// A dispatch chunk includes completion transitions as well as fields, so
    /// deep iterative unwinding cannot become an unbounded host loop either.
    pub(crate) fn advance_aggregate_copy(
        &mut self,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        let source_reservation = self
            .copy_activation
            .as_ref()
            .ok_or_else(|| invalid("copy activation absent"))?
            .source_reservation;
        let limit = self.copy_metadata_limit(source_reservation)?;
        for _ in 0..256 {
            self.fuel.poll()?;
            let activation = self
                .copy_activation
                .as_ref()
                .ok_or_else(|| invalid("copy activation absent"))?;
            let source = match &activation.expected {
                Op::ReadLocal { binding, .. } => self.binding(*binding)?,
                Op::ReadPlace { place, .. } => {
                    store
                        .admit_owning_place_read(self.executed_place(*place)?)
                        .map_err(place_error)?
                        .0
                }
                _ => return Err(invalid("copy activation holds a non-read operation")),
            };
            if source != activation.source {
                return Err(invalid("copy activation source location changed"));
            }
            let current_source = match &activation.expected {
                Op::ReadLocal { binding, .. } => self.binding_sources[binding.0 as usize].as_ref(),
                Op::ReadPlace { place, .. } => self.place_slot(*place)?.source.as_ref(),
                _ => return Err(invalid("copy activation holds a non-read operation")),
            };
            if current_source != activation.source_view.as_ref() {
                return Err(invalid("copy activation source annotation changed"));
            }
            if activation.copy.retained_bytes().is_none_or(|n| n > limit) {
                return Err(crate::runtime::arena::resource());
            }
            if matches!(activation.copy.next(store)?, CopyWork::Field(_)) {
                self.charge_fuel()?;
            }
            let progress = self
                .copy_activation
                .as_mut()
                .ok_or_else(|| invalid("copy activation absent"))?
                .copy
                .advance(store, limit)?;
            if let CopyProgress::Complete(ticket) = progress {
                let activation = self
                    .copy_activation
                    .take()
                    .ok_or_else(|| invalid("copy activation absent"))?;
                let source = activation.source_view;
                self.write(activation.destination, ticket, store)?;
                self.set_source_view(activation.destination, source)?;
                if let Op::ReadPlace { place, .. } = activation.expected {
                    self.clear_place(place)?;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
}
