//! Raw pointers and function pointers (E-R5-008).
//!
//! A raw pointer or fn pointer value is a `Copy` kind-9 value over a `raw`
//! reference record. A thin pointer is a synthetic address
//! (`ReferenceLease::Detached`); the address table here, never a host
//! pointer, maps cells to deterministic addresses and back, so `p as usize`
//! and `usize as *const T` round-trip, `p == q` compares addresses, and
//! `p.add(n)` is address arithmetic. Using a pointer re-derives a lease from
//! its address at that moment (`GuestStore::cell_at`), so no lease or epoch
//! decision is made when the pointer value is formed or copied: a dangling
//! pointer is undefined behaviour in rustc, and the interpreter only reports
//! a fault when it is actually dereferenced. Fat slice and `str` pointers keep
//! the reference lease they were made from. A function item has one code
//! address per callee identity.
use super::*;
use crate::{
    core::digest::Digest,
    runtime::{
        reference::{BufferLease, ElementsLease, LocationLease},
        shape::{FacadeType, Primitive, primitive},
    },
};
use crate::runtime::layout as algorithm;
use std::collections::BTreeMap;

/// First synthetic data address and first code address. Any values would do;
/// these keep addresses non-null, aligned and visibly synthetic.
const DATA_START: u64 = 0x5000_0000;
const CODE_START: u64 = 0x4000;
/// Virtual range reserved for one vector's heap buffer, so later pushes stay
/// inside it.
const VECTOR_RESERVE: u64 = 1 << 24;

#[derive(Default)]
pub(super) struct PointerTable {
    next: u64,
    cells: BTreeMap<CellId, u64>,
    starts: BTreeMap<u64, Vec<CellId>>,
    buffers: BTreeMap<CellId, u64>,
    strings: Vec<(BufferLease, u64)>,
    functions: BTreeMap<Digest, u64>,
    code: BTreeMap<u64, Digest>,
}

fn align_up(value: u64, align: u64) -> u64 {
    let align = align.max(1);
    value.div_ceil(align).saturating_mul(align)
}
/// (size, alignment) of fields given in declaration order, placed as rustc
/// places `repr(Rust)` fields (`runtime/layout.rs`, E-R5-012).
fn struct_layout(fields: impl Iterator<Item = (u64, u64)>) -> (u64, u64) {
    let layout = algorithm::univariant(
        &plain_layouts(fields),
        &algorithm::Repr::default(),
        algorithm::StructKind::Sized,
    )
    .layout;
    (layout.size, layout.align)
}
fn plain_layouts(
    fields: impl Iterator<Item = (u64, u64)>,
) -> Vec<algorithm::Layout> {
    fields
        .map(|(size, align)| algorithm::Layout {
            size,
            align: align.max(1),
            niche: None,
            uninhabited: false,
        })
        .collect()
}
fn dangling() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-POINTER-DANGLING",
        "raw pointer designates no live guest location".into(),
    )
}
fn pointer_type_fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-OP-TYPE", message.into())
}
/// An independent copy of a lease (leases own their ancestry vectors).
fn clone_lease(lease: &ReferenceLease) -> ReferenceLease {
    let location = |base: &LocationLease| LocationLease {
        root: base.root,
        enclosing: base.enclosing.clone(),
        final_cell: base.final_cell,
        source_projections: base.source_projections.clone(),
    };
    match lease {
        ReferenceLease::Location(base) => ReferenceLease::Location(location(base)),
        ReferenceLease::Elements(elements) => ReferenceLease::Elements(ElementsLease {
            base: location(&elements.base),
            start: elements.start,
            len: elements.len,
        }),
        ReferenceLease::Buffer(buffer) => ReferenceLease::Buffer(*buffer),
        ReferenceLease::Detached(address) => ReferenceLease::Detached(*address),
    }
}

/// What a reference or pointer value designates, copied out of its record.
enum Designated {
    Address(u64),
    Cell(CellId),
    Elements { array: CellId, start: u32 },
    Buffer(BufferLease),
}

impl GuestStore {
    /// Whether two descriptors are both raw pointers or both fn pointers. A
    /// location typed as a pointer accepts any pointer of the same family:
    /// rustc has already proven the types agree up to coercion, and the
    /// interpreter never needs the distinction (looser than rustc, never
    /// stricter).
    pub(crate) fn pointer_family_interchange(&self, expected: TypeId, actual: TypeId) -> bool {
        let family = |ty: TypeId| match self.types.descriptor(ty).map(|d| d.key.as_ref()) {
            Ok(TypeKey::RawPtr { .. }) => 1,
            Ok(TypeKey::Function { .. }) => 2,
            _ => 0,
        };
        let expected = family(expected);
        expected != 0 && expected == family(actual)
    }
    /// Storage-contract admission: equal descriptors, or two pointers of one
    /// family (see `pointer_family_interchange`).
    pub(crate) fn contract_admits(&self, expected: Option<TypeId>, actual: Option<TypeId>) -> bool {
        expected == actual
            || matches!((expected, actual), (Some(e), Some(a)) if self.pointer_family_interchange(e, a))
            || matches!((expected, actual), (Some(e), Some(a)) if self.dyn_admits(e, a))
    }
    /// The common Type two array elements of different pointer Types coerce
    /// to (`[Box::new(A), Box::new(B)]` under a `Box<dyn T>` expectation the
    /// literal itself does not see): the same pointer shape over an
    /// unnamed-trait `dyn` pointee. Only valid programs reach here (rustc
    /// accepted the coercions), so no trait check is made.
    pub(crate) fn dyn_join(&mut self, a: TypeId, b: TypeId) -> Result<Option<TypeId>, Diagnostic> {
        if a == b {
            return Ok(Some(a));
        }
        if self.dyn_admits(a, b) {
            return Ok(Some(a));
        }
        if self.dyn_admits(b, a) {
            return Ok(Some(b));
        }
        let (ka, kb) = (
            self.types.descriptor(a)?.key.clone(),
            self.types.descriptor(b)?.key.clone(),
        );
        let any_dyn = |this: &mut Self| this.types.intern(TypeKey::Dyn { schema: Digest([0; 32]) });
        match (ka.as_ref(), kb.as_ref()) {
            (
                TypeKey::Ref { mutable: m1, target: t1 },
                TypeKey::Ref { mutable: m2, target: t2 },
            ) if m1 == m2 => {
                let target = match self.dyn_join(*t1, *t2)? {
                    Some(target) if !matches!(self.types.descriptor(*t1)?.key.as_ref(), TypeKey::Primitive(_)) => target,
                    _ => any_dyn(self)?,
                };
                Ok(Some(self.types.intern(TypeKey::Ref { mutable: *m1, target })?))
            }
            (
                TypeKey::Nominal { identity: i1, definition, schema, args: a1, const_args },
                TypeKey::Nominal { identity: i2, args: a2, .. },
            ) if i1 == i2 && self.owned_box == Some(*i1) && a1.len() == 1 && a2.len() == 1 => {
                let pointee = self.dyn_join(a1[0], a2[0])?;
                let pointee = match pointee {
                    Some(pointee) => pointee,
                    None => any_dyn(self)?,
                };
                Ok(Some(self.types.intern(TypeKey::Nominal {
                    identity: *i1,
                    definition: definition.clone(),
                    schema: *schema,
                    args: Box::new([pointee]),
                    const_args: const_args.clone(),
                })?))
            }
            (TypeKey::Tuple(m1), TypeKey::Tuple(m2)) if m1.len() == m2.len() => {
                let mut members = Vec::with_capacity(m1.len());
                for (x, y) in m1.iter().zip(m2.iter()) {
                    match self.dyn_join(*x, *y)? {
                        Some(joined) => members.push(joined),
                        None => return Ok(None),
                    }
                }
                Ok(Some(self.types.intern(TypeKey::Tuple(members.into_boxed_slice()))?))
            }
            _ => Ok(None),
        }
    }
    /// Whether a declared `dyn Trait` occurs anywhere in this descriptor.
    pub(crate) fn mentions_dyn(&self, ty: TypeId) -> bool {
        let Ok(descriptor) = self.types.descriptor(ty) else {
            return false;
        };
        match descriptor.key.as_ref() {
            TypeKey::Dyn { .. } => true,
            TypeKey::Ref { target, .. } | TypeKey::RawPtr { target, .. } => {
                self.mentions_dyn(*target)
            }
            TypeKey::Slice { element } | TypeKey::Array { element, .. } => {
                self.mentions_dyn(*element)
            }
            TypeKey::Tuple(members) => members.iter().any(|member| self.mentions_dyn(*member)),
            TypeKey::Nominal { args, .. } => args.iter().any(|arg| self.mentions_dyn(*arg)),
            TypeKey::Function { params, result } => {
                params.iter().any(|param| self.mentions_dyn(*param)) || self.mentions_dyn(*result)
            }
            _ => false,
        }
    }
    /// Pure counterpart of `dyn_join`'s admissibility: both Types are
    /// references of one mutability, or boxes.
    pub(crate) fn dyn_joinable(&self, a: TypeId, b: TypeId) -> bool {
        let (Ok(a), Ok(b)) = (self.types.descriptor(a), self.types.descriptor(b)) else {
            return false;
        };
        match (a.key.as_ref(), b.key.as_ref()) {
            (TypeKey::Ref { mutable: m1, .. }, TypeKey::Ref { mutable: m2, .. }) => m1 == m2,
            (TypeKey::Nominal { identity: i1, args: a1, .. }, TypeKey::Nominal { identity: i2, args: a2, .. }) => {
                i1 == i2 && self.owned_box == Some(*i1) && a1.len() == 1 && a2.len() == 1
            }
            (TypeKey::Tuple(m1), TypeKey::Tuple(m2)) => {
                m1.len() == m2.len()
                    && m1
                        .iter()
                        .zip(m2.iter())
                        .all(|(x, y)| x == y || self.dyn_admits(*x, *y) || self.dyn_admits(*y, *x) || self.dyn_joinable(*x, *y))
            }
            _ => false,
        }
    }
    /// A declared `dyn Trait` position admits the concrete Type of any value
    /// (values keep their concrete runtime Type; methods dispatch on it).
    /// Structure around it matches position by position: `Box<dyn T>` admits
    /// `Box<A>`, `&dyn T` admits `&A`, `Vec<Box<dyn T>>` admits `Vec<Box<A>>`.
    pub(crate) fn dyn_admits(&self, expected: TypeId, actual: TypeId) -> bool {
        if expected == actual {
            return true;
        }
        let (Ok(e), Ok(a)) = (self.types.descriptor(expected), self.types.descriptor(actual))
        else {
            return false;
        };
        match (e.key.as_ref(), a.key.as_ref()) {
            (TypeKey::Dyn { .. }, _) => true,
            (
                TypeKey::Ref { mutable: m1, target: t1 },
                TypeKey::Ref { mutable: m2, target: t2 },
            )
            | (
                TypeKey::RawPtr { mutable: m1, target: t1 },
                TypeKey::RawPtr { mutable: m2, target: t2 },
            ) => m1 == m2 && self.dyn_admits(*t1, *t2),
            (TypeKey::Slice { element: e1 }, TypeKey::Slice { element: e2 }) => {
                self.dyn_admits(*e1, *e2)
            }
            (
                TypeKey::Array { element: e1, len: l1 },
                TypeKey::Array { element: e2, len: l2 },
            ) => l1 == l2 && self.dyn_admits(*e1, *e2),
            (TypeKey::Tuple(m1), TypeKey::Tuple(m2)) => {
                m1.len() == m2.len() && m1.iter().zip(m2.iter()).all(|(x, y)| self.dyn_admits(*x, *y))
            }
            (
                TypeKey::Nominal { identity: i1, schema: s1, args: a1, const_args: c1, .. },
                TypeKey::Nominal { identity: i2, schema: s2, args: a2, const_args: c2, .. },
            ) => {
                i1 == i2
                    && s1 == s2
                    && c1 == c2
                    && a1.len() == a2.len()
                    && a1.iter().zip(a2.iter()).all(|(x, y)| self.dyn_admits(*x, *y))
            }
            (
                TypeKey::Function { params: p1, result: r1 },
                TypeKey::Function { params: p2, result: r2 },
            ) => {
                p1.len() == p2.len()
                    && p1.iter().zip(p2.iter()).all(|(x, y)| self.dyn_admits(*x, *y))
                    && self.dyn_admits(*r1, *r2)
            }
            _ => false,
        }
    }
    /// Apply an admitted pointer coercion to a reference or callable value
    /// (the pointer arm of `coerce_unsized_value`). A consumed `&mut`
    /// reference loses its lease, as in every other moved-reference coercion.
    pub(super) fn coerce_to_pointer(
        &mut self,
        value: ValueRepr,
        expected: TypeId,
    ) -> Result<ValueRepr, Diagnostic> {
        let coerced = self.pointer_cast(value, expected)?;
        if value.kind == 9 {
            let id = RefId::from_bits(value.payload);
            if self.references.get(id)?.access == Access::Mutable {
                self.references.take(id, &mut self.budget)?;
            }
        }
        Ok(coerced)
    }
    /// Install the pointer descriptor `*const T` / `*mut T`.
    pub(crate) fn raw_pointer_type(
        &mut self,
        mutable: bool,
        target: TypeId,
    ) -> Result<TypeId, Diagnostic> {
        self.types.intern(TypeKey::RawPtr { mutable, target })
    }

    // ---- Synthetic layout -------------------------------------------------

    fn pointer_width(&self, pointee: TypeId) -> u64 {
        if pointee.0 == 16 {
            return 16;
        }
        match self.types.descriptor(pointee).map(|d| d.key.as_ref()) {
            Ok(TypeKey::Slice { .. }) => 16,
            _ => 8,
        }
    }
    /// Best-effort (size, alignment) of a type with no live cell.
    pub(crate) fn type_layout(&self, ty: TypeId) -> (u64, u64) {
        // The one layout algorithm, once the declarations its type needs are
        // known (a prior `size_of`); a structural estimate otherwise.
        if let Ok(layout) = self.layout_of(ty) {
            return (layout.size, layout.align);
        }
        if let Ok(scalar) = primitive(ty) {
            return match scalar {
                Primitive::Unit | Primitive::Str => (0, 1),
                Primitive::Bool => (1, 1),
                Primitive::Char | Primitive::F32 => (4, 4),
                Primitive::F64 => (8, 8),
                Primitive::Signed(width) | Primitive::Unsigned(width) => {
                    let bytes = u64::from(width) / 8;
                    (bytes, bytes)
                }
            };
        }
        let Ok(descriptor) = self.types.descriptor(ty) else {
            return (8, 8);
        };
        match descriptor.key.as_ref() {
            TypeKey::Primitive(_) | TypeKey::Nominal { .. } => (8, 8),
            TypeKey::Ref { target, .. } | TypeKey::RawPtr { target, .. } => {
                (self.pointer_width(*target), 8)
            }
            TypeKey::Function { .. } => (8, 8),
            TypeKey::Array { element, len } => {
                let (size, align) = self.type_layout(*element);
                (size.saturating_mul(u64::from(*len)), align)
            }
            TypeKey::Slice { element } => (0, self.type_layout(*element).1),
            TypeKey::Dyn { .. } => (0, 1),
            TypeKey::Tuple(members) => struct_layout(members.iter().map(|m| self.type_layout(*m))),
            TypeKey::Callable { captures, .. } => {
                struct_layout(captures.iter().map(|m| self.type_layout(*m)))
            }
            TypeKey::Facade {
                family: FacadeType::String,
                ..
            } => (24, 8),
            TypeKey::Facade { .. } => (8, 8),
        }
    }
    /// Best-effort (size, alignment) of the value a cell holds, from its live
    /// structure.
    pub(crate) fn cell_layout(&self, cell: CellId) -> (u64, u64) {
        let Ok(record) = self.cells.get(cell) else {
            return (8, 8);
        };
        if let CellState::View(view) = record.state {
            return self.layout_of(view.ty).map(|layout|(layout.size,layout.align)).unwrap_or((0,1));
        }
        let Ok(CellState::Live(value)) = self.state(cell) else {
            return match record.declared {
                StorageContract::Exact(ty) => self.type_layout(ty),
                _ => (8, 8),
            };
        };
        match value.kind {
            11 => (4, 4),
            12 => (8, 8),
            8 => {
                let Ok(object) = self.objects.get(ObjectId::from_bits(value.payload)) else {
                    return (8, 8);
                };
                match &object.storage {
                    ObjectStorage::String(_) => (24, 8),
                    ObjectStorage::Constructing => (8, 8),
                    ObjectStorage::Aggregate(storage) => {
                        if let Ok(layout) = self.layout_of(TypeId(value.type_id)) {
                            return (layout.size, layout.align);
                        }
                        if let Ok(TypeKey::Nominal { identity, .. }) = self
                            .types
                            .descriptor(TypeId(value.type_id))
                            .map(|d| d.key.as_ref())
                        {
                            if self.owned_vec == Some(*identity) {
                                return (24, 8);
                            }
                            if self.owned_box == Some(*identity) {
                                return (8, 8);
                            }
                        }
                        let fields =
                            plain_layouts(storage.fields.iter().map(|f| self.cell_layout(*f)));
                        let kind = if storage.variant.is_some() {
                            algorithm::StructKind::Prefixed { size: 1, align: 1 }
                        } else {
                            algorithm::StructKind::Sized
                        };
                        let layout = algorithm::univariant(
                            &fields,
                            &self.declared_repr(TypeId(value.type_id)),
                            kind,
                        )
                        .layout;
                        (layout.size, layout.align)
                    }
                }
            }
            _ => self.type_layout(TypeId(value.type_id)),
        }
    }
    /// The per-element stride of a sequence of element cells.
    fn element_stride(&self, fields: &[CellId]) -> u64 {
        let (mut size, mut align) = (0u64, 1u64);
        for field in fields {
            let (s, a) = self.cell_layout(*field);
            size = size.max(s);
            align = align.max(a);
        }
        align_up(size, align)
    }
    /// Children of a cell that live inside its own allocation, with their
    /// offsets from it. A vector's elements and a box's contents own another
    /// allocation and are not included.
    fn child_offsets(&self, cell: CellId) -> Vec<(CellId, u64)> {
        let Ok(record) = self.cells.get(cell) else {
            return Vec::new();
        };
        let Ok(CellState::Live(value)) = self.state(cell) else {
            return Vec::new();
        };
        if value.kind != 8 {
            return Vec::new();
        }
        let Ok(object) = self.objects.get(ObjectId::from_bits(value.payload)) else {
            return Vec::new();
        };
        let ObjectStorage::Aggregate(storage) = &object.storage else {
            return Vec::new();
        };
        match self
            .types
            .descriptor(TypeId(value.type_id))
            .map(|d| d.key.as_ref())
        {
            Ok(TypeKey::Nominal { identity, .. })
                if self.owned_vec == Some(*identity) || self.owned_box == Some(*identity) =>
            {
                Vec::new()
            }
            Ok(TypeKey::Array { .. }) => {
                let stride = self.element_stride(&storage.fields);
                storage
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, cell)| (*cell, (index as u64).saturating_mul(stride)))
                    .collect()
            }
            Ok(TypeKey::Nominal { definition, .. }) if definition.item_kind == crate::source::manifest::ItemKind::Union => {
                storage.fields.iter().copied().map(|cell|(cell,0)).collect()
            }
            _ => {
                // The same placement the layout module gives a struct, tuple
                // or enum variant (a one-byte tag prefix for the latter).
                let fields =
                    plain_layouts(storage.fields.iter().map(|f| self.cell_layout(*f)));
                let kind = if matches!(self.types.descriptor(TypeId(value.type_id)).map(|d|d.key.as_ref()), Ok(TypeKey::Nominal {definition,..}) if definition.item_kind==crate::source::manifest::ItemKind::Enum) {
                    algorithm::StructKind::Prefixed { size: 1, align: 1 }
                } else {
                    algorithm::StructKind::Sized
                };
                let placed = algorithm::univariant(
                    &fields,
                    &self.declared_repr(TypeId(value.type_id)),
                    kind,
                );
                storage
                    .fields
                    .iter()
                    .copied()
                    .zip(placed.offsets)
                    .collect()
            }
        }
    }

    // ---- Address table ----------------------------------------------------

    fn claim_range(&mut self, size: u64, align: u64) -> u64 {
        let start = if self.pointers.next == 0 {
            DATA_START
        } else {
            self.pointers.next
        };
        let base = align_up(start, align.max(8));
        self.pointers.next = align_up(base.saturating_add(size.max(1)), 8);
        base
    }
    fn register_start(&mut self, address: u64, cell: CellId) {
        let cells = self.pointers.starts.entry(address).or_default();
        if !cells.contains(&cell) {
            cells.push(cell);
        }
    }
    /// Record `cell` and every descendant inside its allocation.
    fn register_subtree(&mut self, cell: CellId, address: u64) {
        let mut pending = vec![(cell, address)];
        while let Some((cell, address)) = pending.pop() {
            match self.pointers.cells.insert(cell, address) {
                Some(previous) if previous == address => {}
                Some(previous) => {
                    if let Some(cells) = self.pointers.starts.get_mut(&previous) {
                        cells.retain(|c| *c != cell);
                    }
                    self.register_start(address, cell);
                }
                None => self.register_start(address, cell),
            }
            for (child, offset) in self.child_offsets(cell) {
                pending.push((child, address.wrapping_add(offset)));
            }
        }
    }
    /// The heap buffer address of a vector's elements, allocated on first use.
    fn vector_buffer(&mut self, vector: CellId) -> Result<u64, Diagnostic> {
        if let Some(base) = self.pointers.buffers.get(&vector) {
            return Ok(*base);
        }
        let fields = self.sequence_cells(vector)?;
        let stride = self.element_stride(&fields);
        let reserve = VECTOR_RESERVE.max((fields.len() as u64).saturating_mul(stride).saturating_mul(2));
        let base = self.claim_range(reserve, 16);
        self.pointers.buffers.insert(vector, base);
        for (index, element) in fields.iter().enumerate() {
            self.register_subtree(*element, base.wrapping_add((index as u64).saturating_mul(stride)));
        }
        Ok(base)
    }
    /// The element cells of the array or vector in `cell`.
    fn sequence_cells(&self, cell: CellId) -> Result<Vec<CellId>, Diagnostic> {
        let (_, storage) = self.place_object(cell)?;
        Ok(storage.fields.clone())
    }
    /// The address of element `index` of the sequence in `sequence`; one past
    /// the end is a plain address in the sequence's allocation.
    fn sequence_element_address(&mut self, sequence: CellId, index: u32) -> Result<u64, Diagnostic> {
        let fields = self.sequence_cells(sequence)?;
        if let Some(element) = fields.get(index as usize) {
            return self.cell_address(*element);
        }
        let ty = self.place_type(sequence)?;
        let base = match self.types.descriptor(ty)?.key.as_ref() {
            TypeKey::Array { .. } => self.cell_address(sequence)?,
            _ => self.vector_buffer(sequence)?,
        };
        let stride = if fields.is_empty() {
            match self.sequence_shape(sequence, ty)? {
                Some((element, _)) => self.type_layout(element).0,
                None => 0,
            }
        } else {
            self.element_stride(&fields)
        };
        Ok(base.wrapping_add(u64::from(index).saturating_mul(stride)))
    }
    fn derive_address(&mut self, cell: CellId) -> Result<u64, Diagnostic> {
        let owner = self.cells.get(cell)?.owner;
        let CellOwner::Field { object, index } = owner else {
            return Ok(self.fresh_range(cell));
        };
        let Some(ValueOwner::Cell(parent)) = self.objects.get(object)?.owner else {
            return Ok(self.fresh_range(cell));
        };
        let Ok(parent_type) = self.place_type(parent) else {
            return Ok(self.fresh_range(cell));
        };
        let key = std::sync::Arc::clone(&self.types.descriptor(parent_type)?.key);
        match key.as_ref() {
            TypeKey::Nominal { identity, .. } if self.owned_box == Some(*identity) => {
                Ok(self.fresh_range(cell))
            }
            TypeKey::Nominal { identity, .. } if self.owned_vec == Some(*identity) => {
                let base = self.vector_buffer(parent)?;
                if let Some(address) = self.pointers.cells.get(&cell) {
                    return Ok(*address);
                }
                let stride = self.cell_layout(cell).0;
                Ok(base.wrapping_add(u64::from(index).saturating_mul(stride)))
            }
            _ => {
                // The parent's allocation holds this cell: registering the
                // parent registers every child at its offset.
                self.cell_address(parent)?;
                match self.pointers.cells.get(&cell) {
                    Some(address) => Ok(*address),
                    None => Ok(self.fresh_range(cell)),
                }
            }
        }
    }
    fn fresh_range(&mut self, cell: CellId) -> u64 {
        let (size, align) = self.cell_layout(cell);
        self.claim_range(size, align)
    }
    /// The stable synthetic address of a cell (and, on first request, of
    /// everything inside its allocation).
    pub(crate) fn cell_address(&mut self, cell: CellId) -> Result<u64, Diagnostic> {
        self.address_stamp(cell)?;
        if let Some(address) = self.pointers.cells.get(&cell) {
            return Ok(*address);
        }
        let address = self.derive_address(cell)?;
        if !self.pointers.cells.contains_key(&cell) {
            self.register_subtree(cell, address);
        }
        Ok(address)
    }
    /// The code address of a function item or closure body.
    pub(crate) fn function_address(&mut self, identity: Digest) -> u64 {
        if let Some(address) = self.pointers.functions.get(&identity) {
            return *address;
        }
        let address = CODE_START + 16 * self.pointers.functions.len() as u64;
        self.pointers.functions.insert(identity, address);
        self.pointers.code.insert(address, identity);
        address
    }
    /// The callee identity a code address designates.
    pub(crate) fn function_at(&self, address: u64) -> Option<Digest> {
        self.pointers.code.get(&address).copied()
    }
    fn string_address(&mut self, buffer: BufferLease) -> Result<u64, Diagnostic> {
        if let Some((_, address)) = self
            .pointers
            .strings
            .iter()
            .find(|(known, _)| *known == buffer)
        {
            return Ok(*address);
        }
        let length = self.strings.resolve(buffer)?.len() as u64;
        let address = self.claim_range(length, 1);
        self.pointers.strings.push((buffer, address));
        Ok(address)
    }
    /// The live cell that starts at `address`, preferring one of type
    /// `pointee` (a struct and its first field share an address).
    pub(super) fn cell_at(&self, address: u64, pointee: TypeId) -> Option<CellId> {
        let candidates = self.pointers.starts.get(&address)?;
        let mut fallback = None;
        for cell in candidates {
            if self.address_stamp(*cell).is_err() {
                continue;
            }
            if self.address_type(*cell).is_ok_and(|ty| ty == pointee) {
                return Some(*cell);
            }
            fallback.get_or_insert(*cell);
        }
        fallback
    }
    fn cell_at_exact(&self, address: u64, pointee: TypeId) -> Option<CellId> {
        let candidates = self.pointers.starts.get(&address)?;
        candidates.iter().copied().find(|cell| {
            self.address_stamp(*cell).is_ok()
                && self.address_type(*cell).is_ok_and(|ty| ty == pointee)
        })
    }
    /// Forget everything the table knows (a new run epoch).
    pub(super) fn reset_pointers(&mut self) {
        self.pointers = PointerTable::default();
    }

    // ---- Pointer values ---------------------------------------------------

    fn publish_pointer(
        &mut self,
        lease: ReferenceLease,
        target: TypeId,
        pointer: TypeId,
    ) -> Result<ValueRepr, Diagnostic> {
        let record = ReferenceRecord {
            access: Access::Shared,
            owner: None,
            interior: false,
            raw: true,
            target,
            lease,
        };
        let bytes = record.owned_bytes()?;
        self.references.reserve(1, &mut self.budget)?;
        let id: RefId = self
            .references
            .allocate(record, bytes, &mut self.budget)?
            .into();
        Ok(ValueRepr {
            kind: 9,
            type_id: pointer.0,
            payload: id.bits(),
        })
    }
    fn designated(&self, value: ValueRepr) -> Result<(Designated, TypeId), Diagnostic> {
        if value.kind != 9 {
            return Err(pointer_type_fault(
                "pointer operand is not a reference or pointer",
            ));
        }
        let record = self.references.get(RefId::from_bits(value.payload))?;
        self.validate_reference_record(record)?;
        let designated = match &record.lease {
            ReferenceLease::Detached(address) => Designated::Address(*address),
            ReferenceLease::Location(location) => Designated::Cell(location.final_cell.cell),
            ReferenceLease::Elements(elements) => Designated::Elements {
                array: elements.base.final_cell.cell,
                start: elements.start,
            },
            ReferenceLease::Buffer(buffer) => Designated::Buffer(*buffer),
        };
        Ok((designated, record.target))
    }
    /// The pointee type a pointer descriptor names.
    fn pointer_pointee(&self, pointer: TypeId) -> Result<TypeId, Diagnostic> {
        match self.types.descriptor(pointer)?.key.as_ref() {
            TypeKey::RawPtr { target, .. } | TypeKey::Ref { target, .. } => Ok(*target),
            _ => Err(pointer_type_fault("operand is not a pointer type")),
        }
    }
    /// The null pointer of a pointer descriptor.
    pub(crate) fn null_pointer(&mut self, pointer: TypeId) -> Result<ValueRepr, Diagnostic> {
        let target = self.pointer_pointee(pointer)?;
        self.publish_pointer(ReferenceLease::Detached(0), target, pointer)
    }
    /// `e as *const T` / `*mut T` / `fn(..)`: the operand is a reference, a
    /// pointer, a function item or closure value, or an integer.
    pub(crate) fn pointer_cast(
        &mut self,
        value: ValueRepr,
        target: TypeId,
    ) -> Result<ValueRepr, Diagnostic> {
        let key = std::sync::Arc::clone(&self.types.descriptor(target)?.key);
        let (pointee, function) = match key.as_ref() {
            TypeKey::RawPtr { target, .. } => (*target, false),
            TypeKey::Function { .. } => (TypeId(1), true),
            _ => {
                return Err(pointer_type_fault(
                    "pointer cast target is not a pointer type",
                ));
            }
        };
        match value.kind {
            // An integer address; a signed value is sign extended by its
            // representation (`!0usize`, `-1i8 as *const ()` agree).
            4 | 5 => self.publish_pointer(ReferenceLease::Detached(value.payload), pointee, target),
            // A function item or non-capturing closure: its code address.
            8 => {
                let identity = self.callable_identity(value)?;
                let address = self.function_address(identity);
                self.publish_pointer(ReferenceLease::Detached(address), pointee, target)
            }
            9 => {
                let id = RefId::from_bits(value.payload);
                let (designated, old_target) = self.designated(value)?;
                let fat = !function && self.pointer_width(pointee) == 16;
                let element_of_window = match self.types.descriptor(old_target)?.key.as_ref() {
                    TypeKey::Slice { element } => *element,
                    _ => old_target,
                };
                let (lease, mut record_target) = match designated {
                    Designated::Address(address) => {
                        (ReferenceLease::Detached(address), old_target)
                    }
                    Designated::Cell(cell) => {
                        (ReferenceLease::Detached(self.cell_address(cell)?), old_target)
                    }
                    Designated::Elements { .. } if fat => {
                        (clone_lease(&self.references.get(id)?.lease), old_target)
                    }
                    Designated::Elements { array, start } => (
                        ReferenceLease::Detached(self.sequence_element_address(array, start)?),
                        element_of_window,
                    ),
                    Designated::Buffer(buffer) if fat => {
                        (ReferenceLease::Buffer(buffer), old_target)
                    }
                    Designated::Buffer(buffer) => {
                        (ReferenceLease::Detached(self.string_address(buffer)?), TypeId(9))
                    }
                };
                // An array pointer cast to its element pointer, or a struct
                // pointer to its first field's, designates that inner cell.
                if !function
                    && let ReferenceLease::Detached(address) = &lease
                    && self.cell_at_exact(*address, pointee).is_some()
                {
                    record_target = pointee;
                }
                self.publish_pointer(lease, record_target, target)
            }
            _ => Err(pointer_type_fault(
                "only references, pointers, function items and integers cast to pointers",
            )),
        }
    }
    fn callable_identity(&self, value: ValueRepr) -> Result<Digest, Diagnostic> {
        match self.types.descriptor(TypeId(value.type_id))?.key.as_ref() {
            TypeKey::Callable {
                identity, captures, ..
            } if captures.is_empty() => Ok(*identity),
            _ => Err(pointer_type_fault(
                "only a function item or non-capturing closure casts to a function pointer",
            )),
        }
    }
    /// Whether `value` (a reference or pointer) coerces implicitly to the
    /// pointer descriptor `expected`: `&T` to `*const T`, `&mut T` to `*mut T`
    /// or `*const T`, a raw pointer to a raw pointer of the same family, and a
    /// function item or non-capturing closure to a fn pointer.
    pub(crate) fn pointer_coercion(
        &self,
        value: ValueRepr,
        expected: TypeId,
    ) -> Result<bool, Diagnostic> {
        if value.kind != 8 && value.kind != 9 {
            return Ok(false);
        }
        self.pointer_type_coercion(TypeId(value.type_id), expected)
    }
    /// Pure descriptor admission shared by reached call contracts and the
    /// physical pointer conversion. This does not convert or consume a value.
    pub(crate) fn pointer_type_coercion(
        &self,
        actual: TypeId,
        expected: TypeId,
    ) -> Result<bool, Diagnostic> {
        let TypeKey::RawPtr {
            mutable: to_mutable,
            ..
        } = self.types.descriptor(expected)?.key.as_ref()
        else {
            return Ok(match self.types.descriptor(expected)?.key.as_ref() {
                TypeKey::Function { .. } => matches!(
                    self.types.descriptor(actual)?.key.as_ref(),
                    TypeKey::Callable { captures, .. } if captures.is_empty()
                ),
                _ => false,
            });
        };
        Ok(match self.types.descriptor(actual)?.key.as_ref() {
            TypeKey::Ref { mutable, .. } => !*to_mutable || *mutable,
            TypeKey::RawPtr { .. } => true,
            _ => false,
        })
    }
    /// The address a pointer or function-pointer value holds, when it is a
    /// plain address (no allocation of a new address is needed).
    pub(crate) fn pointer_address_peek(&self, value: ValueRepr) -> Result<Option<u64>, Diagnostic> {
        if value.kind != 9 {
            return Ok(None);
        }
        let record = self.references.get(RefId::from_bits(value.payload))?;
        if !record.raw {
            return Ok(None);
        }
        Ok(match record.lease {
            ReferenceLease::Detached(address) => Some(address),
            _ => None,
        })
    }
    /// `p as usize` operand of a cell holding a pointer value.
    pub(crate) fn pointer_address_operand(
        &self,
        cell: CellId,
    ) -> Result<Option<ValueRepr>, Diagnostic> {
        let record = self.cells.get(cell)?;
        let CellState::Live(value) = record.state else {
            return Ok(None);
        };
        // A function item value casts as its code address (the address was
        // minted when the item value was formed).
        if value.kind == 8 {
            if let TypeKey::Callable {
                identity, captures, ..
            } = self.types.descriptor(TypeId(value.type_id))?.key.as_ref()
                && captures.is_empty()
                && let Some(address) = self.pointers.functions.get(identity)
            {
                return ValueRepr::unsigned(TypeId(13), *address).map(Some);
            }
            return Ok(None);
        }
        if value.kind != 9 {
            return Ok(None);
        }
        let key = self.types.descriptor(TypeId(value.type_id))?.key.as_ref();
        if !matches!(key, TypeKey::RawPtr { .. } | TypeKey::Function { .. }) {
            return Ok(None);
        }
        let Some(address) = self.pointer_address_peek(value)? else {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "address of a fat slice or str pointer requires its thin pointer first".into(),
            ));
        };
        ValueRepr::unsigned(TypeId(13), address).map(Some)
    }
    /// The address of any reference or pointer value, allocating one if the
    /// value designates a cell without an address yet.
    pub(crate) fn pointer_address(&mut self, value: ValueRepr) -> Result<u64, Diagnostic> {
        let (designated, _) = self.designated(value)?;
        match designated {
            Designated::Address(address) => Ok(address),
            Designated::Cell(cell) => self.cell_address(cell),
            Designated::Elements { array, start } => self.sequence_element_address(array, start),
            Designated::Buffer(buffer) => self.string_address(buffer),
        }
    }
    /// The size in bytes one step of a pointer moves it.
    fn pointer_stride(&self, pointer: ValueRepr, address: u64) -> Result<u64, Diagnostic> {
        let pointee = self.pointer_pointee(TypeId(pointer.type_id))?;
        let record_target = self
            .references
            .get(RefId::from_bits(pointer.payload))?
            .target;
        Ok(match self.cell_at(address, pointee) {
            Some(cell) if self.address_type(cell).is_ok_and(|ty| ty == pointee) => {
                self.cell_layout(cell).0
            }
            _ => {
                let layout = self.type_layout(pointee);
                if layout.0 == 0 && pointee.0 != 1 {
                    self.type_layout(record_target).0
                } else {
                    layout.0
                }
            }
        })
    }
    /// `p.cast_mut()` / `cast_const()`: the same address with the other
    /// mutability.
    pub(crate) fn pointer_with_mutability(
        &mut self,
        value: ValueRepr,
        mutable: bool,
    ) -> Result<ValueRepr, Diagnostic> {
        let pointee = self.pointer_pointee(TypeId(value.type_id))?;
        let target = self.raw_pointer_type(mutable, pointee)?;
        self.pointer_cast(value, target)
    }
    /// `slice.as_ptr()`: the pointer to the first element of the sequence a
    /// slice, array or vector reference designates.
    pub(crate) fn elements_pointer(
        &mut self,
        reference: ValueRepr,
        mutable: bool,
    ) -> Result<ValueRepr, Diagnostic> {
        let (designated, old_target) = self.designated(reference)?;
        let (sequence, start) = match designated {
            Designated::Elements { array, start } => (array, start),
            Designated::Cell(cell) => (cell, 0),
            _ => {
                return Err(pointer_type_fault(
                    "as_ptr requires a slice, array or vector reference",
                ));
            }
        };
        let element = match self.types.descriptor(old_target)?.key.as_ref() {
            TypeKey::Slice { element } => *element,
            _ => match self.sequence_shape(sequence, old_target)? {
                Some((element, _)) => element,
                None => {
                    return Err(pointer_type_fault(
                        "as_ptr requires a slice, array or vector reference",
                    ));
                }
            },
        };
        let address = self.sequence_element_address(sequence, start)?;
        let pointer = self.raw_pointer_type(mutable, element)?;
        self.publish_pointer(ReferenceLease::Detached(address), element, pointer)
    }
    /// `p.offset(n)` / `add` / `sub` (`n` in elements of the pointee).
    pub(crate) fn pointer_offset(
        &mut self,
        pointer: ValueRepr,
        count: i64,
    ) -> Result<ValueRepr, Diagnostic> {
        let address = self.pointer_address(pointer)?;
        let stride = self.pointer_stride(pointer, address)?;
        let moved = address.wrapping_add((count as u64).wrapping_mul(stride));
        let target = self
            .references
            .get(RefId::from_bits(pointer.payload))?
            .target;
        self.publish_pointer(ReferenceLease::Detached(moved), target, TypeId(pointer.type_id))
    }
    /// `p.offset_from(q)`: the distance in elements.
    pub(crate) fn pointer_offset_from(
        &mut self,
        pointer: ValueRepr,
        origin: ValueRepr,
    ) -> Result<i64, Diagnostic> {
        let address = self.pointer_address(pointer)?;
        let base = self.pointer_address(origin)?;
        let stride = self.pointer_stride(pointer, address)?;
        if stride == 0 {
            return Err(Diagnostic::new(
                "LRR-E-OP-TYPE",
                "offset_from on a zero-sized pointee".into(),
            ));
        }
        Ok((address.wrapping_sub(base) as i64) / (stride as i64))
    }
    /// `p == q`: address equality.
    pub(crate) fn pointer_eq(&mut self, a: ValueRepr, b: ValueRepr) -> Result<bool, Diagnostic> {
        Ok(self.pointer_address(a)? == self.pointer_address(b)?)
    }
    /// The cell a pointer designates now.
    pub(crate) fn pointer_cell(&self, pointer: ValueRepr) -> Result<CellId, Diagnostic> {
        let (designated, target) = self.designated(pointer)?;
        match designated {
            Designated::Address(address) => self.cell_at(address, target).ok_or_else(dangling),
            Designated::Cell(cell) => Ok(cell),
            Designated::Elements { array, start } => {
                let fields = self.sequence_cells(array)?;
                fields.get(start as usize).copied().ok_or_else(dangling)
            }
            Designated::Buffer(_) => Err(dangling()),
        }
    }
    /// `ptr::read`: copy the value (or move it, when it is not `Copy`).
    pub(crate) fn pointer_read(&mut self, pointer: ValueRepr) -> Result<TicketId, Diagnostic> {
        let cell = self.pointer_cell(pointer)?;
        self.read_owning(cell)
    }
    /// `ptr::write`: store without dropping the old value.
    pub(crate) fn pointer_write(
        &mut self,
        pointer: ValueRepr,
        incoming: TicketId,
    ) -> Result<(), Diagnostic> {
        let cell = self.pointer_cell(pointer)?;
        if matches!(self.cells.get(cell)?.state, CellState::Live(_)) {
            let old = self.replace(cell, incoming)?;
            if self.peek_ticket(old)?.kind == 8 {
                let unit = self.forget_value(old)?;
                self.discard(unit)
            } else {
                self.discard(old)
            }
        } else {
            self.initialize(cell, incoming)
        }
    }
}
