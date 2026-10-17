//! The `owned_box` lang item: rustc's builtin box dereference.
//!
//! A `Box<T>` value is an object of the alloc facade's `Box` declaration
//! (`pub struct Box<T: ?Sized>(T)`), constructed by its ordinary `Box::new`
//! body. Its single field cell is the box's heap allocation: moving the box
//! moves the owning object, so the cell keeps its identity (as a heap
//! allocation does), and dropping the box runs ordinary drop glue over that
//! cell before the object is released. `*b` is not a `Deref::deref` call but
//! the builtin place projection `Projection::BoxContents`, selected here by
//! the box place's reached descriptor exactly as a reference place selects
//! `Place::deref`. The projected place is then read, written, borrowed and
//! moved out of through the ordinary place operations.
use super::*;

impl GuestStore {
    /// Install the run's `owned_box` lang item, found with the run's impl
    /// member index (absent in a world without the alloc facade).
    pub(crate) fn install_owned_box(&mut self, identity: Option<crate::core::digest::Digest>) {
        self.owned_box = identity;
    }
    /// The run's `owned_box` lang item, when installed.
    pub(crate) fn owned_box(&self) -> Option<crate::core::digest::Digest> {
        self.owned_box
    }
    /// `Some(T)` when `ty` is a `Box<T>` descriptor.
    pub(crate) fn box_contents_type(&self, ty: TypeId) -> Result<Option<TypeId>, Diagnostic> {
        Ok(match self.types.descriptor(ty)?.key.as_ref() {
            TypeKey::Nominal { identity, args, .. }
                if self.owned_box == Some(*identity) && args.len() == 1 =>
            {
                Some(args[0])
            }
            _ => None,
        })
    }
    /// `*b` of a live box place: the place of its heap cell. Address
    /// formation checks the box object and its field ownership but does not
    /// read the contents (a later read, borrow or assignment checks them).
    pub(super) fn dereference_box(&self, place: &Place) -> Result<Place, PlaceFailure> {
        let projected = place.projected(Projection::BoxContents)?;
        self.resolve_place_for_use(&projected, PlaceUse::Address)?;
        Ok(projected)
    }
    /// The heap cell of a box value (`None` for any other value). Reading
    /// never moves or borrows; the caller only inspects the cell.
    pub(crate) fn box_contents_cell(&self, value: ValueRepr) -> Result<Option<CellId>, Diagnostic> {
        if value.kind != 8 || self.box_contents_type(TypeId(value.type_id))?.is_none() {
            return Ok(None);
        }
        let ObjectStorage::Aggregate(storage) =
            &self.objects.get(ObjectId::from_bits(value.payload))?.storage
        else {
            return Err(invalid());
        };
        match storage.fields.as_slice() {
            [contents] => Ok(Some(*contents)),
            _ => Err(invalid()),
        }
    }
    /// The heap cell of the live box a cell holds, if it holds one.
    pub(crate) fn boxed_cell_contents(&self, cell: CellId) -> Result<Option<CellId>, Diagnostic> {
        match self.cells.get(cell)?.state {
            CellState::Live(value) => self.box_contents_cell(value),
            _ => Ok(None),
        }
    }
    /// The heap cell of the box a reference designates (`&b`, `&&b`, an
    /// autoref'd format argument), `None` for any other value.
    pub(crate) fn referent_box_contents(&self, value: ValueRepr) -> Result<Option<CellId>, Diagnostic> {
        if value.kind != 9 {
            return Ok(None);
        }
        let record = self.references.get(RefId::from_bits(value.payload))?;
        self.validate_reference_record(record)?;
        if self.box_contents_type(record.target)?.is_none() {
            return Ok(None);
        }
        match &record.lease {
            ReferenceLease::Location(location) => self.boxed_cell_contents(location.final_cell.cell),
            _ => Ok(None),
        }
    }
    /// rustc's `CoerceUnsized` for boxes: `Some(([T], N))` when `value` is a
    /// `Box<[T; N]>` and `expected` is `Box<[T]>`. Trait-object coercions
    /// (`Box<dyn Tr>`) keep the box's reached Type, as references do.
    pub(super) fn box_unsizing(
        &self,
        value: ValueRepr,
        expected: TypeId,
    ) -> Result<Option<(TypeId, u32)>, Diagnostic> {
        let (Some(from), Some(to)) = (
            self.box_contents_type(TypeId(value.type_id))?,
            self.box_contents_type(expected)?,
        ) else {
            return Ok(None);
        };
        match (
            self.types.descriptor(from)?.key.as_ref(),
            self.types.descriptor(to)?.key.as_ref(),
        ) {
            (TypeKey::Array { element, len }, TypeKey::Slice { element: slice })
                if element == slice =>
            {
                Ok(Some((to, *len)))
            }
            _ => Ok(None),
        }
    }
    /// The unsized box keeps its object and heap cell; only its Type becomes
    /// `Box<[T]>`, whose dereference is the cell's complete element window.
    pub(super) fn coerce_unsized_box(
        &mut self,
        value: ValueRepr,
        expected: TypeId,
    ) -> Result<ValueRepr, Diagnostic> {
        let object = ObjectId::from_bits(value.payload);
        let record = self.objects.get_mut(object)?;
        if record.type_id != TypeId(value.type_id) {
            return Err(ownership());
        }
        record.type_id = expected;
        Ok(ValueRepr {
            type_id: expected.0,
            ..value
        })
    }
    /// `Some(N)` when a `Box<[T]>` place's heap cell holds `[T; N]`: its
    /// dereference is that array's complete element window.
    pub(super) fn unsized_box_window(
        &self,
        parent: TypeId,
        cell: CellId,
    ) -> Result<Option<u32>, Diagnostic> {
        let Some(contents) = self.box_contents_type(parent)? else {
            return Ok(None);
        };
        if !matches!(self.types.descriptor(contents)?.key.as_ref(), TypeKey::Slice { .. }) {
            return Ok(None);
        }
        match self.types.descriptor(self.address_type(cell)?)?.key.as_ref() {
            TypeKey::Array { len, .. } => Ok(Some(*len)),
            _ => Err(invalid()),
        }
    }
    /// Every `Box<[T; N]>` descriptor has its `Box<[T]>` view, so coercion
    /// sites can name the unsized Type without interning (as every array
    /// Type has its `[T]` view).
    pub(super) fn intern_unsized_box_view(&mut self, ty: TypeId) -> Result<(), Diagnostic> {
        if let Some(key) = self.unsized_box_key(ty)? {
            self.types.intern(key)?;
        }
        Ok(())
    }
    /// The `Box<[T]>` view of a `Box<[T; N]>` descriptor (`None` otherwise).
    pub(crate) fn unsized_box_view(&self, ty: TypeId) -> Result<Option<TypeId>, Diagnostic> {
        Ok(self
            .unsized_box_key(ty)?
            .and_then(|key| self.types.find(&key)))
    }
    fn unsized_box_key(&self, ty: TypeId) -> Result<Option<TypeKey>, Diagnostic> {
        let Some(contents) = self.box_contents_type(ty)? else {
            return Ok(None);
        };
        let TypeKey::Array { element, .. } = self.types.descriptor(contents)?.key.as_ref() else {
            return Ok(None);
        };
        let Some(slice) = self.types.find(&TypeKey::Slice { element: *element }) else {
            return Ok(None);
        };
        let TypeKey::Nominal {
            identity,
            definition,
            schema,
            const_args,
            ..
        } = self.types.descriptor(ty)?.key.as_ref()
        else {
            return Ok(None);
        };
        Ok(Some(TypeKey::Nominal {
            identity: *identity,
            definition: definition.clone(),
            schema: *schema,
            args: Box::new([slice]),
            const_args: const_args.clone(),
        }))
    }
}
