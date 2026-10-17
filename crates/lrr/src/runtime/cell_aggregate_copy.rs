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
    object: ObjectId,
    ty: TypeId,
    count: usize,
    /// A nominal Copy value's active variant and its fields' exact Types.
    variant: Option<u32>,
    members: Vec<TypeId>,
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
            TypeKey::Tuple(fields) | TypeKey::Callable { captures: fields, .. } => fields.len(),
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
        let mut members = Vec::new();
        if variant.is_some() {
            let (_, storage) = self.place_object(source)?;
            members.try_reserve_exact(count).map_err(|_| resource())?;
            for field in &storage.fields {
                members.push(TypeId(self.admit_move(*field)?.type_id));
            }
        }
        self.objects.reserve(1, &mut self.budget)?;
        let mut fields = Vec::new();
        fields.try_reserve_exact(count).map_err(|_| resource())?;
        let initialized = InitializedFields::empty(count)?;
        let owned = fields
            .capacity()
            .checked_mul(std::mem::size_of::<CellId>())
            .and_then(|n| n.checked_add(initialized.owned_bytes()?))
            .ok_or_else(resource)?;
        let object = ObjectId::from(self.objects.allocate(
            ObjectRecord {
                type_id: ty,
                owner: None,
                storage: ObjectStorage::Aggregate(AggregateStorage {
                    fields,
                    initialized,
                    next_cleanup: 0,
                    pending_cleanup: None,
                    variant,
                }),
            },
            owned,
            &mut self.budget,
        )?);
        Ok(CopyDestination {
            object,
            ty,
            count,
            variant,
            members,
        })
    }
    fn copy_member_type(
        &self,
        destination: &CopyDestination,
        index: usize,
    ) -> Result<TypeId, Diagnostic> {
        if index >= destination.count {
            return Err(ownership());
        }
        if destination.variant.is_some() {
            return destination.members.get(index).copied().ok_or_else(ownership);
        }
        match self.types.descriptor(destination.ty)?.key.as_ref() {
            TypeKey::Tuple(fields) | TypeKey::Callable { captures: fields, .. } => {
                fields.get(index).copied().ok_or_else(ownership)
            }
            TypeKey::Array { element, len }
                if usize::try_from(*len).ok() == Some(destination.count) =>
            {
                Ok(*element)
            }
            _ => Err(ownership()),
        }
    }
    fn append_copy_field(
        &mut self,
        destination: &CopyDestination,
        ticket: TicketId,
    ) -> Result<(), Diagnostic> {
        let object = self.objects.get(destination.object)?;
        let ObjectStorage::Aggregate(storage) = &object.storage else {
            return Err(ownership());
        };
        if object.owner.is_some()
            || object.type_id != destination.ty
            || storage.next_cleanup != 0
            || storage.variant != destination.variant
            || storage.fields.len() >= destination.count
            || storage.fields.capacity() < destination.count
        {
            return Err(ownership());
        }
        let index = storage.fields.len();
        if storage.initialized.field(index)? {
            return Err(ownership());
        }
        let expected = self.copy_member_type(destination, index)?;
        let value = self.peek_ticket(ticket)?;
        self.require_initialized_value(value)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if TypeId(value.type_id) != expected {
            return Err(ownership());
        }
        self.cells.reserve(1, &mut self.budget)?;
        let cell = self.new_cell(
            StorageContract::Exact(expected),
            true,
            CellOwner::Field {
                object: destination.object,
                index: u32::try_from(index).map_err(|_| resource())?,
            },
        )?;
        // Owner=None is a private guard, so ordinary parent initialization is
        // intentionally unavailable. Publish exactly this prevalidated child.
        self.tickets.take(ticket, &mut self.budget)?;
        self.set_owner(value, ValueOwner::Cell(cell))?;
        self.cells.get_mut(cell)?.state = CellState::Live(value);
        let ObjectStorage::Aggregate(storage) =
            &mut self.objects.get_mut(destination.object)?.storage
        else {
            return Err(ownership());
        };
        storage.fields.push(cell);
        storage.initialized.set(index, true)?;
        Ok(())
    }
    fn finish_copy_destination(
        &mut self,
        destination: CopyDestination,
    ) -> Result<TicketId, Diagnostic> {
        let object = self.objects.get(destination.object)?;
        let ObjectStorage::Aggregate(storage) = &object.storage else {
            return Err(ownership());
        };
        if object.owner.is_some()
            || object.type_id != destination.ty
            || storage.next_cleanup != 0
            || storage.variant != destination.variant
            || storage.fields.len() != destination.count
            || !storage.initialized.complete_value()
        {
            return Err(ownership());
        }
        let value = ValueRepr {
            kind: 8,
            type_id: destination.ty.0,
            payload: destination.object.bits(),
        };
        let ticket = TicketId::from(self.tickets.allocate(
            TransferTicket { value },
            0,
            &mut self.budget,
        )?);
        self.set_owner(value, ValueOwner::Ticket(ticket))?;
        Ok(ticket)
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
        if TypeId(value.type_id) != store.copy_member_type(&frame.destination, frame.next)? {
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
                    store.cells.reserve(1, &mut store.budget)?;
                    let ticket = store.copy_from(source)?;
                    let frame = self.stack.last_mut().ok_or_else(ownership)?;
                    store.append_copy_field(&frame.destination, ticket)?;
                    frame.next = frame.next.checked_add(1).ok_or_else(resource)?;
                }
                Ok(CopyProgress::Pending)
            }
            CopyWork::Finish => {
                let nested = self.stack.len() > 1;
                store.tickets.reserve(1, &mut store.budget)?;
                if nested {
                    store.cells.reserve(1, &mut store.budget)?;
                }
                let frame = self.stack.pop().ok_or_else(ownership)?;
                let ticket = store.finish_copy_destination(frame.destination)?;
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
