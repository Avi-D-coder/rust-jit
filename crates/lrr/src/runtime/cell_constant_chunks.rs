//! Owned physical cursor only. No source, query, compiler or proof references.
use super::{PendingConstantResult, *};
use std::vec::IntoIter;
const CHUNK_STEPS: usize = 256;
const CHUNK_BYTES: usize = 64 << 10;
// Conservative fixed state/ticket/cell/owner transfer traffic for each step.
const STEP_BYTES: usize = std::mem::size_of::<Node>()
    + std::mem::size_of::<Cell>() * 2
    + std::mem::size_of::<ObjectRecord>() * 2
    + std::mem::size_of::<ConstructionRecord>()
    + std::mem::size_of::<TransferTicket>() * 2
    + 128;
struct Building {
    ty: TypeId,
    guard: ConstructionId,
    storage: ReservedArrayStorage,
    children: IntoIter<Node>,
    next: u32,
    child_started: bool,
}
struct Committing {
    ty: TypeId,
    element: TypeId,
    guard: ConstructionId,
    object: ObjectId,
    fields: Vec<CellId>,
    initialized: InitializedFields,
    next: usize,
    count: usize,
}
enum Cleanup {
    Ticket(TicketId),
    Value(ValueRepr),
    Guard(ConstructionId),
    Cells(IntoIter<CellId>),
    Object(ObjectId),
}
/// Single-use owned activation. No Clone/serialization/raw ticket constructor.
pub(crate) struct ConstantChunkActivation {
    session: StoreSession,
    epoch: u32,
    node: Option<Node>,
    stack: Vec<Building>,
    commit: Option<Committing>,
    pending: Option<TicketId>,
    cleanup: Vec<Cleanup>,
    abandoned: Vec<Building>,
    failure: Option<Diagnostic>,
    finished: bool,
    fields_charged: u64,
}
pub(crate) enum ConstantChunkPoll {
    Dispatch {
        steps: u16,
        bytes: u32,
        cleaning: bool,
    },
    Ready(PendingConstantResult),
    Failed(Diagnostic),
}
impl ConstantChunkActivation {
    pub(crate) fn fields_charged(&self) -> u64 {
        self.fields_charged
    }
    pub(crate) fn finished_for_frame(&self) -> bool {
        self.finished
    }
    pub(crate) fn cleaning(&self) -> bool {
        self.failure.is_some()
    }
}
impl GuestStore {
    /// Complete table reservation includes the destination cell not allocated yet.
    /// No opcode/field is charged here and no guest value/guard is installed.
    pub(crate) fn prepare_constant_chunks(
        &mut self,
        plan: PreparedConstantInstantiation,
        destination_cells: usize,
    ) -> Result<ConstantChunkActivation, Diagnostic> {
        if plan.session != self.pattern_binding_session() || plan.epoch != self.budget.epoch() {
            return Err(stale());
        }
        if destination_cells > 1 || STEP_BYTES > CHUNK_BYTES {
            return Err(resource());
        }
        let stack = Vec::new();
        let cleanup = Vec::new();
        let abandoned = Vec::new();
        self.reserve_constant_instantiation_with_destination(&plan.totals, destination_cells)?;
        Ok(ConstantChunkActivation {
            session: self.pattern_binding_session(),
            epoch: plan.epoch,
            node: Some(plan.node),
            stack,
            commit: None,
            pending: None,
            cleanup,
            abandoned,
            failure: None,
            finished: false,
            fields_charged: 0,
        })
    }
    fn start_chunk_array(
        &mut self,
        ty: TypeId,
        mut records: Vec<ConstructionRecord>,
        storage: ReservedArrayStorage,
        children: Vec<Node>,
    ) -> Result<Building, Diagnostic> {
        let record = records.pop().ok_or_else(ownership)?;
        let bytes = record.owned_bytes().ok_or_else(resource)?;
        let guard = ConstructionId::from(self.constructions.allocate(
            record,
            bytes,
            &mut self.budget,
        )?);
        Ok(Building {
            ty,
            guard,
            storage,
            children: children.into_iter(),
            next: 0,
            child_started: false,
        })
    }
    fn start_chunk_commit(
        &mut self,
        slot: &mut Option<Building>,
    ) -> Result<Committing, Diagnostic> {
        let building = slot.as_ref().ok_or_else(ownership)?;
        let preflight = (|| {
            let record = self.constructions.get(building.guard)?;
            let TypeKey::Array { element, len } = self.types.descriptor(building.ty)?.key.as_ref()
            else {
                return Err(ownership());
            };
            let count = *len as usize;
            if record.state != ConstructionState::Building
                || record.nominal.is_some()
                || record.fields.len() != count
                || record.expected.len() != count
                || record.completed.len() != count
                || building.next as usize != count
                || building.children.len() != 0
                || building.child_started
                || building.storage.count != count
                || !building.storage.fields.is_empty()
                || building.storage.fields.capacity() < count
            {
                return Err(ownership());
            }
            Ok((*element, count))
        })();
        let (element, count) = preflight?;
        // The private plan already reserved exact empty field storage/bitmap.
        // Moving this storage is constant work; no all-field scan is performed.
        let owned = building.storage.owned_bytes()?;
        let object = ObjectId::from(self.objects.allocate(
            ObjectRecord {
                type_id: building.ty,
                storage: ObjectStorage::Constructing,
                owner: None,
            },
            owned,
            &mut self.budget,
        )?);
        let building = slot.take().ok_or_else(ownership)?;
        let ReservedArrayStorage {
            fields,
            initialized,
            ..
        } = building.storage;
        Ok(Committing {
            ty: building.ty,
            element,
            guard: building.guard,
            object,
            fields,
            initialized,
            next: 0,
            count,
        })
    }
    fn chunk_commit_field(&mut self, c: &mut Committing) -> Result<(), Diagnostic> {
        let index = c.next;
        let record = self.constructions.get(c.guard)?;
        if record.expected.get(index) != Some(&Some(c.element)) {
            return Err(ownership());
        }
        let value = record
            .fields
            .get(index)
            .copied()
            .flatten()
            .ok_or_else(ownership)?;
        self.require_initialized_value(value)?;
        self.object_owner(
            value,
            ValueOwner::Construction {
                guard: c.guard,
                field: u32::try_from(index).map_err(|_| resource())?,
            },
        )?;
        if value.type_id != c.element.0 {
            return Err(ownership());
        }
        let cell = self.new_cell(
            StorageContract::Exact(c.element),
            true,
            CellOwner::Field {
                object: c.object,
                index: u32::try_from(index).map_err(|_| resource())?,
            },
        )?;
        c.fields.push(cell);
        self.set_owner(value, ValueOwner::Cell(cell))?;
        self.cells.get_mut(cell)?.state = CellState::Live(value);
        self.constructions.get_mut(c.guard)?.fields[index] = None;
        c.next = c.next.checked_add(1).ok_or_else(resource)?;
        Ok(())
    }
    fn finish_chunk_commit(
        &mut self,
        slot: &mut Option<Committing>,
    ) -> Result<TicketId, Diagnostic> {
        let c = slot.as_ref().ok_or_else(ownership)?;
        if c.next != c.count || c.fields.len() != c.count {
            return Err(ownership());
        }
        self.constructions.get(c.guard)?;
        let object = self.objects.get(c.object)?;
        if !matches!(object.storage, ObjectStorage::Constructing) {
            return Err(ownership());
        }
        let value = ValueRepr {
            kind: 8,
            type_id: c.ty.0,
            payload: c.object.bits(),
        };
        let ticket = TicketId::from(self.tickets.allocate(
            TransferTicket { value },
            0,
            &mut self.budget,
        )?);
        // The exact private cursor has consumed every guard field once. Nothing
        // can observe Constructing storage; the final move installs complete data.
        if let Err(e) = self.constructions.take(c.guard, &mut self.budget) {
            let _ = self.tickets.take(ticket, &mut self.budget);
            return Err(e);
        }
        let object = match self.objects.get_mut(c.object) {
            Ok(v) => v,
            Err(e) => {
                let _ = self.tickets.take(ticket, &mut self.budget);
                return Err(e);
            }
        };
        let c = slot.take().ok_or_else(ownership)?;
        object.storage = ObjectStorage::Aggregate(AggregateStorage {
            memory: None,
            fields: c.fields,
            initialized: c.initialized,
            next_cleanup: 0,
            pending_cleanup: None,
            variant: None,
        });
        object.owner = Some(ValueOwner::Ticket(ticket));
        Ok(ticket)
    }
    fn constant_chunk_failure(
        &mut self,
        a: &mut ConstantChunkActivation,
        error: Diagnostic,
    ) -> Result<(), Diagnostic> {
        if a.failure.is_some() {
            return Ok(());
        }
        a.failure = Some(error);
        // Stage each pending owner during a later counted cleanup step. Avoid
        // draining all active nesting/commit state in an already full chunk.

        Ok(())
    }
    pub(crate) fn abort_constant_chunks(
        &mut self,
        a: &mut ConstantChunkActivation,
        error: Diagnostic,
    ) -> Result<(), Diagnostic> {
        if a.session != self.pattern_binding_session()
            || a.epoch != self.budget.epoch()
            || a.finished
        {
            return Err(stale());
        }
        self.constant_chunk_failure(a, error)
    }
    fn cleanup_constant_chunk_step(
        &mut self,
        a: &mut ConstantChunkActivation,
    ) -> Result<(), Diagnostic> {
        if a.cleanup.is_empty() {
            if let Some(ticket) = a.pending.take() {
                a.cleanup.push(Cleanup::Ticket(ticket));
                return Ok(());
            }
            if let Some(c) = a.commit.take() {
                a.cleanup.push(Cleanup::Object(c.object));
                a.cleanup.push(Cleanup::Cells(c.fields.into_iter()));
                a.cleanup.push(Cleanup::Guard(c.guard));
                return Ok(());
            }
            if let Some(b) = a.stack.pop() {
                a.cleanup.push(Cleanup::Guard(b.guard));
                // Retain unused immutable blueprint/storage until fatal metadata
                // teardown; don't drop a whole child tree inside a counted chunk.
                a.abandoned.push(b);
                return Ok(());
            }
            return Err(ownership());
        }
        let task = a.cleanup.pop().ok_or_else(ownership)?;
        match task {
            Cleanup::Ticket(ticket) => {
                let value = self.tickets.take(ticket, &mut self.budget)?.value;
                a.cleanup.push(Cleanup::Value(value));
            }
            Cleanup::Value(value) => {
                if (1..=7).contains(&value.kind) {
                    value.validate_scalar()?;
                } else if value.kind == 8
                    && matches!(
                        self.types.descriptor(TypeId(value.type_id))?.key.as_ref(),
                        TypeKey::Array { .. }
                    )
                {
                    let object = self
                        .objects
                        .take(ObjectId::from_bits(value.payload), &mut self.budget)?;
                    let ObjectStorage::Aggregate(storage) = object.storage else {
                        return Err(ownership());
                    };
                    a.cleanup.push(Cleanup::Cells(storage.fields.into_iter()));
                } else {
                    return Err(ownership());
                }
            }
            Cleanup::Guard(guard) => {
                let field = self.constructions.get_mut(guard)?.completed.pop();
                if let Some(field) = field {
                    let value = self.constructions.get_mut(guard)?.fields[field as usize].take();
                    a.cleanup.push(Cleanup::Guard(guard));
                    if let Some(value) = value {
                        a.cleanup.push(Cleanup::Value(value));
                    }
                } else {
                    self.constructions.take(guard, &mut self.budget)?;
                }
            }
            Cleanup::Cells(mut fields) => {
                if let Some(cell) = fields.next_back() {
                    let field = self.cells.take(cell, &mut self.budget)?;
                    a.cleanup.push(Cleanup::Cells(fields));
                    match field.state {
                        CellState::Live(value) => a.cleanup.push(Cleanup::Value(value)),
                        CellState::Uninit => {}
                        _ => return Err(ownership()),
                    }
                }
            }
            Cleanup::Object(object) => {
                let record = self.objects.take(object, &mut self.budget)?;
                if !matches!(record.storage, ObjectStorage::Constructing) {
                    return Err(ownership());
                }
            }
        }
        Ok(())
    }
    /// One bounded Dispatch chunk; initial opcode charge belongs to the caller.
    /// Failure keeps private cleanup alive until drained, then returns its error.
    pub(crate) fn advance_constant_chunks(
        &mut self,
        a: &mut ConstantChunkActivation,
        steps_counter: &mut RunSteps,
    ) -> ConstantChunkPoll {
        if a.session != self.pattern_binding_session()
            || a.epoch != self.budget.epoch()
            || a.finished
        {
            return ConstantChunkPoll::Failed(stale());
        }
        let mut steps = 0usize;
        let mut bytes = 0usize;
        while steps < CHUNK_STEPS
            && bytes
                .checked_add(STEP_BYTES)
                .is_some_and(|n| n <= CHUNK_BYTES)
        {
            if a.failure.is_some() {
                if a.cleanup.is_empty()
                    && a.pending.is_none()
                    && a.commit.is_none()
                    && a.stack.is_empty()
                {
                    a.finished = true;
                    return ConstantChunkPoll::Failed(a.failure.take().unwrap_or_else(ownership));
                }
                if let Err(e) = self.cleanup_constant_chunk_step(a) {
                    return ConstantChunkPoll::Failed(e);
                }
            } else {
                let result = (|| -> Result<Option<PendingConstantResult>, Diagnostic> {
                    if let Some(c) = &mut a.commit {
                        if c.next < c.count {
                            self.chunk_commit_field(c)?;
                        } else {
                            a.pending = Some(self.finish_chunk_commit(&mut a.commit)?);
                        }
                    } else if let Some(node) = a.node.take() {
                        match node {
                            Node::Scalar(value) => a.pending = Some(self.ticket(value)?),
                            Node::Array {
                                ty,
                                guard,
                                storage,
                                children,
                            } => a
                                .stack
                                .push(self.start_chunk_array(ty, guard, storage, children)?),
                        }
                    } else if let Some(ticket) = a.pending.take() {
                        if let Some(b) = a.stack.last_mut() {
                            if !b.child_started {
                                a.pending = Some(ticket);
                                return Err(ownership());
                            }
                            if let Err(e) = self.construction_push(b.guard, b.next, ticket) {
                                a.pending = Some(ticket);
                                return Err(e);
                            }
                            b.next = b.next.checked_add(1).ok_or_else(resource)?;
                            b.child_started = false;
                        } else {
                            a.finished = true;
                            return Ok(Some(PendingConstantResult {
                                session: self.pattern_binding_session(),
                                epoch: a.epoch,
                                ticket: Some(ticket),
                            }));
                        }
                    } else if let Some(b) = a.stack.last_mut() {
                        if b.children.len() != 0 {
                            if b.child_started {
                                return Err(ownership());
                            }
                            steps_counter.charge();
                            a.fields_charged =
                                a.fields_charged.checked_add(1).ok_or_else(resource)?;
                            b.child_started = true;
                            a.node = Some(b.children.next().ok_or_else(ownership)?);
                        } else {
                            let mut b = Some(a.stack.pop().ok_or_else(ownership)?);
                            match self.start_chunk_commit(&mut b) {
                                Ok(c) => a.commit = Some(c),
                                Err(e) => {
                                    a.stack.push(b.take().ok_or_else(ownership)?);
                                    return Err(e);
                                }
                            }
                        }
                    } else {
                        return Err(ownership());
                    }
                    Ok(None)
                })();
                match result {
                    Ok(Some(value)) => return ConstantChunkPoll::Ready(value),
                    Ok(None) => {}
                    Err(e) => {
                        if let Err(e) = self.constant_chunk_failure(a, e) {
                            return ConstantChunkPoll::Failed(e);
                        }
                    }
                }
            }
            steps += 1;
            bytes += STEP_BYTES;
        }
        ConstantChunkPoll::Dispatch {
            steps: steps as u16,
            bytes: bytes as u32,
            cleaning: a.cleaning(),
        }
    }
    pub(crate) fn cleanup_ready_constant_chunks(
        &mut self,
        a: &mut ConstantChunkActivation,
        pending: PendingConstantResult,
        error: Diagnostic,
    ) -> Result<(), Diagnostic> {
        if a.session != self.pattern_binding_session() || a.epoch != self.budget.epoch() {
            return Err(stale());
        }
        self.validate_pending_constant(&pending)?;
        a.finished = false;
        a.pending = pending.ticket;
        self.constant_chunk_failure(a, error)
    }
}

#[cfg(test)]
#[path = "constant_chunk_tests.rs"]
mod tests;
