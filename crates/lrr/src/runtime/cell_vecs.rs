//! The growable-array lang item: `alloc::vec::Vec<T>`.
//!
//! A `Vec<T>` value is an aggregate object of the alloc facade's `Vec`
//! declaration (`pub struct Vec<T>;`, constructed by its ordinary `Vec::new`
//! body) whose fields are its element cells, in index order. The number of
//! fields grows and shrinks with the vector, so the object's committed
//! declaration has no fields of its own: every element cell is created and
//! retired here, under the same field ownership links, initialization
//! summary and storage contract (`Exact(T)`, the descriptor's argument) as an
//! array's element cells. Moving the vector moves the object; dropping it runs
//! ordinary drop glue over the element cells in index order (the cleanup
//! cursor walks the live field list), then releases the object.
//!
//! `v[i]` and `&v[a..b]` are the builtin element projections of a vector place
//! (as for arrays and slices), and `as_slice`/`as_mut_slice` are the same
//! `Elements` lease an array's unsizing coercion makes, over the vector place:
//! see `sequence_shape`.
use super::*;

/// A live `Vec` an intrinsic operates on.
struct VecAt {
    object: ObjectId,
    element: TypeId,
    len: usize,
}

impl GuestStore {
    /// Install the run's growable-array lang item, found with the run's impl
    /// member index (absent in a world without the alloc facade).
    pub(crate) fn install_owned_vec(&mut self, identity: Option<crate::core::digest::Digest>) {
        self.owned_vec = identity;
        self.types.set_vec_lang(identity);
    }
    /// The run's growable-array lang item, when installed.
    pub(crate) fn owned_vec(&self) -> Option<crate::core::digest::Digest> {
        self.owned_vec
    }
    /// `Some(T)` when `ty` is a `Vec<T>` descriptor.
    pub(crate) fn vec_element_type(&self, ty: TypeId) -> Result<Option<TypeId>, Diagnostic> {
        Ok(match self.types.descriptor(ty)?.key.as_ref() {
            TypeKey::Nominal { identity, args, .. }
                if self.owned_vec == Some(*identity) && args.len() == 1 =>
            {
                Some(args[0])
            }
            _ => None,
        })
    }
    /// Element type and element count of an array-like place: `[T; N]` or
    /// the live `Vec<T>` in `cell`. `None` for any other type.
    pub(super) fn sequence_shape(
        &self,
        cell: CellId,
        ty: TypeId,
    ) -> Result<Option<(TypeId, u32)>, Diagnostic> {
        if let TypeKey::Array { element, len } = self.types.descriptor(ty)?.key.as_ref() {
            return Ok(Some((*element, *len)));
        }
        let Some(element) = self.vec_element_type(ty)? else {
            return Ok(None);
        };
        let (_, storage) = self.place_object(cell)?;
        let len = u32::try_from(storage.fields.len()).map_err(|_| resource())?;
        Ok(Some((element, len)))
    }
    /// Whether `ty` is a `Vec<T>` descriptor.
    pub(crate) fn is_vec_type(&self, ty: TypeId) -> Result<bool, Diagnostic> {
        Ok(self.vec_element_type(ty)?.is_some())
    }
    /// The vector a reference value designates.
    fn vec_at(&self, reference: ValueRepr, mutable: bool) -> Result<VecAt, Diagnostic> {
        if reference.kind != 9 {
            return Err(invalid());
        }
        let record = self.references.get(RefId::from_bits(reference.payload))?;
        self.validate_reference_record(record)?;
        if mutable && record.access != Access::Mutable {
            return Err(access_fault());
        }
        let cell = record.location()?.final_cell.cell;
        let ty = self.place_type(cell)?;
        let element = self.vec_element_type(ty)?.ok_or_else(|| {
            Diagnostic::new("LRR-E-OP-TYPE", "vector operation requires a Vec".into())
        })?;
        let (object, storage) = self.place_object(cell)?;
        Ok(VecAt {
            object,
            element,
            len: storage.fields.len(),
        })
    }
    /// `Vec::len` of the vector behind a shared or mutable reference.
    pub(crate) fn vec_len(&self, reference: ValueRepr) -> Result<u64, Diagnostic> {
        Ok(self.vec_at(reference, false)?.len as u64)
    }
    /// Insert the element a ticket owns at `index` (the end for `None`).
    /// The vector stays complete: the new element cell is linked already
    /// initialized, so no enclosing summary changes.
    pub(crate) fn vec_insert(
        &mut self,
        reference: ValueRepr,
        index: Option<u64>,
        element: TicketId,
    ) -> Result<(), Diagnostic> {
        let at = self.vec_at(reference, true)?;
        let index = match index {
            None => at.len,
            Some(index) => usize::try_from(index)
                .ok()
                .filter(|index| *index <= at.len)
                .ok_or_else(invalid)?,
        };
        let value = self.peek_ticket(element)?;
        // An unconstrained numeric literal takes the vector's element type.
        if matches!(value.kind, 11 | 12) {
            self.materialize(element, at.element)?;
        }
        let value = self.peek_ticket(element)?;
        self.validate_value(value)?;
        self.require_initialized_value(value)?;
        self.object_owner(value, ValueOwner::Ticket(element))?;
        if self.effective_type(value)? != Some(at.element) {
            return Err(Diagnostic::new(
                "LRR-E-CALL-ARG-TYPE",
                "vector element type differs from the stored value".into(),
            ));
        }
        let position = u32::try_from(index).map_err(|_| resource())?;
        self.cells.reserve(1, &mut self.budget)?;
        let cell = self.new_cell(
            StorageContract::Exact(at.element),
            true,
            CellOwner::Field {
                object: at.object,
                index: position,
            },
        )?;
        self.tickets.take(element, &mut self.budget)?;
        self.set_owner(value, ValueOwner::Cell(cell))?;
        let record = self.cells.get_mut(cell)?;
        record.state = CellState::Live(value);
        record.numeric_record = if matches!(value.kind, 11 | 12) {
            Some(LiteralId::from_bits(value.payload))
        } else {
            None
        };
        let ObjectStorage::Aggregate(storage) = &mut self.objects.get_mut(at.object)?.storage
        else {
            return Err(invalid());
        };
        storage.initialized.insert_field(index, true)?;
        storage.fields.insert(index, cell);
        self.renumber_vec_cells(at.object, index + 1)
    }
    /// Remove the element at `index` and return it as a transfer ticket. Its
    /// cell is retired, so a stale lease to it fails.
    pub(crate) fn vec_remove(
        &mut self,
        reference: ValueRepr,
        index: u64,
    ) -> Result<TicketId, Diagnostic> {
        let at = self.vec_at(reference, true)?;
        let index = usize::try_from(index)
            .ok()
            .filter(|index| *index < at.len)
            .ok_or_else(invalid)?;
        let cell = {
            let ObjectStorage::Aggregate(storage) = &self.objects.get(at.object)?.storage else {
                return Err(invalid());
            };
            storage.fields[index]
        };
        let value = self.admit_move(cell)?;
        self.tickets.reserve(1, &mut self.budget)?;
        let ticket = self
            .tickets
            .allocate(TransferTicket { value }, 0, &mut self.budget)
            .map(TicketId::from)?;
        self.set_owner(value, ValueOwner::Ticket(ticket))?;
        let record = self.cells.get_mut(cell)?;
        record.state = CellState::Dropped;
        record.retired = true;
        record.location_epoch = record.location_epoch.saturating_add(1);
        let ObjectStorage::Aggregate(storage) = &mut self.objects.get_mut(at.object)?.storage
        else {
            return Err(invalid());
        };
        storage.initialized.remove_field(index)?;
        storage.fields.remove(index);
        self.renumber_vec_cells(at.object, index)?;
        Ok(ticket)
    }
    /// Element cells at and after `from` record their new index.
    fn renumber_vec_cells(&mut self, object: ObjectId, from: usize) -> Result<(), Diagnostic> {
        let ObjectStorage::Aggregate(storage) = &self.objects.get(object)?.storage else {
            return Err(invalid());
        };
        let later: Vec<CellId> = storage.fields.get(from..).unwrap_or_default().to_vec();
        for (offset, cell) in later.into_iter().enumerate() {
            let index = u32::try_from(from + offset).map_err(|_| resource())?;
            self.cells.get_mut(cell)?.owner = CellOwner::Field { object, index };
        }
        Ok(())
    }
    /// `Vec::as_slice` / `as_mut_slice`: the vector's complete element window,
    /// the same `Elements` lease an array's unsizing coercion makes.
    pub(crate) fn vec_slice(
        &mut self,
        reference: ValueRepr,
        mutable: bool,
    ) -> Result<TicketId, Diagnostic> {
        let at = self.vec_at(reference, mutable)?;
        let slice = self.types.intern(TypeKey::Slice {
            element: at.element,
        })?;
        let id = RefId::from_bits(reference.payload);
        let access = if mutable {
            Access::Mutable
        } else {
            Access::Shared
        };
        let len = u32::try_from(at.len).map_err(|_| resource())?;
        let window = self.references.get(id)?.unsized_array(access, slice, len)?;
        let bytes = window.owned_bytes()?;
        self.references.reserve(1, &mut self.budget)?;
        let window: RefId = self
            .references
            .allocate(window, bytes, &mut self.budget)?
            .into();
        self.reference_ticket(window)
    }
    /// The text of a `Vec<u8>` as a shared `&str` (a copy of its bytes in the
    /// run's immutable string pool; the vector cannot change while the
    /// borrow it was made under lives).
    pub(crate) fn vec_str(&mut self, reference: ValueRepr) -> Result<TicketId, Diagnostic> {
        let at = self.vec_at(reference, false)?;
        if at.element != TypeId(9) {
            return Err(Diagnostic::new(
                "LRR-E-OP-TYPE",
                "string view requires a Vec<u8>".into(),
            ));
        }
        let cells = {
            let ObjectStorage::Aggregate(storage) = &self.objects.get(at.object)?.storage else {
                return Err(invalid());
            };
            storage.fields.clone()
        };
        let mut bytes = Vec::with_capacity(cells.len());
        for cell in cells {
            bytes.push(u8::try_from(self.read_scalar(cell)?.payload).map_err(|_| invalid())?);
        }
        self.immutable_str_ticket(&bytes)
    }
}

impl GuestStore {
    /// `<[T]>::swap`: exchange the values of two element cells of a mutable
    /// slice window (no moves out of the window: both cells stay live).
    pub(crate) fn slice_swap(
        &mut self,
        reference: ValueRepr,
        a: u64,
        b: u64,
    ) -> Result<(), Diagnostic> {
        if reference.kind != 9
            || self.references.get(RefId::from_bits(reference.payload))?.access != Access::Mutable
        {
            return Err(access_fault());
        }
        let cells = self.referent_element_cells(reference)?.ok_or_else(invalid)?;
        let cell = |index: u64| {
            usize::try_from(index)
                .ok()
                .and_then(|index| cells.get(index).copied())
                .ok_or_else(invalid)
        };
        let (first, second) = (cell(a)?, cell(b)?);
        self.swap_cell_values(first, second)
    }
    /// Exchange the values of two live cells (no move out of either).
    pub(crate) fn swap_cell_values(&mut self, first: CellId, second: CellId) -> Result<(), Diagnostic> {
        if first == second {
            return Ok(());
        }
        let (left, right) = (self.admit_move(first)?, self.admit_move(second)?);
        let (left_record, right_record) = (
            self.cells.get(first)?.numeric_record,
            self.cells.get(second)?.numeric_record,
        );
        self.set_owner(left, ValueOwner::Cell(second))?;
        self.set_owner(right, ValueOwner::Cell(first))?;
        let record = self.cells.get_mut(first)?;
        record.state = CellState::Live(right);
        record.numeric_record = right_record;
        let record = self.cells.get_mut(second)?;
        record.state = CellState::Live(left);
        record.numeric_record = left_record;
        Ok(())
    }
}

impl GuestStore {
    /// Move every element of the array value a ticket owns into the end of
    /// the vector, in index order (`vec![a, b, c]`): the array's element
    /// cells are moved out one by one and the emptied array object is
    /// released through its ordinary cleanup cursor.
    pub(crate) fn vec_append_array(
        &mut self,
        reference: ValueRepr,
        array: TicketId,
    ) -> Result<(), Diagnostic> {
        let at = self.vec_at(reference, true)?;
        let value = self.peek_ticket(array)?;
        self.object_owner(value, ValueOwner::Ticket(array))?;
        if value.kind != 8 {
            return Err(invalid());
        }
        match self.types.descriptor(TypeId(value.type_id))?.key.as_ref() {
            TypeKey::Array { element, .. } if *element == at.element => {}
            TypeKey::Array { .. } => {
                return Err(Diagnostic::new(
                    "LRR-E-CALL-ARG-TYPE",
                    "array element type differs from the vector's".into(),
                ));
            }
            _ => return Err(invalid()),
        }
        let cells = {
            let ObjectStorage::Aggregate(storage) =
                &self.objects.get(ObjectId::from_bits(value.payload))?.storage
            else {
                return Err(invalid());
            };
            storage.fields.clone()
        };
        for cell in cells {
            let element = self.move_from(cell)?;
            self.vec_insert(reference, None, element)?;
        }
        loop {
            match self.aggregate_cleanup_step(array)? {
                AggregateCleanupStep::Complete => break,
                AggregateCleanupStep::Skipped => {}
                AggregateCleanupStep::Field(_) => return Err(ownership()),
            }
        }
        self.discard(array)
    }
}

impl GuestStore {
    /// The text of a `&[u8]` window (the caller validated it as UTF-8): a
    /// copy of its bytes in the run's immutable string pool.
    pub(crate) fn bytes_as_str(&mut self, reference: ValueRepr) -> Result<TicketId, Diagnostic> {
        let cells = self.referent_element_cells(reference)?.ok_or_else(invalid)?;
        let mut bytes = Vec::with_capacity(cells.len());
        for cell in cells {
            bytes.push(u8::try_from(self.read_scalar(cell)?.payload).map_err(|_| invalid())?);
        }
        self.immutable_str_ticket(&bytes)
    }
}
