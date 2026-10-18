//! Iterative structural Copy. Private destinations never alias an owning source.
//! Each advance installs one leaf, enters one aggregate, or completes one guard.
use super::super::shape::CopyStatus;
use super::*;

pub(crate) fn requires_activation() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-UNSUPPORTED",
        "aggregate Copy requires its resumable frame activation".into(),
    )
}
struct CopyDestination {
    guard: ConstructionId,
    ty: TypeId,
    count: usize,
    /// A nominal Copy value's active variant and its fields' exact Types.
    variant: Option<u32>,
}
struct CopyFrame {
    source: CellStamp,
    value: ValueRepr,
    destination: CopyDestination,
    next: usize,
}
pub(crate) enum CopyWork {
    Field(CellId),
    Finish,
}
pub(crate) enum CopyProgress {
    Pending,
    Complete(TicketId),
}
pub(crate) struct AggregateCopy {
    root: CellStamp,
    root_value: ValueRepr,
    stack: Vec<CopyFrame>,
    complete: bool,
}
impl GuestStore {
    fn copy_source(&self, stamp: CellStamp, value: ValueRepr) -> Result<(), Diagnostic> {
        if self.place_stamp(stamp.cell)? != stamp || self.admit_move(stamp.cell)? != value {
            return Err(lease_fault());
        }
        Ok(())
    }
    fn copy_layout(&self, source: CellId) -> Result<(TypeId, usize, Option<u32>), Diagnostic> {
        let value = self.admit_move(source)?;
        let ty = TypeId(value.type_id);
        let descriptor = self.types.descriptor(ty)?;
        if value.kind != 8 {
            return Err(requires_activation());
        }
        // A nominal value is Copy exactly when its declaration has a Copy
        // impl in the run's complete impl inventory.
        if let TypeKey::Nominal { identity, .. } = descriptor.key.as_ref() {
            if !self.nominal_copy(*identity) {
                return Err(requires_activation());
            }
            let (_, storage) = self.place_object(source)?;
            if storage.variant.is_none() || !storage.initialized.complete_value() {
                return Err(ownership());
            }
            return Ok((ty, storage.fields.len(), storage.variant));
        }
        if descriptor.copy != CopyStatus::Copy
            && !(descriptor.copy == CopyStatus::Deferred && self.fields_copy(source)?)
        {
            return Err(requires_activation());
        }
        let count = match descriptor.key.as_ref() {
            TypeKey::Tuple(fields)
            | TypeKey::Callable {
                captures: fields, ..
            } => fields.len(),
            TypeKey::Array { len, .. } => usize::try_from(*len).map_err(|_| resource())?,
            _ => return Err(requires_activation()),
        };
        let (_, storage) = self.place_object(source)?;
        if storage.fields.len() != count
            || storage.variant.is_some()
            || !storage.initialized.complete_value()
        {
            return Err(ownership());
        }
        Ok((ty, count, None))
    }
    fn begin_copy_destination(&mut self, source: CellId) -> Result<CopyDestination, Diagnostic> {
        let (ty, count, variant) = self.copy_layout(source)?;
        // The source is already an owning, fully initialized value admitted by
        // the same Copy rule above. Its real field contracts, rather than an
        // invented source constructor, determine this physical replica guard.
        let mut members = Vec::new();
        members.try_reserve_exact(count).map_err(|_| resource())?;
        if variant.is_some() {
            let (_, storage) = self.place_object(source)?;
            for field in &storage.fields {
                members.push(TypeId(self.admit_move(*field)?.type_id));
            }
        } else {
            match self.types.descriptor(ty)?.key.as_ref() {
                TypeKey::Tuple(fields)
                | TypeKey::Callable {
                    captures: fields, ..
                } => {
                    members.extend(fields.iter().copied());
                }
                TypeKey::Array { element, .. } => members.resize(count, *element),
                _ => return Err(ownership()),
            }
        }
        let guard = self.begin_construction(&members)?;
        if let Some(variant) = variant {
            self.constructions.get_mut(guard)?.nominal = Some((ty, variant));
        }
        Ok(CopyDestination {
            guard,
            ty,
            count,
            variant,
        })
    }
    fn copy_destination(
        &self,
        destination: &CopyDestination,
    ) -> Result<&ConstructionRecord, Diagnostic> {
        let record = self.constructions.get(destination.guard)?;
        if record.state != ConstructionState::Building
            || record.expected.len() != destination.count
            || record.fields.len() != destination.count
            || record.nominal != destination.variant.map(|variant| (destination.ty, variant))
            || record.observed_kind.is_some()
            || record.pending_nominal.is_some()
        {
            return Err(ownership());
        }
        Ok(record)
    }
    fn copy_member_type(
        &self,
        destination: &CopyDestination,
        index: usize,
    ) -> Result<TypeId, Diagnostic> {
        self.copy_destination(destination)?
            .expected
            .get(index)
            .copied()
            .flatten()
            .ok_or_else(ownership)
    }
    fn append_copy_field(
        &mut self,
        destination: &CopyDestination,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let index = self.copy_destination(destination)?.completed.len();
        if index >= destination.count {
            return Err(ownership());
        }
        let field = u32::try_from(index).map_err(|_| resource())?;
        self.construction_push(destination.guard, field, ticket)
    }
    fn finish_copy_destination(
        &mut self,
        destination: &CopyDestination,
    ) -> Result<TicketId, Diagnostic> {
        self.copy_destination(destination)?;
        // The common commit validates every field/owner and reserves all
        // destination storage before consuming the actual construction guard.
        self.commit_construction(destination.guard, destination.ty)
    }
}
impl AggregateCopy {
    pub(crate) fn begin(store: &mut GuestStore, source: CellId) -> Result<Self, Diagnostic> {
        let root = store.place_stamp(source)?;
        let root_value = store.admit_move(source)?;
        store.copy_layout(source)?;
        let mut copy = Self {
            root,
            root_value,
            stack: Vec::new(),
            complete: false,
        };
        copy.reserve_frame()?;
        let destination = store.begin_copy_destination(source)?;
        copy.stack.push(CopyFrame {
            source: root,
            value: root_value,
            destination,
            next: 0,
        });
        Ok(copy)
    }
    /// Reserve the next nesting frame before any owner moves (no depth cap).
    fn reserve_frame(&mut self) -> Result<(), Diagnostic> {
        self.stack.try_reserve(1).map_err(|_| resource())
    }
    pub(crate) fn next(&self, store: &GuestStore) -> Result<CopyWork, Diagnostic> {
        if self.complete {
            return Err(ownership());
        }
        store.copy_source(self.root, self.root_value)?;
        let frame = self.stack.last().ok_or_else(ownership)?;
        store.copy_source(frame.source, frame.value)?;
        if frame.next == frame.destination.count {
            return Ok(CopyWork::Finish);
        }
        let (object, storage) = store.place_object(frame.source.cell)?;
        let field = *storage.fields.get(frame.next).ok_or_else(ownership)?;
        if store.cells.get(field)?.owner
            != (CellOwner::Field {
                object,
                index: u32::try_from(frame.next).map_err(|_| resource())?,
            })
        {
            return Err(ownership());
        }
        let value = store.admit_move(field)?;
        let member = store.copy_member_type(&frame.destination, frame.next)?;
        if TypeId(value.type_id) != member
            && !store.contract_admits(Some(member), Some(TypeId(value.type_id)))
        {
            return Err(ownership());
        }
        if value.kind == 8 {
            store.copy_layout(field)?;
        } else {
            store.admit_copy(field)?;
        }
        Ok(CopyWork::Field(field))
    }
    pub(crate) fn advance(&mut self, store: &mut GuestStore) -> Result<CopyProgress, Diagnostic> {
        match self.next(store)? {
            CopyWork::Field(source) => {
                let value = store.admit_move(source)?;
                if value.kind == 8 {
                    self.reserve_frame()?;
                    let stamp = store.place_stamp(source)?;
                    let destination = store.begin_copy_destination(source)?;
                    self.stack.push(CopyFrame {
                        source: stamp,
                        value,
                        destination,
                        next: 0,
                    });
                } else {
                    store.tickets.reserve(1, &mut store.budget)?;
                    let ticket = store.copy_from(source)?;
                    let frame = self.stack.last_mut().ok_or_else(ownership)?;
                    store.append_copy_field(&frame.destination, ticket)?;
                    frame.next = frame.next.checked_add(1).ok_or_else(resource)?;
                }
                Ok(CopyProgress::Pending)
            }
            CopyWork::Finish => {
                store.tickets.reserve(1, &mut store.budget)?;
                // A failed common commit leaves the exact live guard reachable
                // in this activation rather than losing it by popping first.
                let frame = self.stack.last().ok_or_else(ownership)?;
                let ticket = store.finish_copy_destination(&frame.destination)?;
                self.stack.pop().ok_or_else(ownership)?;
                if let Some(parent) = self.stack.last_mut() {
                    store.append_copy_field(&parent.destination, ticket)?;
                    parent.next = parent.next.checked_add(1).ok_or_else(resource)?;
                    Ok(CopyProgress::Pending)
                } else {
                    self.complete = true;
                    Ok(CopyProgress::Complete(ticket))
                }
            }
        }
    }
}
