//! Omitted record fields use ordinary place ownership and the same resumable
//! structural Copy engine as ReadLocal/ReadPlace. No source planning here.
use super::*;
use crate::runtime::{
    cell::{
        IntrinsicRead,
        aggregate_copy::{AggregateCopy, CopyProgress},
    },
    place::PlaceFailure,
};

pub(super) struct RestActivation {
    guard_slot: SlotId,
    guard: ConstructionId,
    base: SlotId,
    next: usize,
    copy: Option<(CellId, AggregateCopy)>,
}
fn place_error(failure: PlaceFailure) -> Diagnostic {
    match failure {
        PlaceFailure::RuntimeFault(error) => error,
        PlaceFailure::GuestPanic(_) => {
            invalid("record update base changed bounds after evaluation")
        }
    }
}
impl Frame {
    pub(crate) fn preflight_record_rest(
        &self,
        guard: SlotId,
        base: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let active = self
            .constructor_slot(guard)?
            .construction
            .as_ref()
            .filter(|active| !active.cleaning && active.base_place == Some(base))
            .ok_or_else(|| invalid("record update guard/source base differs"))?;
        let bound = active
            .mapping
            .as_ref()
            .ok_or_else(|| invalid("record update requires its nominal source mapping"))?
            .bound()?;
        let place = self.executed_place(base)?;
        let ty = store.place_type_id(place).map_err(place_error)?;
        if !matches!(store.type_descriptor(ty)?.key.as_ref(),
            crate::runtime::shape::TypeKey::Nominal {identity, schema, ..}
                if *identity == bound.variant().identity() && *schema == bound.variant().schema())
            || store.place_variant(place).map_err(place_error)? != Some(bound.variant().variant())
        {
            return Err(invalid(
                "record update base/source nominal identity differs",
            ));
        }
        Ok(())
    }
    pub(super) fn begin_record_rest(
        &self,
        guard: SlotId,
        base: SlotId,
        store: &GuestStore,
    ) -> Result<RestActivation, Diagnostic> {
        self.preflight_record_rest(guard, base, store)?;
        let active = self
            .constructor_slot(guard)?
            .construction
            .as_ref()
            .ok_or_else(|| invalid("record update guard absent"))?;
        Ok(RestActivation {
            guard_slot: guard,
            guard: active.guard,
            base,
            next: 0,
            copy: None,
        })
    }
    pub(super) fn advance_record_rest(
        &mut self,
        rest: &mut RestActivation,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        self.preflight_record_rest(rest.guard_slot, rest.base, store)?;
        let active = self
            .constructor_slot(rest.guard_slot)?
            .construction
            .as_ref()
            .filter(|active| active.guard == rest.guard)
            .ok_or_else(|| invalid("record update construction identity changed"))?;
        let bound = active
            .mapping
            .as_ref()
            .ok_or_else(|| invalid("record update source mapping absent"))?
            .bound()?;
        let Some(field) = bound.mapping().base_fields().get(rest.next).copied() else {
            if rest.copy.is_some() {
                return Err(invalid("record update copy cursor differs"));
            }
            self.clear_place(rest.base)?;
            return Ok(true);
        };
        let place = store
            .project_nominal_field(self.executed_place(rest.base)?, bound.variant(), field)
            .map_err(place_error)?;
        let (source, _, mode) = store.admit_owning_place_read(&place).map_err(place_error)?;
        store.admit_construction_move(rest.guard, field, source)?;
        if let Some((expected, copy)) = &mut rest.copy {
            if source != *expected || mode != IntrinsicRead::CopyAggregate {
                return Err(invalid("record update Copy source changed"));
            }
            if let CopyProgress::Complete(ticket) = self.advance_copy_work(copy, store)? {
                store.construction_push(rest.guard, field, ticket)?;
                rest.copy = None;
                rest.next = rest
                    .next
                    .checked_add(1)
                    .ok_or_else(|| invalid("record update cursor overflow"))?;
            }
        } else if mode == IntrinsicRead::CopyAggregate {
            rest.copy = Some((source, AggregateCopy::begin(store, source)?));
        } else {
            let ticket = store.read_owning_place(&place).map_err(place_error)?;
            store.construction_push(rest.guard, field, ticket)?;
            rest.next = rest
                .next
                .checked_add(1)
                .ok_or_else(|| invalid("record update cursor overflow"))?;
        }
        Ok(false)
    }
}
