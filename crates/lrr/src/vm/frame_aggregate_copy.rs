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
    work: CopyOperation,
}
enum CopyOperation {
    Template {
        copy: AggregateCopy,
        template: crate::runtime::cell::aggregate_copy::VmTemplate,
        destination: SlotId,
        source_view: Option<crate::traits::term::TyTerm>,
        promote: bool,
        ticket: Option<TicketId>,
    },
    Read {
        copy: AggregateCopy,
        source: CellId,
        destination: SlotId,
        source_view: Option<crate::traits::term::TyTerm>,
    },
    Pattern(super::pattern_publication_frame::Publication),
    RecordRest(super::record_update_frame::RestActivation),
    Repeat(super::repeat_frame::RepeatActivation),
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
            .or_else(|| self.operator_operation_index())
            .or_else(|| self.rerun_operation_index())
            .or_else(|| self.format_operation_index())
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
        if let Op::Pattern { operation } = op {
            let selected = match operation {
                crate::ir::op::PatternOperation::Expose { owner } => Some((*owner, true)),
                crate::ir::op::PatternOperation::Commit { owner } => Some((*owner, false)),
                _ => None,
            };
            if let Some((owner, guard_view)) = selected {
                let work = self.begin_pattern_publication(owner, guard_view, store)?;
                self.copy_activation = Some(OpActivation {
                    pc: self.pc.fingerprint(),
                    operation_index: u32::try_from(index)
                        .map_err(|_| invalid("pattern publication operation index overflow"))?,
                    expected: op.clone(),
                    work: CopyOperation::Pattern(work),
                });
                return Ok(true);
            }
        }
        if let Op::ValuePath { destination, .. } = op
            && let Some((template, source_view, promote)) = self.item_template_read()
        {
            self.preflight_source_view(*destination, source_view.as_ref())?;
            store.reserve_owning_read_result(self.slot(*destination)?.cell.is_none(), false)?;
            self.prepare_write(*destination, store)?;
            let copy = AggregateCopy::begin_template(store, template.clone())?;
            self.copy_activation = Some(OpActivation {
                pc: self.pc.fingerprint(),
                operation_index: u32::try_from(index)
                    .map_err(|_| invalid("copy operation index overflow"))?,
                expected: op.clone(),
                work: CopyOperation::Template {
                    copy,
                    template,
                    destination: *destination,
                    source_view,
                    promote,
                    ticket: None,
                },
            });
            return Ok(true);
        }
        if let Op::ArrayRepeat {
            source,
            count,
            destination,
        } = op
        {
            if let Some(work) = self.begin_repeat_copy(*source, *count, *destination, store)? {
                self.copy_activation = Some(OpActivation {
                    pc: self.pc.fingerprint(),
                    operation_index: u32::try_from(index)
                        .map_err(|_| invalid("copy operation index overflow"))?,
                    expected: op.clone(),
                    work: CopyOperation::Repeat(work),
                });
                return Ok(true);
            }
            return Ok(false);
        }
        if let Op::ConstructRest { guard, base } = op {
            let work = self.begin_record_rest(*guard, *base, store)?;
            self.copy_activation = Some(OpActivation {
                pc: self.pc.fingerprint(),
                operation_index: u32::try_from(index)
                    .map_err(|_| invalid("copy operation index overflow"))?,
                expected: op.clone(),
                work: CopyOperation::RecordRest(work),
            });
            return Ok(true);
        }
        let (source, destination, mode) = match op {
            Op::ReadLocal {
                binding,
                destination,
            } => {
                let cell = self.binding(*binding)?;
                (cell, *destination, store.admit_owning_read(cell)?)
            }
            // A Copy aggregate parameter (`fn f(a: [i32; 3]) { g(a) }`).
            Op::ReadParameter {
                ordinal,
                destination,
            } => {
                let cell = self.parameter(*ordinal)?;
                (cell, *destination, store.admit_owning_read(cell)?)
            }
            Op::ReadPlace { place, destination } => {
                // A reborrowed `&mut` place is borrowed, not read.
                if self
                    .place_reborrow(*place, *destination, store)
                    .map_err(place_error)?
                    .is_some()
                {
                    return Ok(false);
                }
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
            Op::ReadParameter { ordinal, .. } => {
                self.clone_source(self.parameter_sources.get(*ordinal as usize))?
            }
            Op::ReadPlace { place, .. } => {
                self.clone_source(self.place_slot(*place)?.source.as_ref())?
            }
            _ => return Err(invalid("aggregate Copy holds a non-read operation")),
        };
        self.preflight_source_view(destination, source_view.as_ref())?;
        store.reserve_owning_read_result(self.slot(destination)?.cell.is_none(), false)?;
        self.prepare_write(destination, store)?;
        let copy = AggregateCopy::begin(store, source)?;
        self.copy_activation = Some(OpActivation {
            pc: self.pc.fingerprint(),
            operation_index: index,
            expected: op.clone(),
            work: CopyOperation::Read {
                copy,
                source,
                destination,
                source_view,
            },
        });
        Ok(true)
    }
    /// A dispatch chunk includes completion transitions as well as fields, so
    /// deep iterative unwinding cannot become an unbounded host loop either.
    pub(crate) fn advance_aggregate_copy(
        &mut self,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        for _ in 0..256 {
            let mut activation = self
                .copy_activation
                .take()
                .ok_or_else(|| invalid("copy activation absent"))?;
            let progress = match &mut activation.work {
                CopyOperation::Template {
                    copy,
                    template,
                    destination,
                    source_view,
                    promote,
                    ticket,
                } => (|| {
                    if self.item_template_read()
                        != Some((template.clone(), source_view.clone(), *promote))
                    {
                        return Err(invalid("evaluated template operation source changed"));
                    }
                    if ticket.is_none() {
                        match self.advance_copy_work(copy, store)? {
                            CopyProgress::Pending => return Ok(false),
                            CopyProgress::Complete(value) => *ticket = Some(value),
                        }
                    }
                    let result =
                        ticket.ok_or_else(|| invalid("evaluated template result absent"))?;
                    self.finish_template_read(
                        *destination,
                        result,
                        source_view.clone(),
                        *promote,
                        store,
                    )?;
                    *ticket = None;
                    Ok(true)
                })(),
                CopyOperation::Pattern(publication) => {
                    self.advance_pattern_publication(publication, store)
                }
                CopyOperation::RecordRest(rest) => self.advance_record_rest(rest, store),
                CopyOperation::Repeat(repeat) => self.advance_repeat_copy(repeat, store),
                CopyOperation::Read {
                    copy,
                    source,
                    destination,
                    source_view,
                } => (|| {
                    let current = match &activation.expected {
                        Op::ReadLocal { binding, .. } => self.binding(*binding)?,
                        Op::ReadParameter { ordinal, .. } => self.parameter(*ordinal)?,
                        Op::ReadPlace { place, .. } => {
                            store
                                .admit_owning_place_read(self.executed_place(*place)?)
                                .map_err(place_error)?
                                .0
                        }
                        _ => return Err(invalid("copy activation holds a non-read operation")),
                    };
                    if current != *source {
                        return Err(invalid("copy activation source location changed"));
                    }
                    let current_source = match &activation.expected {
                        Op::ReadLocal { binding, .. } => {
                            self.binding_sources[binding.0 as usize].as_ref()
                        }
                        Op::ReadParameter { ordinal, .. } => {
                            self.parameter_sources.get(*ordinal as usize)
                        }
                        Op::ReadPlace { place, .. } => self.place_slot(*place)?.source.as_ref(),
                        _ => return Err(invalid("copy activation holds a non-read operation")),
                    };
                    if current_source != source_view.as_ref() {
                        return Err(invalid("copy activation source annotation changed"));
                    }
                    if let CopyProgress::Complete(ticket) = self.advance_copy_work(copy, store)? {
                        self.write(*destination, ticket, store)?;
                        self.set_source_view(*destination, source_view.clone())?;
                        if let Op::ReadPlace { place, .. } = &activation.expected {
                            self.clear_place(*place)?;
                        }
                        return Ok(true);
                    }
                    Ok(false)
                })(),
            };
            match progress {
                Ok(true) => return Ok(true),
                Ok(false) => self.copy_activation = Some(activation),
                Err(error) => {
                    // Preserve the ordinary partial-copy/guard owner on refusal.
                    self.copy_activation = Some(activation);
                    return Err(error);
                }
            }
        }
        Ok(false)
    }
    /// Shared structural Copy work accounting/engine for ordinary reads and
    /// omitted record fields; neither operation implements its own copier.
    pub(super) fn advance_copy_work(
        &mut self,
        copy: &mut AggregateCopy,
        store: &mut GuestStore,
    ) -> Result<CopyProgress, Diagnostic> {
        if matches!(copy.next(store)?, CopyWork::Field(_)) {
            self.count_step();
        }
        copy.advance(store)
    }
}
