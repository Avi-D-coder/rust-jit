//! Canonical union bytes and stable typed cell views. No source lookup or parsing.
use super::layouts::NominalKind;
use super::*;
use crate::runtime::object::{MemoryRegion, PointerLeaf, PointerSlot};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MemoryView {
    pub(super) cell: CellId,
    pub(super) root: ObjectId,
    pub(super) offset: u64,
    pub(super) ty: TypeId,
    pub(super) aggregate: Option<ObjectId>,
    pub(super) union_field: bool,
}
impl GuestStore {
    pub(super) fn is_union_type(&self, ty: TypeId) -> bool {
        matches!(self.types.descriptor(ty).map(|d|d.key.as_ref()),
            Ok(TypeKey::Nominal { definition, .. }) if definition.item_kind == crate::source::manifest::ItemKind::Union)
    }
    fn memory_fields(
        &self,
        ty: TypeId,
    ) -> Result<Option<(Vec<TypeId>, Vec<u64>, Option<u32>)>, Diagnostic> {
        match self.types.descriptor(ty)?.key.as_ref() {
            TypeKey::Array { element, len } => {
                let count = usize::try_from(*len).map_err(|_| resource())?;
                let stride = self.layout_of(*element)?.size;
                let offsets = (0..*len)
                    .map(|i| stride.checked_mul(u64::from(i)).ok_or_else(invalid))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Some((vec![*element; count], offsets, None)))
            }
            TypeKey::Tuple(members) => {
                let offsets = crate::runtime::layout::univariant(
                    &self.layouts_of(members)?,
                    &crate::runtime::layout::Repr::default(),
                    crate::runtime::layout::StructKind::Sized,
                )
                .offsets;
                Ok(Some((members.to_vec(), offsets, None)))
            }
            TypeKey::Nominal { .. } => {
                let (declaration, instance) = self.declaration_of(ty)?;
                if declaration.kind == NominalKind::Union {
                    let instance = instance.ok_or_else(invalid)?;
                    let [members] = instance.as_slice() else {
                        return Err(invalid());
                    };
                    Ok(Some((members.clone(), vec![0; members.len()], Some(0))))
                } else {
                    Ok(self
                        .nominal_image_fields(ty)?
                        .map(|(members, offsets)| (members, offsets, Some(0))))
                }
            }
            _ => Ok(None),
        }
    }
    fn memory_region(&self, root: ObjectId) -> Result<&MemoryRegion, Diagnostic> {
        let ObjectStorage::Aggregate(storage) = &self.objects.get(root)?.storage else {
            return Err(invalid());
        };
        storage.memory.as_ref().ok_or_else(invalid)
    }
    pub(super) fn memory_view_value(&self, view: MemoryView) -> Result<ValueRepr, Diagnostic> {
        let memory = self.memory_region(view.root)?;
        if let Some(object) = view.aggregate {
            self.objects.get(object)?;
            return Ok(ValueRepr {
                kind: 8,
                type_id: view.ty.0,
                payload: object.bits(),
            });
        }
        let size = usize::try_from(self.layout_of(view.ty)?.size).map_err(|_| invalid())?;
        let range = Self::image_range(view.offset, size)?;
        if !memory
            .initialized
            .get(range)
            .ok_or_else(invalid)?
            .iter()
            .all(|bit| *bit)
        {
            return Err(Diagnostic::new(
                "LRR-E-UNINITIALIZED",
                "read of uninitialized shared storage bytes".into(),
            ));
        }
        if self.pointer_leaf_type(view.ty) {
            // The bytes hold an address; the reference or pointer recovered
            // from it (`refresh_pointer_slots`) is the value while they still
            // hold that address.
            let address = Self::slot_address(&memory.bytes, view.offset)?;
            return memory
                .pointers
                .iter()
                .find(|slot| slot.cell == view.cell && slot.address == address)
                .map(|slot| slot.value)
                .ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-POINTER-DANGLING",
                        format!(
                            "{} stored in shared storage designates no live guest location",
                            self.describe_type(view.ty)
                        ),
                    )
                });
        }
        self.image_scalar(view.ty, view.offset, &memory.bytes)?
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    format!(
                        "shared storage view of {} requires its value/provenance representation",
                        self.describe_type(view.ty)
                    ),
                )
            })
    }
    fn validate_memory_view(&self, view: MemoryView) -> Result<(), Diagnostic> {
        if let Some(object) = view.aggregate {
            if self.is_union_type(view.ty) {
                return Ok(());
            }
            let ObjectStorage::Aggregate(storage) = &self.objects.get(object)?.storage else {
                return Err(invalid());
            };
            for cell in &storage.fields {
                let CellState::View(child) = self.cells.get(*cell)?.state else {
                    return Err(invalid());
                };
                self.validate_memory_view(child)?;
            }
            Ok(())
        } else {
            self.memory_view_value(view).map(|_| ())
        }
    }
    pub(super) fn read_memory_view(&mut self, view: MemoryView) -> Result<TicketId, Diagnostic> {
        if self.is_union_type(view.ty) {
            let image = self.snapshot_memory_view(view)?;
            return self.install_union_memory(view.ty, image);
        }
        self.validate_memory_view(view)?;
        if let Some(ticket) = self.move_out_unique_reference(view)? {
            return Ok(ticket);
        }
        let image = self.snapshot_memory_view(view)?;
        self.read_plain_image(view.ty, 0, &image.bytes)
    }
    fn snapshot_memory_view(&self, view: MemoryView) -> Result<MemoryRegion, Diagnostic> {
        let region = self.memory_region(view.root)?;
        let size = usize::try_from(self.layout_of(view.ty)?.size).map_err(|_| invalid())?;
        let range = Self::image_range(view.offset, size)?;
        Ok(MemoryRegion {
            bytes: region
                .bytes
                .get(range.clone())
                .ok_or_else(invalid)?
                .to_vec(),
            initialized: region.initialized.get(range).ok_or_else(invalid)?.to_vec(),
            pointers: Vec::new(),
            leaves: Vec::new(),
        })
    }
    pub(super) fn copy_union_memory(&mut self, source: CellId) -> Result<TicketId, Diagnostic> {
        let ty = self.place_type(source)?;
        if !self.is_union_type(ty) {
            return Err(invalid());
        }
        let image = match self.cells.get(source)?.state {
            CellState::View(view) => self.snapshot_memory_view(view)?,
            CellState::Live(value) => self
                .memory_region(ObjectId::from_bits(value.payload))?
                .clone(),
            _ => return Err(ownership()),
        };
        self.install_union_memory(ty, image)
    }
    fn make_memory_view(
        &mut self,
        root: ObjectId,
        offset: u64,
        ty: TypeId,
        parent: ObjectId,
        index: u32,
    ) -> Result<CellId, Diagnostic> {
        let cell = self.new_cell(
            StorageContract::Exact(ty),
            true,
            CellOwner::Field {
                object: parent,
                index,
            },
        )?;
        let aggregate = if let Some((members, offsets, variant)) = self.memory_fields(ty)? {
            let object = ObjectId::from(self.objects.allocate(
                ObjectRecord {
                    type_id: ty,
                    storage: ObjectStorage::Constructing,
                    owner: Some(ValueOwner::Cell(cell)),
                },
                0,
                &mut self.budget,
            )?);
            let mut fields = Vec::with_capacity(members.len());
            for (i, (member, at)) in members.into_iter().zip(offsets).enumerate() {
                fields.push(self.make_memory_view(
                    root,
                    offset.checked_add(at).ok_or_else(invalid)?,
                    member,
                    object,
                    u32::try_from(i).map_err(|_| resource())?,
                )?);
            }
            let initialized = InitializedFields::complete(fields.len())?;
            self.objects.get_mut(object)?.storage = ObjectStorage::Aggregate(AggregateStorage {
                memory: None,
                fields,
                initialized,
                next_cleanup: 0,
                pending_cleanup: None,
                variant,
            });
            Some(object)
        } else {
            None
        };
        let union_field = self.is_union_type(self.objects.get(parent)?.type_id);
        self.cells.get_mut(cell)?.state = CellState::View(MemoryView {
            cell,
            root,
            offset,
            ty,
            aggregate,
            union_field,
        });
        Ok(cell)
    }
    pub(super) fn install_union_memory(
        &mut self,
        ty: TypeId,
        image: MemoryRegion,
    ) -> Result<TicketId, Diagnostic> {
        if !self.is_union_type(ty) {
            return Err(invalid());
        }
        let size = usize::try_from(self.layout_of(ty)?.size).map_err(|_| invalid())?;
        if image.bytes.len() != size || image.initialized.len() != size {
            return Err(invalid());
        }
        let (members, offsets, variant) = self.memory_fields(ty)?.ok_or_else(invalid)?;
        let object = ObjectId::from(self.objects.allocate(
            ObjectRecord {
                type_id: ty,
                storage: ObjectStorage::Constructing,
                owner: None,
            },
            0,
            &mut self.budget,
        )?);
        let mut fields = Vec::with_capacity(members.len());
        for (i, (member, offset)) in members.into_iter().zip(offsets).enumerate() {
            fields.push(self.make_memory_view(
                object,
                offset,
                member,
                object,
                u32::try_from(i).map_err(|_| resource())?,
            )?);
        }
        let initialized = InitializedFields::complete(fields.len())?;
        self.objects.get_mut(object)?.storage = ObjectStorage::Aggregate(AggregateStorage {
            memory: Some(image),
            fields,
            initialized,
            next_cleanup: 0,
            pending_cleanup: None,
            variant,
        });
        let mut leaves = Vec::new();
        self.pointer_leaves(object, &mut leaves)?;
        let ObjectStorage::Aggregate(storage) = &mut self.objects.get_mut(object)?.storage else {
            return Err(invalid());
        };
        let memory = storage.memory.as_mut().ok_or_else(invalid)?;
        memory.leaves = leaves;
        // A cloned region's slots belong to the union it was cloned from.
        memory.pointers.clear();
        self.refresh_pointer_slots(object)?;
        let value = ValueRepr {
            kind: 8,
            type_id: ty.0,
            payload: object.bits(),
        };
        self.validate_value(value)?;
        self.tickets.reserve(1, &mut self.budget)?;
        let ticket = TicketId::from(self.tickets.allocate(
            TransferTicket { value },
            0,
            &mut self.budget,
        )?);
        self.set_owner(value, ValueOwner::Ticket(ticket))?;
        Ok(ticket)
    }
    pub(crate) fn construction_union_type(
        &mut self,
        guard: ConstructionId,
    ) -> Result<Option<TypeId>, Diagnostic> {
        let record = self.constructions.get(guard)?;
        if let Some((ty, _)) = record.nominal {
            return Ok(self.is_union_type(ty).then_some(ty));
        }
        if record.pending_nominal.as_ref().is_some_and(|bound| {
            bound.definition().item_kind == crate::source::manifest::ItemKind::Union
        }) {
            return self.intern_pending_nominal(guard).map(Some);
        }
        Ok(None)
    }
    pub(super) fn commit_union_memory(
        &mut self,
        guard: ConstructionId,
        ty: TypeId,
    ) -> Result<TicketId, Diagnostic> {
        let record = self.constructions.get(guard)?;
        if record.state != ConstructionState::Building || record.nominal != Some((ty, 0)) {
            return Err(ownership());
        }
        let (members, _, _) = self.memory_fields(ty)?.ok_or_else(invalid)?;
        let values = record
            .completed
            .iter()
            .map(|i| Ok((*i, record.fields[*i as usize].ok_or_else(ownership)?)))
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let size = usize::try_from(self.layout_of(ty)?.size).map_err(|_| invalid())?;
        let mut image = MemoryRegion {
            bytes: vec![0; size],
            initialized: vec![false; size],
            pointers: Vec::new(),
            leaves: Vec::new(),
        };
        for (field, value) in values {
            let field_ty = *members.get(field as usize).ok_or_else(invalid)?;
            if !self.plain_type(field_ty) {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "shared storage initializer requires its value/provenance image".into(),
                ));
            }
            self.write_plain_image(value, field_ty, 0, &mut image.bytes)?;
            let length = usize::try_from(self.layout_of(field_ty)?.size).map_err(|_| invalid())?;
            image
                .initialized
                .get_mut(..length)
                .ok_or_else(invalid)?
                .fill(true);
        }
        let ticket = self.install_union_memory(ty, image)?;
        while let Some(source) = self.construction_cleanup_next(guard)? {
            self.consume_image_source(source)?;
        }
        Ok(ticket)
    }
    pub(super) fn write_memory_view(
        &mut self,
        view: MemoryView,
        incoming: TicketId,
    ) -> Result<(), Diagnostic> {
        let value = self.peek_ticket(incoming)?;
        self.object_owner(value, ValueOwner::Ticket(incoming))?;
        let bytes = self.prepare_memory_view_write(view, value)?;
        self.commit_memory_view_write(view, incoming, bytes)
    }
    pub(super) fn prepare_memory_view_write(
        &mut self,
        view: MemoryView,
        value: ValueRepr,
    ) -> Result<Vec<u8>, Diagnostic> {
        self.admit_new_contract(Some(view.ty), value)?;
        if !self.plain_type(view.ty) {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "shared storage assignment requires its value/provenance image".into(),
            ));
        }
        let length = usize::try_from(self.layout_of(view.ty)?.size).map_err(|_| invalid())?;
        let mut bytes = vec![0; length];
        self.write_plain_image(value, view.ty, 0, &mut bytes)?;
        let range = Self::image_range(view.offset, length)?;
        let memory = self.memory_region(view.root)?;
        memory.bytes.get(range.clone()).ok_or_else(invalid)?;
        memory.initialized.get(range).ok_or_else(invalid)?;
        Ok(bytes)
    }
    pub(super) fn commit_memory_view_write(
        &mut self,
        view: MemoryView,
        incoming: TicketId,
        bytes: Vec<u8>,
    ) -> Result<(), Diagnostic> {
        let ObjectStorage::Aggregate(storage) = &mut self.objects.get_mut(view.root)?.storage
        else {
            return Err(invalid());
        };
        let memory = storage.memory.as_mut().ok_or_else(invalid)?;
        let range = Self::image_range(view.offset, bytes.len())?;
        memory
            .bytes
            .get_mut(range.clone())
            .ok_or_else(invalid)?
            .copy_from_slice(&bytes);
        memory
            .initialized
            .get_mut(range)
            .ok_or_else(invalid)?
            .fill(true);
        self.refresh_pointer_slots(view.root)?;
        self.consume_image_source(incoming)
    }

    /// Whether a view of `ty` holds an address that reads back as a pointer
    /// or reference: a thin raw pointer, fn pointer or thin reference.
    fn pointer_leaf_type(&self, ty: TypeId) -> bool {
        match self.types.descriptor(ty).map(|d| d.key.as_ref()) {
            Ok(TypeKey::RawPtr { target, .. } | TypeKey::Ref { target, .. }) => {
                !self.unsized_pointee(*target)
            }
            Ok(TypeKey::Function { .. }) => true,
            _ => false,
        }
    }
    fn unique_reference_leaf(&self, ty: TypeId) -> bool {
        matches!(
            self.types.descriptor(ty).map(|d| d.key.as_ref()),
            Ok(TypeKey::Ref { mutable: true, .. })
        ) && self.pointer_leaf_type(ty)
    }
    fn slot_address(bytes: &[u8], offset: u64) -> Result<u64, Diagnostic> {
        let range = Self::image_range(offset, 8)?;
        let word: [u8; 8] = bytes
            .get(range)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?;
        Ok(u64::from_le_bytes(word))
    }
    /// Every pointer-typed leaf view below a union's field views.
    fn pointer_leaves(&self, object: ObjectId, leaves: &mut Vec<PointerLeaf>) -> Result<(), Diagnostic> {
        let ObjectStorage::Aggregate(storage) = &self.objects.get(object)?.storage else {
            return Err(invalid());
        };
        for cell in &storage.fields {
            let CellState::View(view) = self.cells.get(*cell)?.state else {
                return Err(invalid());
            };
            match view.aggregate {
                Some(child) => self.pointer_leaves(child, leaves)?,
                None if self.pointer_leaf_type(view.ty) => leaves.push(PointerLeaf {
                    cell: *cell,
                    offset: view.offset,
                    ty: view.ty,
                }),
                None => {}
            }
        }
        Ok(())
    }
    fn is_unique_record(&self, value: ValueRepr) -> Result<bool, Diagnostic> {
        Ok(value.kind == 9
            && self.references.get(RefId::from_bits(value.payload))?.access == Access::Mutable)
    }
    /// Recover the reference or pointer each pointer-typed view of a union's
    /// bytes now names. The bytes are the one source of truth: a slot is the
    /// value `pointer_from_address` gives for the address the view's bytes
    /// hold (the ordinary `&*(address as *const T)` reborrow), kept while
    /// that address is unchanged. An address that names no live cell of the
    /// pointee type has no slot, and a view read then reports a dangling
    /// reference; the write itself never fails over it. A recovered `&mut`
    /// belongs to its view cell, so reading it in place (`*u.r = 1`,
    /// `&mut *u.r`) is the same checked access as through any `&mut` local.
    fn refresh_pointer_slots(&mut self, root: ObjectId) -> Result<(), Diagnostic> {
        let memory = self.memory_region(root)?;
        if memory.leaves.is_empty() && memory.pointers.is_empty() {
            return Ok(());
        }
        let mut kept = Vec::with_capacity(memory.leaves.len());
        let mut recover = Vec::new();
        for leaf in &memory.leaves {
            let range = Self::image_range(leaf.offset, 8)?;
            let covered = memory.initialized.get(range).ok_or_else(invalid)?;
            if !covered.iter().all(|bit| *bit) {
                continue;
            }
            let address = Self::slot_address(&memory.bytes, leaf.offset)?;
            match memory
                .pointers
                .iter()
                .find(|slot| slot.cell == leaf.cell && slot.address == address)
            {
                Some(slot) => kept.push(*slot),
                None => recover.push((*leaf, address)),
            }
        }
        // The superseded slots of this union's own views release their lease.
        let superseded: Vec<ValueRepr> = memory
            .pointers
            .iter()
            .filter(|slot| {
                memory.leaves.iter().any(|leaf| leaf.cell == slot.cell)
                    && !kept.iter().any(|keep| keep.cell == slot.cell)
            })
            .map(|slot| slot.value)
            .collect();
        for value in superseded {
            if self.is_unique_record(value)? {
                self.references.take(RefId::from_bits(value.payload), &mut self.budget)?;
            }
        }
        for (leaf, address) in recover {
            // A failed recovery is the absence of a slot, reported when the
            // view is read.
            let Ok(ticket) = self.pointer_from_address(address, leaf.ty) else {
                continue;
            };
            let value = self.peek_ticket(ticket)?;
            if self.is_unique_record(value)? {
                self.tickets.take(ticket, &mut self.budget)?;
                self.set_owner(value, ValueOwner::Cell(leaf.cell))?;
            } else {
                self.discard(ticket)?;
            }
            kept.push(PointerSlot { cell: leaf.cell, address, value });
        }
        let ObjectStorage::Aggregate(storage) = &mut self.objects.get_mut(root)?.storage else {
            return Err(invalid());
        };
        storage.memory.as_mut().ok_or_else(invalid)?.pointers = kept;
        Ok(())
    }
    /// Moving a `&mut` out of its view hands out the view's own lease, once:
    /// rustc treats the union as moved from afterwards, so the slot is not
    /// left to be read a second time.
    fn move_out_unique_reference(&mut self, view: MemoryView) -> Result<Option<TicketId>, Diagnostic> {
        if view.aggregate.is_some() || !self.unique_reference_leaf(view.ty) {
            return Ok(None);
        }
        let memory = self.memory_region(view.root)?;
        let address = Self::slot_address(&memory.bytes, view.offset)?;
        let Some(index) = memory
            .pointers
            .iter()
            .position(|slot| slot.cell == view.cell && slot.address == address)
        else {
            return Ok(None);
        };
        let value = memory.pointers[index].value;
        self.tickets.reserve(1, &mut self.budget)?;
        let ticket = self
            .tickets
            .allocate(TransferTicket { value }, 0, &mut self.budget)
            .map(TicketId::from)?;
        self.set_owner(value, ValueOwner::Ticket(ticket))?;
        let ObjectStorage::Aggregate(storage) = &mut self.objects.get_mut(view.root)?.storage else {
            return Err(invalid());
        };
        storage.memory.as_mut().ok_or_else(invalid)?.pointers.swap_remove(index);
        Ok(Some(ticket))
    }
    /// A retiring view gives up its slot; a recovered `&mut` lease is destroyed
    /// with it, never the referent.
    fn release_pointer_slot(&mut self, view: MemoryView) -> Result<(), Diagnostic> {
        let ObjectStorage::Aggregate(storage) = &mut self.objects.get_mut(view.root)?.storage else {
            return Ok(());
        };
        let Some(memory) = storage.memory.as_mut() else {
            return Ok(());
        };
        let Some(index) = memory.pointers.iter().position(|slot| slot.cell == view.cell) else {
            return Ok(());
        };
        let slot = memory.pointers.swap_remove(index);
        if self.is_unique_record(slot.value)? {
            self.references.take(RefId::from_bits(slot.value.payload), &mut self.budget)?;
        }
        Ok(())
    }
    pub(super) fn retire_memory_views(&mut self, fields: Vec<CellId>) -> Result<(), Diagnostic> {
        for cell in fields {
            if let CellState::View(view) = self.cells.get(cell)?.state
                && view.aggregate.is_none()
                && self.pointer_leaf_type(view.ty)
            {
                self.release_pointer_slot(view)?;
            }
            if let CellState::View(view) = self.cells.get(cell)?.state
                && let Some(object) = view.aggregate
            {
                let ObjectStorage::Aggregate(storage) = &self.objects.get(object)?.storage else {
                    return Err(invalid());
                };
                self.retire_memory_views(storage.fields.clone())?;
                self.objects.take(object, &mut self.budget)?;
            }
            let entry = self.cells.get_mut(cell)?;
            entry.state = CellState::Dropped;
            entry.retired = true;
            entry.location_epoch = entry.location_epoch.saturating_add(1);
        }
        Ok(())
    }
}
