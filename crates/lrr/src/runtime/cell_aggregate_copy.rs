//! Iterative structural replication. Private destinations never alias an owning source.
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
    Transfer,
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
    /// A replica is owned here until its parent accepts the transfer.
    transfer: Option<TicketId>,
    template: Option<VmTemplate>,
}
/// Physical preparation of a source-checked VM result. This confers no source
/// identity or generic application authority; the interpreter owns those.
pub(crate) struct PreparedVmTemplate {
    session: StoreSession,
    epoch: u32,
    ticket: TicketId,
    value: ValueRepr,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct VmTemplate {
    session: StoreSession,
    epoch: u32,
    root: CellStamp,
    value: ValueRepr,
}
impl VmTemplate {
    pub(crate) fn cell(&self) -> CellId {
        self.root.cell
    }
}
impl GuestStore {
    pub(crate) fn prepare_vm_template(
        &self,
        ticket: TicketId,
    ) -> Result<Option<PreparedVmTemplate>, Diagnostic> {
        let value = self.peek_ticket(ticket)?;
        self.object_owner(value, ValueOwner::Ticket(ticket))?;
        if value.kind != 8 {
            return Ok(None);
        }
        // This is private constant data, not a promoted guest borrow. Each
        // use gets a fresh owning graph and ordinary cleanup/Drop; neither
        // Copy nor absence of Drop is an admission fact for replication.
        // The same physical work engine must replicate every supported leaf
        // without invoking Clone or guest code.
        let mut pending = vec![value];
        while let Some(value) = pending.pop() {
            match self.types.descriptor(TypeId(value.type_id))?.key.as_ref() {
                TypeKey::Nominal { identity, .. } => {
                    // Owning heap containers need their allocation/alias
                    // reconstruction contract, not a shallow field replica.
                    if self.owned_box == Some(*identity) || self.owned_vec == Some(*identity) {
                        return Ok(None);
                    }
                }
                TypeKey::Tuple(_) | TypeKey::Array { .. } | TypeKey::Callable { .. } => {}
                _ => return Ok(None),
            }
            // Use the same layout admission as actual replication, including
            // active nominal variant and structural field-count checks.
            self.copy_value_layout(value, true)?;
            let object = ObjectId::from_bits(value.payload);
            let ObjectStorage::Aggregate(storage) = &self.objects.get(object)?.storage else {
                return Err(ownership());
            };
            for (index, field) in storage.fields.iter().enumerate() {
                if self.cells.get(*field)?.owner
                    != (CellOwner::Field {
                        object,
                        index: u32::try_from(index).map_err(|_| resource())?,
                    })
                {
                    return Err(ownership());
                }
                let member = self.admit_move(*field)?;
                match member.kind {
                    8 => pending.push(member),
                    1..=7 | 11 | 12 => {
                        self.admit_copy(*field)?;
                    }
                    9 => {
                        if self
                            .references
                            .get(RefId::from_bits(member.payload))?
                            .access
                            == Access::Mutable
                        {
                            return Ok(None);
                        }
                        self.admit_copy(*field)?;
                    }
                    _ => return Ok(None),
                }
            }
        }
        Ok(Some(PreparedVmTemplate {
            session: self.pattern_binding_session(),
            epoch: self.budget.epoch(),
            ticket,
            value,
        }))
    }
    /// Move-to-cell is the only operation between preparation and sealing.
    /// The source graph never escapes into a guest-visible mutable place.
    pub(crate) fn seal_vm_template(
        &self,
        prepared: PreparedVmTemplate,
        cell: CellId,
    ) -> Result<VmTemplate, Diagnostic> {
        if prepared.session != self.pattern_binding_session()
            || prepared.epoch != self.budget.epoch()
            || !matches!(self.tickets.get(prepared.ticket), Err(error) if error.code == "LRR-E-STALE-HANDLE")
            || self.cells.get(cell)?.writable
            || self.admit_move(cell)? != prepared.value
        {
            return Err(ownership());
        }
        let template = VmTemplate {
            session: prepared.session,
            epoch: prepared.epoch,
            root: self.place_stamp(cell)?,
            value: prepared.value,
        };
        self.validate_vm_template(&template)?;
        Ok(template)
    }
    pub(crate) fn validate_vm_template(&self, template: &VmTemplate) -> Result<(), Diagnostic> {
        if template.session != self.pattern_binding_session()
            || template.epoch != self.budget.epoch()
        {
            return Err(lease_fault());
        }
        self.copy_source(template.root, template.value)?;
        if self.cells.get(template.root.cell)?.writable {
            return Err(ownership());
        }
        Ok(())
    }
    fn copy_source(&self, stamp: CellStamp, value: ValueRepr) -> Result<(), Diagnostic> {
        if self.place_stamp(stamp.cell)? != stamp || self.admit_move(stamp.cell)? != value {
            return Err(lease_fault());
        }
        Ok(())
    }
    fn copy_layout(
        &self,
        source: CellId,
        template: bool,
    ) -> Result<(TypeId, usize, Option<u32>), Diagnostic> {
        self.place_stamp(source)?;
        let value = self.admit_move(source)?;
        let descriptor = self.types.descriptor(TypeId(value.type_id))?;
        if !template
            && !matches!(descriptor.key.as_ref(), TypeKey::Nominal { .. })
            && descriptor.copy != CopyStatus::Copy
            && !(descriptor.copy == CopyStatus::Deferred && self.fields_copy(source)?)
        {
            return Err(requires_activation());
        }
        self.copy_value_layout(value, template)
    }
    fn copy_value_layout(
        &self,
        value: ValueRepr,
        template: bool,
    ) -> Result<(TypeId, usize, Option<u32>), Diagnostic> {
        let ty = TypeId(value.type_id);
        let descriptor = self.types.descriptor(ty)?;
        if value.kind != 8 {
            return Err(requires_activation());
        }
        let ObjectStorage::Aggregate(storage) = &self
            .objects
            .get(ObjectId::from_bits(value.payload))?
            .storage
        else {
            return Err(ownership());
        };
        if storage.next_cleanup != 0 {
            return Err(lease_fault());
        }
        // Ordinary Copy still requires the declaration's real Copy fact.
        // A private sealed constant-data template permits physical replication
        // without changing that trait fact.
        if let TypeKey::Nominal { identity, .. } = descriptor.key.as_ref() {
            if !template && !self.nominal_copy(*identity) {
                return Err(requires_activation());
            }
            if storage.variant.is_none() || !storage.initialized.complete_value() {
                return Err(ownership());
            }
            return Ok((ty, storage.fields.len(), storage.variant));
        }
        let count = match descriptor.key.as_ref() {
            TypeKey::Tuple(fields)
            | TypeKey::Callable {
                captures: fields, ..
            } => fields.len(),
            TypeKey::Array { len, .. } => usize::try_from(*len).map_err(|_| resource())?,
            _ => return Err(requires_activation()),
        };
        if storage.fields.len() != count
            || storage.variant.is_some()
            || !storage.initialized.complete_value()
        {
            return Err(ownership());
        }
        Ok((ty, count, None))
    }
    fn begin_copy_destination(
        &mut self,
        source: CellId,
        template: bool,
    ) -> Result<CopyDestination, Diagnostic> {
        let (ty, count, variant) = self.copy_layout(source, template)?;
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
        Self::begin_with_source(store, source, None)
    }
    pub(crate) fn begin_template(
        store: &mut GuestStore,
        template: VmTemplate,
    ) -> Result<Self, Diagnostic> {
        store.validate_vm_template(&template)?;
        Self::begin_with_source(store, template.root.cell, Some(template))
    }
    fn begin_with_source(
        store: &mut GuestStore,
        source: CellId,
        template: Option<VmTemplate>,
    ) -> Result<Self, Diagnostic> {
        let root = store.place_stamp(source)?;
        let root_value = store.admit_move(source)?;
        store.copy_layout(source, template.is_some())?;
        let mut copy = Self {
            root,
            root_value,
            stack: Vec::new(),
            complete: false,
            transfer: None,
            template,
        };
        copy.reserve_frame()?;
        let destination = store.begin_copy_destination(source, copy.template.is_some())?;
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
        if let Some(template) = self.template.as_ref() {
            store.validate_vm_template(template)?;
        }
        store.copy_source(self.root, self.root_value)?;
        let frame = self.stack.last().ok_or_else(ownership)?;
        store.copy_source(frame.source, frame.value)?;
        if let Some(ticket) = self.transfer {
            store.peek_ticket(ticket)?;
            return Ok(CopyWork::Transfer);
        }
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
            store.copy_layout(field, self.template.is_some())?;
        } else {
            store.admit_copy(field)?;
        }
        Ok(CopyWork::Field(field))
    }
    fn finish_transfer(&mut self, store: &mut GuestStore) -> Result<(), Diagnostic> {
        let ticket = self.transfer.ok_or_else(ownership)?;
        let parent = self.stack.last_mut().ok_or_else(ownership)?;
        let next = parent.next.checked_add(1).ok_or_else(resource)?;
        // A refused transactional append leaves the actual replica reachable.
        // Clear it only after the common construction transfer consumes it.
        store.append_copy_field(&parent.destination, ticket)?;
        self.transfer = None;
        parent.next = next;
        Ok(())
    }
    pub(crate) fn advance(&mut self, store: &mut GuestStore) -> Result<CopyProgress, Diagnostic> {
        match self.next(store)? {
            CopyWork::Transfer => {
                self.finish_transfer(store)?;
                Ok(CopyProgress::Pending)
            }
            CopyWork::Field(source) => {
                let value = store.admit_move(source)?;
                if value.kind == 8 {
                    self.reserve_frame()?;
                    let stamp = store.place_stamp(source)?;
                    let destination =
                        store.begin_copy_destination(source, self.template.is_some())?;
                    self.stack.push(CopyFrame {
                        source: stamp,
                        value,
                        destination,
                        next: 0,
                    });
                } else {
                    store.tickets.reserve(1, &mut store.budget)?;
                    let ticket = store.copy_from(source)?;
                    self.transfer = Some(ticket);
                    self.finish_transfer(store)?;
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
                if !self.stack.is_empty() {
                    self.transfer = Some(ticket);
                    self.finish_transfer(store)?;
                    Ok(CopyProgress::Pending)
                } else {
                    self.complete = true;
                    Ok(CopyProgress::Complete(ticket))
                }
            }
        }
    }
}

#[cfg(test)]
mod template_authority_tests {
    use super::*;

    fn sealed(store: &mut GuestStore) -> VmTemplate {
        let ty = store
            .intern_type(TypeKey::Tuple(vec![TypeId(6)].into()))
            .unwrap();
        let guard = store.begin_construction(&[TypeId(6)]).unwrap();
        let member = store
            .ticket(ValueRepr::signed(TypeId(6), 7).unwrap())
            .unwrap();
        store.construction_push(guard, 0, member).unwrap();
        let value = store.commit_construction(guard, ty).unwrap();
        let prepared = store.prepare_vm_template(value).unwrap().unwrap();
        let frame = store.new_frame_anchor().unwrap();
        let cell = store
            .new_cell(
                StorageContract::Exact(ty),
                false,
                CellOwner::FrameSlot { frame, slot: 0 },
            )
            .unwrap();
        store.initialize(cell, value).unwrap();
        store.seal_vm_template(prepared, cell).unwrap()
    }

    #[test]
    fn evaluated_template_seals_reject_foreign_store_and_changed_owner_before_replication() {
        let mut a = GuestStore::for_epoch(1).unwrap();
        let mut b = GuestStore::for_epoch(1).unwrap();
        let first = sealed(&mut a);
        let second = sealed(&mut b);
        assert_eq!(first.cell().bits(), second.cell().bits());
        let before = b.state(second.cell()).unwrap();
        assert_eq!(
            AggregateCopy::begin_template(&mut b, first.clone())
                .err()
                .unwrap()
                .code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(b.state(second.cell()).unwrap(), before);
        b.validate_vm_template(&second).unwrap();
        let moved = a.move_from(first.cell()).unwrap();
        assert_eq!(
            AggregateCopy::begin_template(&mut a, first.clone())
                .err()
                .unwrap()
                .code,
            "LRR-E-STALE-HANDLE"
        );
        assert_eq!(a.peek_ticket(moved).unwrap(), first.value);
    }
    #[test]
    fn refused_copy_parent_retains_committed_child_ticket_for_retry() {
        let mut store = GuestStore::for_epoch(1).unwrap();
        let inner = sealed(&mut store);
        let source = {
            let ty = store.place_type(inner.cell()).unwrap();
            let ticket = store.move_from(inner.cell()).unwrap();
            let outer_ty = store.intern_type(TypeKey::Tuple(vec![ty].into())).unwrap();
            let guard = store.begin_construction(&[ty]).unwrap();
            store.construction_push(guard, 0, ticket).unwrap();
            let ticket = store.commit_construction(guard, outer_ty).unwrap();
            let frame = store.new_frame_anchor().unwrap();
            let cell = store
                .new_cell(
                    StorageContract::Exact(outer_ty),
                    false,
                    CellOwner::FrameSlot { frame, slot: 0 },
                )
                .unwrap();
            store.initialize(cell, ticket).unwrap();
            cell
        };
        let original = store.admit_move(source).unwrap();
        let mut copy = AggregateCopy::begin(&mut store, source).unwrap();
        assert!(matches!(
            copy.advance(&mut store).unwrap(),
            CopyProgress::Pending
        ));
        assert!(matches!(
            copy.advance(&mut store).unwrap(),
            CopyProgress::Pending
        ));
        let parent = copy.stack[0].destination.guard;
        // An authority refusal before the parent consumes the replica.
        store.constructions.get_mut(parent).unwrap().state = ConstructionState::Cleaning;
        assert!(copy.advance(&mut store).is_err());
        let retained = copy
            .transfer
            .expect("replica must remain owned by the continuation");
        let replica = store.peek_ticket(retained).unwrap();
        assert_eq!(replica.kind, 8);
        store.constructions.get_mut(parent).unwrap().state = ConstructionState::Building;
        assert!(matches!(copy.next(&store).unwrap(), CopyWork::Transfer));
        assert!(matches!(
            copy.advance(&mut store).unwrap(),
            CopyProgress::Pending
        ));
        assert!(copy.transfer.is_none());
        assert_eq!(
            store.peek_ticket(retained).unwrap_err().code,
            "LRR-E-STALE-HANDLE"
        );
        let CopyProgress::Complete(result) = copy.advance(&mut store).unwrap() else {
            panic!("completed parent must return its actual result");
        };
        let copied = store.peek_ticket(result).unwrap();
        assert_eq!(copied.type_id, original.type_id);
        assert_ne!(copied.payload, original.payload);
        assert_eq!(store.admit_move(source).unwrap(), original);
    }
}
